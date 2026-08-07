//! Pet evolution / exchange / restore — the Pet Manager's three bypass verbs
//! (`model/actor/instance/PetManager.onBypassFeedback` + `util/Evolve`).
//!
//! Found by the 2026-07-31 milestone verification pass: the port already served
//! `petmanager/evolve.htm` and `exchange.htm` through the `Link` whitelist, so
//! the buttons rendered — and did nothing, because no handler claimed the
//! `evolve` / `exchange` / `restore` verbs. Serving a page is not the same as
//! implementing the verb behind it.
//!
//! - **`exchange <n>`** — hand in a Pet Exchange Ticket, get the matching pet
//!   collar. Three pairs, no level or pet requirement.
//! - **`evolve <n>`** — a *summoned, living* pet of the right species and level
//!   becomes its evolved form: the old collar is destroyed with its saved row,
//!   a new collar is added, and the new pet is summoned carrying the old one's
//!   experience and name.
//! - **`restore <n>`** — the reverse, and it works on an **item in the
//!   inventory** rather than a live pet: a seasonal/evolved collar is swapped
//!   back for its base form and the pet is summoned at the level the collar's
//!   enchant recorded.
//!
//! **Java's exp handling is the load-bearing part.** `doEvolve` carries the old
//! pet's exp across but floors it at the *new* species' exp-for-`petminLevel`
//! ("fix for non-linear baby pet exp"), so a level-55 Wolf becomes a Great Wolf
//! at level 55 rather than dropping to the new curve's level 1. `doRestore`
//! reads the level out of the **collar's enchant level** — which is where the
//! summon path stamps it — and floors that at `petminLevel` the same way.
//!
//! **Dist finding:** only `evolve` and `exchange` are reachable on this dist.
//! Their pages hang off Lundy (30827), who is spawned; `restore`'s page is
//! `36478.htm` and **npc 36478 has no spawn anywhere in `data/spawns`**. The
//! verb is ported anyway — it lives on the shared `PetManager` class, so any
//! pet manager would accept it, and leaving it out would be a silent gap if the
//! NPC is ever spawned.

use tracing::warn;

use super::helpers::send_to_client as send;
use crate::game_loop::helpers::item_id_of;
use crate::model::Player;
use crate::model::inventory::Inventory;
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

/// `MagicSkillUse(npc, 2046, 1, 1000, 600000)` — the summoning animation Java
/// plays from the *manager*, not the player.
const SUMMON_ANIMATION_SKILL: i32 = 2046;

/// `PetManager`'s `exchange` table: ticket item → collar item.
const EXCHANGE: [(i32, i32, i32); 3] = [(1, 7585, 6650), (2, 7583, 6648), (3, 7584, 6649)];

/// `PetManager`'s `evolve` table: `(button, take, give, min level)`.
const EVOLVE: [(i32, i32, i32, i32); 5] = [
    (1, 2375, 9882, 55),  // Wolf → Great Wolf
    (2, 9882, 10426, 70), // Great Wolf → Fenrir
    (3, 6648, 10311, 55), // Baby Buffalo → Improved
    (4, 6650, 10313, 55), // Baby Kookaburra → Improved
    (5, 6649, 10312, 55), // Baby Cougar → Improved
];

/// `PetManager`'s `restore` table, same shape.
const RESTORE: [(i32, i32, i32, i32); 5] = [
    (1, 10307, 9882, 55),  // Great Snow Wolf → Great Wolf
    (2, 10611, 10426, 70), // Snow Fenrir → Fenrir
    (3, 10308, 4422, 55),  // Red Wind Strider → Wind Strider
    (4, 10309, 4423, 55),  // Red Star Strider → Star Strider
    (5, 10310, 4424, 55),  // Red Twilight Strider → Twilight Strider
];

/// Serve one of the manager's refusal pages (`evolve_no.htm` /
/// `restore_no.htm` / `exchange_no.htm`) — Java's answer to every failed
/// attempt, with no system message of its own.
fn refuse(world: &mut World, client_id: u32, npc_object_id: i32, file: &str) {
    let html = crate::data::htm_cache::read_htm(format!("{}data/html/{file}", world.data.root))
        .map(|c| c.replace("%objectId%", &npc_object_id.to_string()))
        .unwrap_or_default();
    send(
        world,
        client_id,
        server_packets::npc_html_message(npc_object_id, &html),
    );
}

