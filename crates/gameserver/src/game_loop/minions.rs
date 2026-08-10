//! Minions — port of `util/MinionList`.
//!
//! 460 NPCs on this dist declare an escort (962 `<minions><npc>` entries).
//! The parser skipped them entirely, so every "boss with a pack" — raid bosses
//! and ordinary leaders alike — stood alone.
//!
//! Lifecycle:
//! - **Leader spawns** → [`spawn_minions`] tops the escort up to each entry's
//!   `count`, scattered in a ring around the leader.
//! - **Minion dies** → [`on_minion_die`] schedules its return, but only if the
//!   leader is still alive and the respawn delay resolves to something > 0.
//! - **Leader dies** → [`on_master_die`] despawns the surviving escort, but
//!   **only for a raid boss** (or when `ForceDeleteMinions` is set). An
//!   ordinary leader's minions outlive it — that's Java's default and it's
//!   deliberately not "tidied".
//! - **Either is attacked** → [`on_assist`] spreads aggro across the pack.

use crate::game_loop::guard::position;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::npc_template;
use commons::util::rnd;

use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::region_cell_of;
use crate::model::components::Vitals;
use crate::model::npc::{AggroInfo, AggroList, Npc, NpcAi, NpcIntention};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `MinionList.initializeNpc`: minions land in a ring around the leader,
/// `offset` out, no closer than the leader's collision radius + 30.
const SPAWN_OFFSET: i32 = 200;

/// Top the leader's escort up to the declared counts (`MinionList.spawnMinions`
/// — it spawns `count - alreadyAlive`, so it doubles as the respawn top-up).
/// Returns how many minions were actually placed, so callers that keep a
/// spawn tally (`spawn_all`) stay consistent with the world's NPC count.
///
/// Only the default `"Privates"` group spawns here — that is the group Java's
/// generic escort path asks for by name. Named groups (the Ragna Orc leaders'
/// `Privates1`/`2`/`3`) are chosen by their scripts via
/// [`spawn_minion_group`]; spawning them all would over-escort the leader.
pub(crate) fn spawn_minions(world: &mut World, master_oid: i32) -> usize {
    spawn_minion_group(world, master_oid, "Privates")
}

/// `spawnMinions(npc, "<group>")` — top up one named `<minions>` group.
pub(crate) fn spawn_minion_group(world: &mut World, master_oid: i32, group: &str) -> usize {
    if is_dead(world, master_oid) {
        return 0;
    }
    let Some(master_npc_id) = npc_id_of(world, master_oid) else {
        return 0;
    };
    let Some(entries) = world.data.npc_data.get(master_npc_id).map(|t| {
        t.minions
            .iter()
            .filter(|m| m.group == group)
            .cloned()
            .collect::<Vec<_>>()
    }) else {
        return 0;
    };
    if entries.is_empty() {
        return 0;
    }

    let mut placed = 0;
    for entry in entries {
        let alive = count_alive_minions(world, master_oid, entry.npc_id);
        for _ in alive..entry.count.max(0) {
            if spawn_one_minion(world, master_oid, entry.npc_id) {
                placed += 1;
            }
        }
    }
    // Every escort spawn lands in the boot tally here — including the ones a
    // leader's `onSpawn` script places (which `spawn_one`'s own call cannot
    // see); `spawn_all` folds and resets the counter once boot placement ends.
    world.minions_placed += placed;
    placed
}

/// `MinionList.addMinion(master, npcId)` — place **one** extra minion of the
/// given id beside its leader, ignoring the `<minions>` count cap. The
/// on-attack "call for help" scripts (Timak Orc Troop Leader) summon one at a
/// time instead of topping a whole group up.
pub(crate) fn add_minion(world: &mut World, master_oid: i32, npc_id: i32) -> bool {
    if is_dead(world, master_oid) {
        return false;
    }
    spawn_one_minion(world, master_oid, npc_id)
}

/// `MinionList.countSpawnedMinions()` — how much of the leader's escort is
/// alive right now.
pub(crate) fn count_spawned_minions(world: &World, master_oid: i32) -> usize {
    live_pack(world, master_oid).len()
}

