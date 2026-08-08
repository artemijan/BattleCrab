//! Carried weight — port of `Creature.getMaxLoad`, `Player.refreshOverloaded`
//! and `Player.getInventoryLimit`/`isInventoryUnder80`.
//!
//! Three separate deferrals across this port were waiting on "no weight calc
//! exists": `//diet`'s overload immunity, TvT's `isInventoryUnder80` +
//! `getWeightPenalty()` registration gates, and the inventory-full refusals.
//! They all read from here now.
//!
//! The penalty itself is not bespoke arithmetic: Java applies **skill 4270
//! "Weight Penalty"** at level 1-4, a passive carrying the actual speed and
//! HP/MP-regen maluses. That makes this a sibling of
//! [`super::expertise::refresh_expertise_penalty`], and it is built the same
//! way — swap a passive buff, then resend `EtcStatusUpdate` + `UserInfo`.

use crate::model::Player;
use crate::model::components::{
    BaseStats, Buffs, CombatStats, Speeds, StatModifiers, WeightPenalty,
};
use crate::model::inventory::Inventory;
use crate::model::skill::{ActiveBuff, StatModifierEffect};
use crate::world::World;

/// Java `CommonSkill.WEIGHT_PENALTY`.
const WEIGHT_PENALTY_SKILL: i32 = 4270;

/// Java's `BaseStat.CON.calcBonus(this) * 69000` — the "weight limit" constant
/// from the 2007 formula its own comment cites.
const BASE_LOAD_PER_CON_BONUS: f64 = 69_000.0;

/// `Player.getCurrentLoad()` — the summed weight of everything carried.
///
/// Equipped items count: in L2 wearing armour does not make it weightless.
pub(crate) fn total_load(inventory: &Inventory, data: &crate::data::GameData) -> i64 {
    inventory
        .items()
        .iter()
        .map(|item| {
            data.item_data
                .get(item.item_id)
                .map_or(0, |t| t.weight as i64 * item.count)
        })
        .sum()
}

/// `Creature.getMaxLoad()` — `floor(CON bonus * 69000 * AltWeightLimit)`.
///
/// `AltWeightLimit` is **3** on this dist, so the shipped limit is three times
/// the retail formula. Reading it from config rather than inlining matters:
/// the whole penalty ladder is a ratio against this number, so a hard-coded
/// 1.0 would put every character permanently overloaded on an operator's
/// server that raised it.
pub(crate) fn max_load(world: &World, object_id: i32) -> i32 {
    let Some(base) = world.objects.get_component::<BaseStats>(&object_id) else {
        return 0;
    };
    let bonus = world.data.stat_bonus.con_bonus(base.con);
    // Java: `getValue(Stat.WEIGHT_LIMIT, floor(CON bonus × 69000 × config))` —
    // the CON formula is the *base* the stat's add/mul apply to, and Java
    // floors it before the stat pass (G34 S4: Weight Limit 150, Quiver of
    // Holding 418, Super Haste 7029 all pump it as `PER`).
    let base_load =
        (bonus * BASE_LOAD_PER_CON_BONUS * world.cfg.character.alt_weight_limit).floor();
    stat_value(
        world,
        object_id,
        crate::model::stats::Stat::WeightLimit,
        base_load,
    ) as i32
}

/// Java `CreatureStat.getValue(stat, base)` for the two stats this module
/// reads: `(base + add) × mul`, identity when the creature carries neither.
fn stat_value(world: &World, object_id: i32, stat: crate::model::stats::Stat, base: f64) -> f64 {
    let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&object_id)
    else {
        return base;
    };
    (base + mods.add.get(&stat).copied().unwrap_or(0.0))
        * mods.mul.get(&stat).copied().unwrap_or(1.0)
}

/// Java `Creature.getBonusWeightPenalty()` — `getValue(WEIGHT_PENALTY, 1)`.
///
/// **The name lies.** It reads like a penalty *band*, but every caller
/// subtracts it from the **carried weight**:
/// `weightproc = (getCurrentLoad() - getBonusWeightPenalty()) * 1000 / getMaxLoad()`
/// (`Player.refreshOverloaded`, `Pet`, and the two weight conditions). The
/// datapack settles it — Decrease Weight (1257) grants 3000/6000/9000, which
/// are weight units, not bands. Ported as the code behaves, not as the name
/// reads ([[l2r-port-behaviour-not-intent]]).
///
/// Base 1, so a character with no such skill still has 1 subtracted — Java's,
/// kept.
pub(crate) fn bonus_weight_penalty(world: &World, object_id: i32) -> i64 {
    stat_value(
        world,
        object_id,
        crate::model::stats::Stat::WeightPenalty,
        1.0,
    ) as i64
}