/// The numeric argument of `evolve 3` / `exchange 1` / `restore 2`. Java parses
/// it with `Integer.parseInt` and lets a bad value throw (the bypass dies); the
/// port drops the command with a log line instead.
fn button_of(command: &str) -> Option<i32> {
    command
        .split_whitespace()
        .nth(1)
        .and_then(|t| t.parse::<i32>().ok())
}

/// `PetManager.onBypassFeedback`'s `exchange` branch: destroy the ticket, add
/// the collar. Java's `exchange` helper takes exactly one ticket.
pub(crate) fn handle_exchange(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_object_id: i32,
    command: &str,
) {
    let Some(button) = button_of(command) else {
        warn!("PetManager: bad exchange command [{command}].");
        return;
    };
    let Some(&(_, ticket, collar)) = EXCHANGE.iter().find(|(b, ..)| *b == button) else {
        return; // Java's switch has no default — nothing happens.
    };
    let has_ticket = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .is_some_and(|inv| inv.count_of(ticket) >= 1);
    if !has_ticket {
        refuse(
            world,
            client_id,
            npc_object_id,
            "petmanager/exchange_no.htm",
        );
        return;
    }
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_oid) {
        inv.remove_item(ticket, 1);
    }
    let _ = super::items::add_inventory_item(world, player_oid, collar, 1);
    refresh_inventory(world, player_oid);
}

/// `Evolve.doEvolve`: the summoned pet becomes its evolved form.
pub(crate) fn handle_evolve(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_object_id: i32,
    command: &str,
) {
    let Some(button) = button_of(command) else {
        warn!("PetManager: bad evolve command [{command}].");
        return;
    };
    let Some(&(_, take, give, min_level)) = EVOLVE.iter().find(|(b, ..)| *b == button) else {
        return;
    };
    if !do_evolve(world, player_oid, npc_object_id, take, give, min_level) {
        refuse(world, client_id, npc_object_id, "petmanager/evolve_no.htm");
    }
}

/// `Evolve.doRestore`: an evolved/seasonal collar in the inventory is swapped
/// back for its base form and summoned.
pub(crate) fn handle_restore(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_object_id: i32,
    command: &str,
) {
    let Some(button) = button_of(command) else {
        warn!("PetManager: bad restore command [{command}].");
        return;
    };
    let Some(&(_, take, give, min_level)) = RESTORE.iter().find(|(b, ..)| *b == button) else {
        return;
    };
    if !do_restore(world, player_oid, npc_object_id, take, give, min_level) {
        refuse(world, client_id, npc_object_id, "petmanager/restore_no.htm");
    }
}