/// Whether a minion of `npc_id` is currently out with this leader — Java's
/// `for (Monster minion : getSpawnedMinions()) if (minion.getId() == id)` scan.
pub(crate) fn minion_of_id_alive(world: &World, master_oid: i32, npc_id: i32) -> bool {
    count_alive_minions(world, master_oid, npc_id) > 0
}

/// `MinionList.countSpawnedMinionsById`, over the leader's own roster.
///
/// **Not** a world scan: this runs once per minion spawned, and a full
/// `for_each` over ~39k NPCs here made boot take long enough that the game
/// server missed its login-server registration window. Java keeps the same
/// per-master `_spawnedMinions` list for the same reason.
fn count_alive_minions(world: &World, master_oid: i32, npc_id: i32) -> i32 {
    let Some(roster) = world.objects.get_component::<Minions>(&master_oid) else {
        return 0;
    };
    roster
        .0
        .iter()
        .filter(|&&oid| {
            world
                .objects
                .get_component::<Npc>(&oid)
                .is_some_and(|n| n.npc_id == npc_id)
                && world
                    .objects
                    .get_component::<Vitals>(&oid)
                    .is_some_and(|v| !v.dead)
        })
        .count() as i32
}

/// Live members of a leader's escort, from its roster.
pub(crate) fn live_pack(world: &World, master_oid: i32) -> Vec<i32> {
    let Some(roster) = world.objects.get_component::<Minions>(&master_oid) else {
        return Vec::new();
    };
    roster
        .0
        .iter()
        .copied()
        .filter(|oid| {
            world
                .objects
                .get_component::<Vitals>(oid)
                .is_some_and(|v| !v.dead)
        })
        .collect()
}

/// `MinionList.initializeNpc`'s placement, transcribed. The odd
/// `sqrt(newY² - newX²)` shape is Java's own — it biases minions into a band
/// around the leader rather than a uniform disc, and is kept as-is so packs
/// sit the way retail players expect.
fn spawn_one_minion(world: &mut World, master_oid: i32, minion_npc_id: i32) -> bool {
    let Some(master_pos) = position(world, master_oid) else {
        return false;
    };
    let min_radius = npc_template(world, master_oid)
        .map(|t| t.collision_radius as i32 + 30)
        .unwrap_or(30);

    let mut new_x = rnd::get_range(min_radius * 2, (SPAWN_OFFSET * 2).max(min_radius * 2 + 1));
    let mut new_y = rnd::get_range(new_x, (SPAWN_OFFSET * 2).max(new_x + 1));
    new_y = (((new_y * new_y) - (new_x * new_x)) as f64).sqrt() as i32;

    new_x = if new_x > (SPAWN_OFFSET + min_radius) {
        master_pos.x + new_x - SPAWN_OFFSET
    } else {
        master_pos.x - new_x + min_radius
    };
    new_y = if new_y > (SPAWN_OFFSET + min_radius) {
        master_pos.y + new_y - SPAWN_OFFSET
    } else {
        master_pos.y - new_y + min_radius
    };

    match crate::model::npc::spawn_minion_npc_at(
        world,
        minion_npc_id,
        new_x,
        new_y,
        master_pos.z,
        master_pos.heading,
    ) {
        Some(oid) => {
            world.objects.add_components(&oid, MinionOf(master_oid));
            clear_champion_for_raid_minion(world, master_oid, oid);
            // Keep the leader's roster in step (Java `onMinionSpawn`).
            match world.objects.get_component_mut::<Minions>(&master_oid) {
                Some(roster) => roster.0.push(oid),
                None => world
                    .objects
                    .add_components(&master_oid, Minions(vec![oid])),
            }
            true
        }
        None => false,
    }
}

