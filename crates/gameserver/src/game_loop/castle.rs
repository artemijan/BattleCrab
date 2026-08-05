//! The castle treasury — port of `Castle.addToTreasury` /
//! `Castle.addToTreasuryNoTax` / `Castle.getTaxPercent` (Java
//! `model/siege/Castle`).
//!
//! The treasury (`castle.treasury`) is the castle owner's vault. Money reaches
//! it three ways, all of them routed through this module:
//!
//! - **tax** on purchases made inside the castle's `TaxZone`
//!   ([`add_to_treasury`], which pays the liege castle its cut first),
//! - **manor** seed sales at the Manor Manager ([`add_to_treasury_no_tax`]),
//! - the owner's **chamberlain deposits** (also no-tax).
//!
//! It drains through chamberlain withdrawals and (once the manor settlement
//! lands) the period's manor cost. Java persists on *every* change with a
//! single-row `UPDATE castle SET treasury = ?`, before which nothing is
//! cached — the port does the same through [`DbCommand::UpdateCastleTreasury`],
//! so a crash can lose at most the in-flight command.
//!
//! **An unowned castle has no treasury**: Java's `_ownerId <= 0` guard makes
//! every add a no-op returning `false`, so seed money spent at a castle nobody
//! holds simply vanishes (and the tax cascade skips a liege with no owner).

use crate::db::DbCommand;
use crate::model::castle::{CastleSide, TaxType};
use crate::world::World;

/// Java `Castle.getTaxPercent(type)` — the percent for the castle's current
/// side, off `Feature.ini`. An unknown castle taxes nothing.
pub(crate) fn tax_percent(world: &World, castle_id: i32, tax_type: TaxType) -> i32 {
    let Some(castle) = world.castles.iter().find(|c| c.id == castle_id) else {
        return 0;
    };
    let f = &world.cfg.feature;
    match (castle.side, tax_type) {
        (CastleSide::Light, TaxType::Buy) => f.castle_buy_tax_light,
        (CastleSide::Light, TaxType::Sell) => f.castle_sell_tax_light,
        (CastleSide::Dark, TaxType::Buy) => f.castle_buy_tax_dark,
        (CastleSide::Dark, TaxType::Sell) => f.castle_sell_tax_dark,
        (CastleSide::Neutral, TaxType::Buy) => f.castle_buy_tax_neutral,
        (CastleSide::Neutral, TaxType::Sell) => f.castle_sell_tax_neutral,
    }
}

/// Java `Castle.getTaxRate(type)` — [`tax_percent`] as a fraction. The buy rate
/// is what a merchant standing in the castle's tax zone adds to its prices.
pub(crate) fn tax_rate(world: &World, castle_id: i32, tax_type: TaxType) -> f64 {
    f64::from(tax_percent(world, castle_id, tax_type)) / 100.0
}

/// Java `Npc.getTaxCastle()` — the castle whose `TaxZone` this NPC stands in,
/// or `None` when it stands in none (most of the map). Java latches the zone
/// onto the NPC when it spawns/enters (`setTaxZone`) and never recomputes it;
/// NPCs don't wander between tax zones, so resolving it by geometry at sale
/// time gives the same answer. An NPC inside an instance has no tax castle in
/// Java (`setTaxZone` ignores the zone) — the same gate applies here.
pub(crate) fn npc_tax_castle(world: &World, npc_oid: i32) -> Option<i32> {
    if super::helpers::instance_of(world, npc_oid) != 0 {
        return None;
    }
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&npc_oid)?;
    world
        .data
        .zone_data
        .tax_castle_at(pos.x, pos.y, pos.z)
        .filter(|&id| id > 0)
}

/// Java `Npc.getCastleTaxRate(TaxType.BUY)` — the buy-tax fraction a merchant
/// adds to its prices, or 0 when it stands outside every tax zone.
pub(crate) fn npc_tax_rate(world: &World, npc_oid: i32) -> f64 {
    npc_tax_castle(world, npc_oid).map_or(0.0, |id| tax_rate(world, id, TaxType::Buy))
}

/// Java `Npc.handleTaxPayment(amount)` — pay this NPC's castle its tax, through
/// the liege cascade. A non-positive amount, an NPC outside every tax zone, or
/// an unowned castle all make it a no-op.
pub(crate) fn handle_tax_payment(world: &mut World, npc_oid: i32, amount: i64) {
    if amount <= 0 {
        return;
    }
    if let Some(castle_id) = npc_tax_castle(world, npc_oid) {
        add_to_treasury(world, castle_id, amount);
    }
}

/// Java `Castle.getTreasury()`; 0 for an unknown castle.
pub(crate) fn treasury(world: &World, castle_id: i32) -> i64 {
    world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .map_or(0, |c| c.treasury)
}

/// Java `Castle.addToTreasury(amount)` — credit tax income, paying the castle's
/// **liege** its own buy-tax cut off the top first.
///
/// The liege map is Java's `switch (getName().toLowerCase())`, verbatim: the two
/// north-eastern castles feed Rune, the five original ones feed Aden, and Aden /
/// Rune themselves (and Schuttgart's own liege chain) keep everything. Note the
/// asymmetry Java has and the port keeps: the cut is **subtracted from the
/// payer regardless** of whether the liege is owned — an unowned Aden makes that
/// tax evaporate rather than staying with the vassal.
pub(crate) fn add_to_treasury(world: &mut World, castle_id: i32, amount: i64) {
    // Java's first check: an unowned castle takes nothing (and pays no liege).
    if !is_owned(world, castle_id) {
        return;
    }
    let mut amount = amount;
    if let Some(liege_id) = liege_castle_id(world, castle_id) {
        // Java uses the *liege's* BUY rate, whatever the payer's side is.
        let liege_tax = (amount as f64 * tax_rate(world, liege_id, TaxType::Buy)) as i64;
        if is_owned(world, liege_id) {
            add_to_treasury(world, liege_id, liege_tax);
        }
        amount -= liege_tax;
    }
    add_to_treasury_no_tax(world, castle_id, amount);
}