/// Java's band table in `refreshOverloaded`, extracted so it can be tested
/// without a world: permille of the limit → penalty level.
///
/// `diet` short-circuits to 0 — that is `//diet`, the GM immunity that had no
/// reader in this port until now.
pub(crate) fn penalty_level(load: i64, max_load: i32, diet: bool) -> i32 {
    if max_load <= 0 {
        return 0;
    }
    let permille = load.saturating_mul(1000) / max_load as i64;
    if diet || permille < 500 {
        0
    } else if permille < 666 {
        1
    } else if permille < 800 {
        2
    } else if permille < 1000 {
        3
    } else {
        4
    }
}

/// Port of `Player.refreshOverloaded(true)`. Recomputes the carried load, swaps
/// the 4270 passive to the new level (or removes it), records the overloaded
/// flag, and resends the status packets when the level changed.
///
/// Call after anything that changes what a character carries.
pub(crate) fn refresh_weight_penalty(world: &mut World, object_id: i32) {
    let max = max_load(world, object_id);
    if max <= 0 {
        return; // Java: `if (maxLoad > 0)` — no limit, no penalty.
    }
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return;
    };
    // The memoized load makes the periodic sweep skip the per-stack walk for
    // every player whose inventory didn't change since last time. The debug
    // re-derivation is the rot guard the sweep's doc-comment worries about: a
    // new count-mutating `Inventory` method that forgets to unsettle the
    // cache fails the whole test suite, not production.
    let load = match inventory.cached_load() {
        Some(cached) => {
            #[cfg(debug_assertions)]
            assert_eq!(
                cached,
                total_load(inventory, &world.data),
                "stale cached inventory load for {object_id} — a count-mutating \
                 Inventory method missed `load_settled = false`"
            );
            cached
        }
        None => {
            let computed = total_load(inventory, &world.data);
            if let Some(inv) = world.objects.get_component_mut::<Inventory>(&object_id) {
                inv.settle_load(computed);
            }
            computed
        }
    };
    let diet = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.diet);
    // `refreshOverloaded` weighs `getCurrentLoad() - getBonusWeightPenalty()`,
    // so the bonus comes off the load *before* the band lookup.
    let effective_load = (load - bonus_weight_penalty(world, object_id)).max(0);
    let level = penalty_level(effective_load, max, diet);
    // Java sets `overloaded` only on the penalised branch, and clears it
    // otherwise — so a diet-mode GM is never overloaded however much they
    // carry, which is the point of the flag.
    let overloaded = level > 0 && !diet && effective_load > max as i64;

    let current = world
        .objects
        .get_component::<WeightPenalty>(&object_id)
        .copied()
        .unwrap_or_default();
    if current.level == level && current.overloaded == overloaded {
        return;
    }

    let effects = penalty_effects(world, level);
    {
        let Some((player, base, mut mods, inv, mut buffs, mut speeds, mut combat)) =
            world.objects.get_many_mut::<(
                &Player,
                &BaseStats,
                &mut StatModifiers,
                &Inventory,
                &mut Buffs,
                &mut Speeds,
                &mut CombatStats,
            )>(&object_id)
        else {
            return;
        };
        // Remove first so the modifier maps rebuild from the remaining buffs —
        // otherwise stepping 2 → 3 would stack both levels' speed maluses.
        player.remove_buff(
            &world.data,
            base,
            &mut mods,
            inv,
            &mut buffs,
            &mut speeds,
            &mut combat,
            WEIGHT_PENALTY_SKILL,
        );
        if let Some((lvl, effects)) = effects {
            player.apply_buff(
                &world.data,
                base,
                &mut mods,
                inv,
                &mut buffs,
                &mut speeds,
                &mut combat,
                passive_weight_buff(lvl, effects),
            );
        }
    }

    world
        .objects
        .add_components(&object_id, WeightPenalty { level, overloaded });

    // Java: `sendPacket(new EtcStatusUpdate(this)); broadcastUserInfo();`
    if let Some(client_id) = crate::game_loop::helpers::client_for_player(world, object_id)
        && let Some(view) = crate::model::PlayerView::of_world(world, object_id)
    {
        let flags = world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&object_id)
            .copied()
            .unwrap_or_default();
        let ep = world
            .objects
            .get_component::<crate::model::components::ExpertisePenalty>(&object_id)
            .copied()
            .unwrap_or_default();
        let user_info = crate::network::user_info::user_info(
            &view,
            &world.data,
            &world.cfg.character,
            super::party::calculate_relation(world, view.p),
        );
        let charges = view.p.charges;
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::enter_world::etc_status_update(
                charges,
                level,
                ep.weapon,
                ep.armor,
                flags.silence,
            ));
            cs.send(user_info);
        }
    }
    // The weight bar itself rides in `ExUserInfoInvenWeight`, which the
    // inventory-update helper already sends on every item change.
}

/// `Player.isOverloaded()` — carrying more than `getMaxLoad()`.
///
/// Computed from the inventory rather than read off the cached component, so
/// the *gate* is exact the instant an item lands. Only the 4270 passive and the
/// client icon settle on the sweep below; a caller asking "may this player pick
/// this up" never sees a stale answer.
pub(crate) fn is_overloaded(world: &World, object_id: i32) -> bool {
    let max = max_load(world, object_id);
    if max <= 0 {
        return false;
    }
    if world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.diet)
    {
        return false; // `//diet` — the GM immunity
    }
    world
        .objects
        .get_component::<Inventory>(&object_id)
        .is_some_and(|inv| total_load(inv, &world.data) > max as i64)
}