/// Port of `Evolve.doEvolve`. `false` on any refusal, which the caller turns
/// into `evolve_no.htm` — Java has no per-reason message here.
fn do_evolve(
    world: &mut World,
    player_oid: i32,
    npc_object_id: i32,
    take: i32,
    give: i32,
    min_level: i32,
) -> bool {
    // A pet must be **out**: evolution reads its live exp and species.
    let Some(pet_oid) = super::servitor::pet_of(world, player_oid) else {
        return false;
    };
    // Java `Evolve` calls a dead pet here an exploit attempt and punishes
    // with `DEFAULT_PUNISH`.
    if world
        .objects
        .get_component::<crate::model::components::Vitals>(&pet_oid)
        .is_some_and(|v| v.dead)
    {
        let punish = world.cfg.general.default_punish;
        super::punishment::handle_illegal_player_action(
            world,
            player_oid,
            &format!("Player {player_oid} tried to use death pet exploit!"),
            punish,
        );
        return false;
    }

    let (old_npc_id, old_level, old_exp, collar_object_id) = {
        let Some(pet) = world
            .objects
            .get_component::<crate::model::components::PetOf>(&pet_oid)
        else {
            return false;
        };
        let Some(npc) = world
            .objects
            .get_component::<crate::model::npc::Npc>(&pet_oid)
        else {
            return false;
        };
        (npc.npc_id, pet.level, pet.exp, pet.collar_object_id)
    };
    // The species must match the button's "from" collar, and the pet must be
    // high enough.
    let Some(from) = world.data.pet_data.by_item_id(take) else {
        return false;
    };
    if old_level < min_level || old_npc_id != from.npc_id {
        return false;
    }
    if world.data.pet_data.by_item_id(give).is_none() {
        return false;
    }
    // Java keeps the pet's *name* across the evolution (the html says so).
    let old_name = world
        .objects
        .get_component::<crate::model::components::PlayerPets>(&player_oid)
        .and_then(|p| p.0.get(&collar_object_id).map(|r| r.name.clone()))
        .unwrap_or_default();
    let old_pos = world
        .objects
        .get_component::<crate::model::components::Position>(&pet_oid)
        .copied();

    // `unSummon` then `destroyControlItem(player, true)`: the old collar and
    // its saved row both go, or the evolved pet would inherit the old one's
    // stored state on the next summon.
    super::servitor::unsummon_servitor(world, player_oid);
    destroy_collar(world, player_oid, collar_object_id);

    let Some(new_collar) = super::items::add_inventory_item(world, player_oid, give, 1)
        .and_then(|ids| ids.first().copied())
    else {
        return false;
    };
    // "Fix for non-linear baby pet exp": floor the carried exp at the *new*
    // species' exp for `min_level`, so a qualifying pet never lands below the
    // level it evolved at.
    let floor = world
        .data
        .pet_data
        .by_item_id(give)
        .map(|t| t.exp_for_level(min_level))
        .unwrap_or(0);
    summon_evolved(
        world,
        player_oid,
        npc_object_id,
        new_collar,
        old_exp.max(floor),
        (!old_name.is_empty()).then_some(old_name),
        old_pos,
    )
}

/// Port of `Evolve.doRestore`.
fn do_restore(
    world: &mut World,
    player_oid: i32,
    npc_object_id: i32,
    take: i32,
    give: i32,
    min_level: i32,
) -> bool {
    // Unlike evolve this works off an *item*, and Java does not require (or
    // check) a summoned pet — the collar carries everything it needs.
    let Some((collar_object_id, enchant)) = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.first_of_item(take)
                .map(|i| (i.object_id, i.enchant_level))
        })
    else {
        return false;
    };
    if world.data.pet_data.by_item_id(take).is_none()
        || world.data.pet_data.by_item_id(give).is_none()
    {
        return false;
    }
    // The collar's **enchant level is the pet's level** (the summon path stamps
    // it there), floored at `petminLevel`.
    let level = enchant.max(min_level);

    destroy_collar(world, player_oid, collar_object_id);
    super::clans::send_sm_with(
        world,
        player_oid,
        sm_ids::S1_DISAPPEARED,
        &[server_packets::SmParam::ItemName(take)],
    );
    let Some(new_collar) = super::items::add_inventory_item(world, player_oid, give, 1)
        .and_then(|ids| ids.first().copied())
    else {
        return false;
    };
    let exp = world
        .data
        .pet_data
        .by_item_id(give)
        .map(|t| t.exp_for_level(level))
        .unwrap_or(0);
    summon_evolved(
        world,
        player_oid,
        npc_object_id,
        new_collar,
        exp,
        None,
        None,
    )
}

/// Drop a collar and the pet row keyed to it (Java
/// `Pet.destroyControlItem(owner, evolve = true)` — the `true` is what also
/// deletes the `pets` row, so the new pet starts clean).
fn destroy_collar(world: &mut World, player_oid: i32, collar_object_id: i32) {
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_oid) {
        inv.remove_by_object_id(collar_object_id, 1);
    }
    world
        .objects
        .get_component_mut::<crate::model::components::PlayerPets>(&player_oid)
        .map(|p| p.0.remove(&collar_object_id));
    let _ = world
        .db
        .send(crate::db::DbCommand::DeletePetRow { collar_object_id });
    refresh_inventory(world, player_oid);
}

/// Resend the inventory window after a collar/ticket swap.
fn refresh_inventory(world: &World, player_oid: i32) {
    if let (Some(cid), Some(inv)) = (
        super::helpers::client_for_player(world, player_oid),
        world.objects.get_component::<Inventory>(&player_oid),
    ) {
        send(
            world,
            cid,
            crate::network::enter_world::item_list(inv, &world.data, false),
        );
    }
}