/// Java `MinionList.initializeNpcInstance` sets `minion.setIsRaidMinion(true)`
/// for a raid boss's minions, and `Attackable.onRespawn`'s champion guard
/// chain then rejects on `!_isRaidMinion`. The port has no raid-minion flag —
/// the relationship only exists as the `MinionOf` component written just
/// above — so the roll is undone here instead. Nothing has observed the minion
/// between the two points (it has not been introduced to any client yet), so
/// this is the same outcome, including the team aura the roll would have set.
fn clear_champion_for_raid_minion(world: &mut World, master_oid: i32, minion_oid: i32) {
    let master_is_raid = npc_template(world, master_oid).is_some_and(|t| t.is_raid());
    if !master_is_raid {
        return;
    }
    let Some(npc_id) = world
        .objects
        .get_component_mut::<Npc>(&minion_oid)
        .map(|npc| {
            npc.champion = false;
            npc.npc_id
        })
    else {
        return;
    };
    // The spawn already finalized the stats *with* the champion multipliers,
    // so undoing the flag alone would leave a raid minion swinging at
    // `ChampionAtk`× — recompute off the plain template. A just-spawned minion
    // carries no buffs, so this lands back on the template values exactly.
    let Some(t) = world.data.npc_data.get(npc_id).cloned() else {
        return;
    };
    if let Some((buffs, mut combat, mut speeds, mut vitals)) = world.objects.get_many_mut::<(
        &crate::model::components::Buffs,
        &mut crate::model::components::CombatStats,
        &mut crate::model::components::Speeds,
        &mut Vitals,
    )>(&minion_oid)
    {
        crate::model::recompute_npc_stats_from_buffs(
            &world.data,
            &t,
            buffs,
            crate::model::ChampionStatMods::default(),
            &mut combat,
            &mut speeds,
            &mut vitals,
        );
    }
}