/// Re-apply the weight penalty for every in-game player.
///
/// **A deliberate deviation, and the reason is structural.** Java hangs
/// `refreshOverloaded` off `ItemContainer.refreshWeight`, which every add and
/// remove funnels through. This port has no such funnel — inventories are
/// mutated directly through the component — so an event-driven port would mean
/// finding and annotating every mutation site, and would silently rot the first
/// time someone added another.
///
/// Instead the *gates* (`is_overloaded`, `is_inventory_under_80`) read live and
/// are always exact; only the passive's stat malus and the client's icon settle
/// on this sweep, within one regen tick. Equip, enter-world and offline-shop
/// restore additionally refresh immediately, so the common cases are instant.
pub(crate) fn sweep(world: &mut World) {
    let players: Vec<i32> = world.in_game_player_oids().collect();
    for oid in players {
        refresh_weight_penalty(world, oid);
    }
}

/// `Player.getWeightPenalty()` — the live band, computed rather than read off
/// the component so callers gating on it (TvT registration) are never told a
/// stale answer.
pub(crate) fn current_penalty(world: &World, object_id: i32) -> i32 {
    let max = max_load(world, object_id);
    let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
        return 0;
    };
    let diet = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.diet);
    penalty_level(total_load(inv, &world.data), max, diet)
}

/// `Player.getInventoryLimit()` — the config slot cap for the race (GMs get
/// their own), plus any `EnlargeSlot` passive bonus.
pub(crate) fn inventory_limit(world: &World, object_id: i32) -> i32 {
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return 0;
    };
    let cfg = &world.cfg.character;
    let base = cfg.inventory_limit_for(player.race, player.is_gm(&world.data));
    let Some(mods) = world.objects.get_component::<StatModifiers>(&object_id) else {
        return base;
    };
    crate::model::finalize(
        mods,
        crate::model::stats::Stat::InventoryNormal,
        base as f64,
    ) as i32
}

/// `Player.isInventoryUnder80(false)` — the **slot** check, not the weight one.
///
/// Java's argument selects whether quest items count; every caller in this
/// datapack passes `false`, so this is `getNonQuestSize()`.
pub(crate) fn is_inventory_under_80(world: &World, object_id: i32) -> bool {
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return true;
    };
    let used = inventory.non_quest_size(&world.data.item_data) as f64;
    used <= inventory_limit(world, object_id) as f64 * 0.8
}

fn penalty_effects(world: &World, level: i32) -> Option<(i32, Vec<StatModifierEffect>)> {
    if level <= 0 {
        return None;
    }
    let skill = world.data.skill_data.get(WEIGHT_PENALTY_SKILL, level)?;
    Some((level, skill.stat_modifier_effects()))
}

/// The 4270 passive, shaped like the grade-penalty buffs: hidden from
/// `AbnormalStatusUpdate` and never scheduled to expire.
fn passive_weight_buff(level: i32, effects: Vec<StatModifierEffect>) -> ActiveBuff {
    ActiveBuff::passive_pump(WEIGHT_PENALTY_SKILL, level, effects)
}

/// Java `PlayerInventory.validateWeight(long)` — would the player still be
/// within `getMaxLoad()` after taking on `added` more weight?
///
/// The GM bypass is Java's and is deliberately all three conditions
/// (`isGM() && getDietMode() && getAccessLevel().allowTransaction()`), not just
/// `isGM()`: a GM with diet mode off is weighed like anyone else.
pub(crate) fn validate_weight(world: &World, object_id: i32, added: i64) -> bool {
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return false;
    };
    let diet = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.diet);
    if player.is_gm(&world.data) && diet {
        return true;
    }
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return false;
    };
    total_load(inventory, &world.data) + added <= i64::from(max_load(world, object_id))
}

/// Java `PlayerInventory.validateCapacity(long slots)` — is there room for
/// `slots` more *non-quest* slots? Every caller in this datapack passes
/// `questItem = false`, so only that branch is ported.
pub(crate) fn validate_capacity(world: &World, object_id: i32, slots: i64) -> bool {
    let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
        return false;
    };
    let used = inventory.non_quest_size(&world.data.item_data) as i64;
    used + slots <= i64::from(inventory_limit(world, object_id))
}

/// The slot cost of adding `count` of `item_id`, as Java counts it when
/// validating a bulk purchase: a non-stackable item needs one slot per unit, a
/// stackable one needs a slot only if the player holds none yet.
pub(crate) fn slots_needed(world: &World, object_id: i32, item_id: i32, count: i64) -> i64 {
    let stackable = world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_stackable);
    if !stackable {
        return count;
    }
    let held = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map_or(0, |i| i.count_of(item_id));
    i64::from(held == 0)
}