/// The shared tail of both flows: seed the saved row so the normal summon path
/// brings the new pet up at the carried experience, summon it, and play Java's
/// manager-cast animation.
///
/// Seeding the row rather than post-patching the live pet is what keeps this
/// consistent with every other summon: `summon_pet` reads the row for level,
/// exp, HP/MP and food, so the evolved pet is built exactly like a re-summoned
/// one instead of by a parallel construction that could drift.
fn summon_evolved(
    world: &mut World,
    player_oid: i32,
    npc_object_id: i32,
    collar_object_id: i32,
    exp: i64,
    name: Option<String>,
    at: Option<crate::model::components::Position>,
) -> bool {
    let level = level_for_exp(world, player_oid, collar_object_id, exp);
    if let Some(pets) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerPets>(&player_oid)
    {
        pets.0.insert(
            collar_object_id,
            crate::db::PetRow {
                collar_object_id,
                name: name.unwrap_or_default(),
                level,
                // Java sets the new pet to full HP/MP/food; `summon_pet` clamps
                // these to the recomputed maxima, so "very large" is simply
                // "full" without having to know the new species' stats here.
                cur_hp: f64::MAX,
                cur_mp: f64::MAX,
                exp,
                sp: 0,
                fed: i32::MAX,
                restore: true,
            },
        );
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.pending_pet_collar = Some(collar_object_id);
    }
    let Some(pet_oid) = super::servitor::summon_pet(world, player_oid) else {
        return false;
    };
    // `petSummon.spawnMe(oldX, oldY, oldZ)` — the new pet appears where the old
    // one stood, not beside the owner. The region index has to follow the move
    // or the pet would stay listed in the cell `spawn_npc_at` chose.
    if let Some(pos) = at {
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::components::Position>(&pet_oid)
        {
            *p = pos;
        }
        super::visibility::update_npc_region(world, pet_oid);
    }
    // `item.setEnchantLevel(petSummon.getLevel())` — the collar records the
    // level, which is what a later `restore` reads back out.
    super::servitor::sync_collar_enchant_for_admin(world, player_oid, pet_oid);

    // `MagicSkillUse(npc, 2046, …)` is cast **by the manager**, plus the
    // summoning system message.
    if let Some(npc_pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&npc_object_id)
        .copied()
    {
        let anim = server_packets::magic_skill_use_raw(
            (npc_object_id, npc_pos.x, npc_pos.y, npc_pos.z),
            (npc_object_id, npc_pos.x, npc_pos.y, npc_pos.z),
            SUMMON_ANIMATION_SKILL,
            1,
            1000,
        );
        if let Some(cid) = super::helpers::client_for_player(world, player_oid) {
            send(world, cid, anim);
        }
    }
    super::clans::send_sm_with(world, player_oid, sm_ids::SUMMONING_YOUR_PET, &[]);
    true
}

/// The level whose exp bracket `exp` falls in, for the species the collar
/// names. Without this the seeded row would say "level 1" and `summon_pet`
/// would floor the carried exp back down to it, losing the whole point of the
/// evolution.
fn level_for_exp(world: &World, player_oid: i32, collar_object_id: i32, exp: i64) -> i32 {
    let Some(item_id) = item_id_of(world, player_oid, collar_object_id) else {
        return 1;
    };
    let Some(t) = world.data.pet_data.by_item_id(item_id) else {
        return 1;
    };
    // The highest level in the species' own table whose exp floor `exp` still
    // covers (Java `PetStat.addExp` → the level the pet ends on).
    //
    // **Not a `while level_row(level + 1).is_some()` walk**: `level_row`
    // deliberately clamps *up* — past the top row it keeps returning the top —
    // while `exp_for_level` is an exact lookup returning 0 for a missing level,
    // so `0 <= exp` holds forever and such a loop never terminates. That hang
    // was real; it cost a test run.
    t.levels
        .iter()
        .filter(|(_, row)| row.exp <= exp)
        .map(|(&lvl, _)| lvl)
        .max()
        .unwrap_or_else(|| t.min_level())
        .max(t.min_level())
}