/// `Attackable.doDie` → `MinionList.onMinionDie`. The delay is Java's ladder:
/// a per-npc `CustomMinionsRespawnTime` override wins outright (so an explicit
/// `0` means "gone for good" even on a raid), otherwise only a **raid** leader
/// brings its escort back.
pub(crate) fn on_minion_die(world: &mut World, minion_oid: i32) {
    let Some(master_oid) = world
        .objects
        .get_component::<MinionOf>(&minion_oid)
        .map(|m| m.0)
    else {
        return;
    };
    let Some(minion_npc_id) = npc_id_of(world, minion_oid) else {
        return;
    };

    // A dead (or vanished) leader doesn't rebuild its pack.
    if is_dead(world, master_oid) {
        return;
    }
    let master_is_raid = npc_template(world, master_oid)
        .is_some_and(|t| matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss"));

    let delay_ms = match world
        .cfg
        .npc
        .custom_minions_respawn_time
        .get(&minion_npc_id)
    {
        Some(&secs) => secs * 1000,
        None if master_is_raid => world.cfg.npc.raid_minion_respawn_time,
        None => 0,
    };
    if delay_ms <= 0 {
        return;
    }

    world.scheduler.schedule(
        world.tick + (delay_ms as u64 / 100).max(1),
        ScheduledTask::MinionRespawn {
            master_object_id: master_oid,
            minion_npc_id,
        },
    );
}

/// `ScheduledTask::MinionRespawn` — one minion's return. Routed through
/// [`spawn_minions`] so it re-counts first: if the leader died, was despawned,
/// or the pack refilled some other way, nothing is over-spawned.
pub(crate) fn handle_minion_respawn(world: &mut World, master_object_id: i32, _minion_npc_id: i32) {
    let _ = spawn_minions(world, master_object_id);
}

/// `MinionList.onMasterDie`: a **raid** leader takes its escort with it
/// (or any leader when `ForceDeleteMinions` is on). An ordinary leader's
/// minions are left alive — Java's default, and the reason a mob camp doesn't
/// evaporate when you kill the biggest one in it.
pub(crate) fn on_master_die(world: &mut World, master_oid: i32) {
    let is_raid = npc_template(world, master_oid)
        .is_some_and(|t| matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss"));
    if !is_raid && !world.cfg.npc.force_delete_minions {
        return;
    }

    for oid in live_pack(world, master_oid) {
        if let Some(region) = region_cell_of(world, oid) {
            super::death::despawn_npc(world, oid, region);
        }
    }
}

/// `MinionList.onAssist`: attacking any member of a pack pulls the rest in.
/// The leader takes 1 hate; the pack takes 10 when the *leader* was the one
/// attacked (versus 1 for a minion), ×10 again for a raid — so hitting the
/// boss aggros its escort far harder than hitting one minion does.
pub(crate) fn on_assist(world: &mut World, victim_oid: i32, attacker_oid: i32) {
    // Resolve the pack's leader from whichever member was hit.
    let (master_oid, caller_is_master) = match world.objects.get_component::<MinionOf>(&victim_oid)
    {
        Some(m) => (m.0, false),
        None => (victim_oid, true),
    };
    // Only packs participate.
    if !caller_is_master && world.objects.get_component::<Vitals>(&master_oid).is_none() {
        return;
    }

    let master_is_raid = npc_template(world, master_oid)
        .is_some_and(|t| matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss"));

    // The leader wakes with 1 hate, unless it's already fighting.
    let master_alive = world
        .objects
        .get_component::<Vitals>(&master_oid)
        .is_some_and(|v| !v.dead);
    let master_engaged = world
        .objects
        .get_component::<NpcAi>(&master_oid)
        .is_some_and(|ai| ai.intention == NpcIntention::Attack);
    if master_alive && !master_engaged {
        add_hate(world, master_oid, attacker_oid, 1.0);
    }

    let aggro =
        (if caller_is_master { 10.0 } else { 1.0 }) * if master_is_raid { 10.0 } else { 1.0 };
    for oid in live_pack(world, master_oid) {
        // A minion already in a fight of its own is left alone unless the
        // leader itself was the one struck.
        let engaged = world
            .objects
            .get_component::<NpcAi>(&oid)
            .is_some_and(|ai| ai.intention == NpcIntention::Attack);
        if caller_is_master || !engaged {
            add_hate(world, oid, attacker_oid, aggro);
        }
    }
}

/// `addDamageHate(attacker, 0, n)` — hate without damage, plus the AI wake.
pub(crate) fn add_hate(world: &mut World, npc_oid: i32, attacker_oid: i32, hate: f64) {
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        let entry = aggro.0.entry(attacker_oid).or_insert(AggroInfo {
            hate: 0.0,
            damage: 0.0,
        });
        entry.hate += hate;
    }
    // Java's `MinionList.onAssist` only seeds hate; the minion's own
    // `thinkActive` then promotes it to `AI_INTENTION_ATTACK` **and** calls
    // `setRunning()` in the same breath. This helper short-circuits straight to
    // the attack intention, so it has to do the run flip `thinkActive` would
    // otherwise have done — without it an assisting minion walks to the fight.
    if world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .is_some_and(|ai| ai.intention != NpcIntention::Attack)
    {
        super::ai::set_running(world, npc_oid);
        if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
            ai.intention = NpcIntention::Attack;
            ai.attack_timeout_tick = world.tick + super::combat::ATTACK_TIMEOUT_TICKS;
        }
    }
}

/// Marks an NPC as a member of a leader's escort (Java `Monster._master` /
/// `setLeader`). Held on the minion, pointing at the leader's object id.
#[derive(Debug, Clone, Copy, bevy_ecs::component::Component)]
pub struct MinionOf(pub i32);

/// The leader's side of the link (Java `MinionList._spawnedMinions`): the
/// object ids it has spawned. Dead entries are tolerated and filtered on read,
/// so nothing has to prune it from the death path.
#[derive(Debug, Clone, bevy_ecs::component::Component)]
pub struct Minions(pub Vec<i32>);

/// Java `Attackable.isRaidMinion()` — set by `Monster.onSpawn` as
/// `setIsRaidMinion(_master.isRaid())`, so it is simply "my master is a raid".
///
/// The port has no separate minion NPC type; the link is the `MinionOf`
/// component written when the master spawns its group, which is why this is a
/// lookup rather than a template flag.
pub(crate) fn is_raid_minion(world: &World, npc_oid: i32) -> bool {
    let Some(MinionOf(master)) = world.objects.get_component::<MinionOf>(&npc_oid).copied() else {
        return false;
    };
    world
        .objects
        .get_component::<Npc>(&master)
        .and_then(|n| world.data.npc_data.get(n.npc_id))
        .is_some_and(|t| t.is_raid())
}