/// Java `Castle.addToTreasuryNoTax(amount)`. Returns `false` (changing nothing)
/// when the castle is unowned, or when a withdrawal (negative `amount`) is
/// larger than the balance. A credit that would overflow is **clamped to
/// `MaxAdena`**, not rejected. Persists on every change, like Java.
pub(crate) fn add_to_treasury_no_tax(world: &mut World, castle_id: i32, amount: i64) -> bool {
    if !is_owned(world, castle_id) {
        return false;
    }
    let max_adena = world.cfg.character.max_adena;
    let Some(castle) = world.castles.iter_mut().find(|c| c.id == castle_id) else {
        return false;
    };
    if amount < 0 {
        let debit = -amount;
        if castle.treasury < debit {
            return false;
        }
        castle.treasury -= debit;
    } else if castle.treasury.saturating_add(amount) > max_adena {
        castle.treasury = max_adena;
    } else {
        castle.treasury += amount;
    }
    let treasury = castle.treasury;
    let _ = world.db.send(DbCommand::UpdateCastleTreasury {
        castle_id,
        treasury,
    });
    true
}

/// Java `_ownerId > 0` — a clan holds this castle.
fn is_owned(world: &World, castle_id: i32) -> bool {
    super::siege::owner_clan_id_opt(world, castle_id).is_some()
}

/// The castle whose buy tax is skimmed off this castle's tax income, from
/// Java's name `switch` in `addToTreasury`. Names are matched case-insensitively
/// against the `castle` table, exactly as Java does.
fn liege_castle_id(world: &World, castle_id: i32) -> Option<i32> {
    let name = world
        .castles
        .iter()
        .find(|c| c.id == castle_id)?
        .name
        .to_lowercase();
    let liege = match name.as_str() {
        "schuttgart" | "goddard" => "rune",
        "dion" | "giran" | "gludio" | "innadril" | "oren" => "aden",
        _ => return None,
    };
    world
        .castles
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(liege))
        .map(|c| c.id)
}

/// `Util.formatAdena` — group digits into thousands with commas
/// (`200000` → `200,000`), the form every castle-vault page shows.
pub(crate) fn format_adena(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Java `CastleManager._castleCirclets` — castle id → its circlet item id.
/// Index 0 is unused (castle ids are 1..=9), exactly as Java's array is.
const CASTLE_CIRCLETS: [i32; 10] = [0, 6838, 6835, 6839, 6837, 6840, 6834, 6836, 8182, 8183];

/// Java `CastleManager.getCircletByCastleId` — `0` for anything outside 1..=9.
pub(crate) fn circlet_of(castle_id: i32) -> i32 {
    if (1..10).contains(&castle_id) {
        CASTLE_CIRCLETS[castle_id as usize]
    } else {
        0
    }
}

/// Java `CastleManager.removeCirclet(ClanMember, castleId)` — take this
/// castle's circlet off one character, unequipping it first if worn.
///
/// Java has an online and an offline branch (the latter edits the `items` rows
/// directly). This port is memory-first and keeps every logged-in character's
/// inventory in the ECS, so the online branch covers everyone it can reach;
/// a member who is offline keeps the circlet until they log in, where Java
/// would have deleted the row. Recorded rather than silently equivalent.
pub(crate) fn remove_circlet(world: &mut World, member_oid: i32, castle_id: i32) {
    let circlet_id = circlet_of(castle_id);
    if circlet_id == 0 {
        return;
    }
    // `if (circlet.isEquipped()) unEquipItemInSlot(...)` — a worn circlet is
    // taken off before it is destroyed, so the paperdoll does not keep
    // referencing a deleted object.
    let equipped_oid = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&member_oid)
        .and_then(|inv| {
            inv.equipped_items()
                .into_iter()
                .find(|i| i.item_id == circlet_id)
                .map(|i| i.object_id)
        });
    if let Some(oid) = equipped_oid
        && let Some(inv) = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&member_oid)
    {
        inv.unequip_item(oid);
    }
    let changes = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&member_oid)
        .map(|inv| inv.remove_item(circlet_id, 1))
        .unwrap_or_default();
    if changes.is_empty() {
        return;
    }
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, member_oid) {
        let iu = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
        crate::game_loop::helpers::send_inventory_update(world, cid, member_oid, iu);
        // Java's `broadcastUserInfo()` after removal — the circlet is a
        // head-slot item, so onlookers must stop drawing it.
        crate::game_loop::party::broadcast_user_info(world, member_oid);
    }
}

/// Java `CastleManager.removeCirclet(Clan, castleId)` — every member of the
/// clan, gated by the caller on `RemoveCastleCirclets`.
pub(crate) fn remove_circlets_from_clan(world: &mut World, clan_id: i32, castle_id: i32) {
    if !world.cfg.character.remove_castle_circlets {
        return;
    }
    let members: Vec<i32> = world
        .clans
        .get(&clan_id)
        .map(|c| c.members.iter().map(|m| m.char_id).collect())
        .unwrap_or_default();
    for oid in members {
        remove_circlet(world, oid, castle_id);
    }
}

#[cfg(test)]
mod tests {
    use super::format_adena;

    #[test]
    fn format_adena_groups_thousands() {
        assert_eq!(format_adena(0), "0");
        assert_eq!(format_adena(999), "999");
        assert_eq!(format_adena(200_000), "200,000");
        assert_eq!(format_adena(9_999_999_999_999), "9,999,999,999,999");
    }
}
