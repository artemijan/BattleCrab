//! Death, decay/respawn, rewards (XP/SP/level-ups, drops), and the
//! die → "to village" → teleport → revive loop (G9).
//!
//! Java counterparts: `Creature.doDie`/`Npc.doDie`/`Player.doDie`,
//! `DecayTaskManager`/`RespawnTaskManager`/`Spawn.decreaseCount`,
//! `Attackable.calculateRewards` + `NpcTemplate.calculateDrops`,
//! `PlayerStat.addExpAndSp`/`addLevel`, `Player.calculateDeathExpPenalty`,
//! `RequestRestartPoint`/`Appearing`/`Player.doRevive`.
//!
//! This file keeps the NPC side of dying — `npc_do_die`, decay, respawn,
//! relocate. The rest is split by phase and re-exported, so callers keep
//! saying `death::…`:
//!
//! - `rewards` — `calculate_rewards`: XP/SP shares, drops, spoil, and the
//!   corpse item drop.
//! - `progression` — exp/SP award and loss, level changes, and the skill
//!   grants/removals that follow a level change.
//! - `player_death` — `player_do_die` and the death XP penalty.
//! - `restart` — the "to village" choice: die options, clan-hall and siege
//!   restart points, the teleport itself and its watchdog.
//! - `resurrect` — revive requests/answers, the restore percentages, pet
//!   revive, and raid points.

use crate::game_loop::guard::maybe_position;

use crate::game_loop::helpers::set_position_heading;

use crate::model::components::{Movement, RegionCell, Vitals};
use crate::network::server_packets::{self};
use crate::scheduler::ScheduledTask;

use crate::world::World;

use super::helpers::{broadcast_near_region_in, instance_of};
use crate::game_loop::helpers::npc_id_of;

use crate::game_loop::helpers::region_cell_of;

mod player_death;
mod progression;
mod restart;
mod resurrect;
mod rewards;

#[cfg(test)]
pub(crate) use player_death::apply_death_exp_penalty_ex;

#[cfg(test)]
pub(crate) use player_death::stop_effects_on_death_for_test;
pub(crate) use player_death::{apply_death_exp_penalty, is_lucky, player_do_die};

#[cfg(test)]
pub(crate) use progression::check_player_skills;
pub(crate) use progression::{
    add_exp_and_sp, cap_level, consume_kill_vitality, level_for_exp, maybe_skill_remove_on_delevel,
    overhit_bonus, remove_exp_and_sp, reward_skill_grants, reward_skills, set_level,
};
pub(crate) use restart::{
    TELEPORT_WATCHDOG_PERIOD, die_options, handle_appearing, handle_request_restart_point,
    teleport_player, teleport_player_scattered, teleport_to_object, teleport_to_town,
    teleport_watchdog_tick,
};
#[cfg(test)]
pub(crate) use resurrect::do_revive_with;
pub(crate) use resurrect::{award_raid_points, do_revive, handle_revive_answer, revive_request};
#[cfg(test)]
pub(crate) use rewards::{PremiumDropRate, premium_drop_mult};
#[cfg(test)]
pub(crate) use rewards::{
    auto_loots_for_test, chest_drop_template_for_test, roll_champion_drops_for_test,
    roll_drops_for_test, roll_spoil_drops_for_test,
};
pub(crate) use rewards::{calculate_rewards, give_item, on_die_drop_item};

/// `Inventory.ADENA_ID`.
pub(crate) use crate::data::item_data::ADENA_ID;

// ---------------------------------------------------------------------------
// NPC death → decay → respawn
// ---------------------------------------------------------------------------

/// `Npc/Attackable.doDie`: mark dead, hand out rewards, broadcast `Die`,
/// schedule the decay task.
pub(crate) fn npc_do_die(world: &mut World, npc_oid: i32, killer_oid: i32) {
    let (corpse_secs, max_hp) = {
        let Some((npc, mut vitals)) = world
            .objects
            .get_many_mut::<(&mut crate::model::npc::Npc, &mut Vitals)>(&npc_oid)
        else {
            return;
        };
        if vitals.dead {
            return;
        }
        vitals.dead = true;
        vitals.cur_hp = 0.0;
        let npc_id = npc.npc_id;
        // `DecayTaskManager.add`: a corpse someone spoiled or seeded lingers
        // `SpoiledCorpseExtendTime` seconds longer, so the sweeper/harvester
        // has time to walk to it. Read here, before the borrows drop.
        let extended = npc.spoiler_object_id != 0 || npc.seeded;
        let max_hp = vitals.max_hp;
        world.objects.remove_component::<Movement>(&npc_oid);
        let mut corpse_secs = world
            .data
            .npc_data
            .get(npc_id)
            .and_then(|t| t.corpse_time)
            .unwrap_or(world.cfg.npc.default_corpse_time);
        if extended {
            corpse_secs += world.cfg.npc.spoiled_corpse_extend_time;
        }
        (corpse_secs, max_hp)
    };
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    // Scope the death packets to the corpse's instance (G27).
    let instance = instance_of(world, npc_oid);

    // A grand boss dying: mark it dead, roll and persist its respawn window,
    // arm the timer. No-op for every other NPC.
    if let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    {
        let npc_id = npc.npc_id;
        super::grand_boss::on_grand_boss_killed(world, npc_id);
        // Core's script-spawned minions: respawn one, or clear them all when
        // Core itself falls.
        if npc_id == super::core_boss::CORE {
            super::core_boss::say_death_lines(world, npc_oid);
            super::core_boss::on_core_killed(world);
        } else if super::core_boss::is_core_minion(npc_id) {
            super::core_boss::on_minion_killed(world, npc_id);
        }
        // The Gigantic Chaos Golem carries no config window, so the shared
        // lifecycle no-ops for it — Dr. Chaos owns its death.
        if npc_id == super::dr_chaos::CHAOS_GOLEM {
            super::dr_chaos::on_golem_killed(world, npc_oid);
        }
        // Sailren's wave ladder — only *tagged* mobs advance it (the same
        // dinosaurs also roam the open world).
        if world
            .objects
            .has_component::<crate::model::components::SailrenWaveMob>(&npc_oid)
        {
            let killer = super::pvp::acting_player(world, killer_oid);
            super::sailren::on_wave_kill(world, killer, npc_id);
        }
        // Antharas's `onKill` tail: despawn the adds, drop the exit cube, and
        // arm the 15-minute lair clear (the respawn window is already set
        // above). Without it players are stranded in the lair after the kill.
        if npc_id == super::antharas::ANTHARAS {
            super::antharas::on_antharas_killed(world);
        }
        // Valakas's `onKill` tail: the death cinematic, the exit cubes, and the
        // 15-minute lair clear — the symmetric counterpart to Antharas's.
        if npc_id == super::valakas::VALAKAS {
            super::valakas::on_valakas_killed(world, npc_oid);
        }
    }

    // `Pet.doDie`: the exp penalty, the owner's warning and the state capture.
    // No-op for every NPC that is not a pet.
    super::servitor::pet_do_die(world, npc_oid);

    // `ControlTower.onDeath` → `Siege.killedCT`: a felled control tower weakens
    // the defenders (no-op for every other NPC).
    super::siege::killed_control_tower(world, npc_oid);
    // `SiegeFlag.doDie` → `Siege.killedFlag`: a destroyed HQ flag stops being an
    // attacker respawn point.
    super::siege::killed_siege_flag(world, npc_oid);

    calculate_rewards(world, npc_oid, killer_oid);

    // `Creature.doDie`'s "Clan help range aggro on kill": the dying monster
    // calls its faction onto the killer. Java's *other* faction call runs from
    // the AI think tick, so this is the only one that fires when a mob is
    // one-shot — without it `[G]` packs never retaliate for a mob that dropped
    // before it could think.
    super::ai::faction_call_on_kill(world, npc_oid, killer_oid);

    // `CursedWeaponsManager.checkDrop`: an ordinary monster slain by an
    // un-cursed player has a tiny chance to drop a cursed weapon.
    super::cursed_weapon::on_monster_killed(world, npc_oid, killer_oid);

    // `Attackable.doDie`'s minion notifications, in Java's order: tell this
    // NPC's leader it lost a minion, then (if it led a pack itself) clear its
    // own escort.
    super::minions::on_minion_die(world, npc_oid);
    super::minions::on_master_die(world, npc_oid);

    // `OnAttackableKill` listeners (Java fires them async off the death
    // path; here it's an ordinary call after rewards — same tick, no
    // component borrow held). Killer-only: party quest sharing is deferred.
    {
        let npc_id = npc_id_of(world, npc_oid);
        if let Some(npc_id) = npc_id {
            // Quest kill credit also follows the acting player: a pet's kill
            // has to advance its owner's quest.
            let quest_killer = crate::game_loop::pvp::acting_player(world, killer_oid);
            // Java's `isSummon`: the blow came from a pet/servitor, not the
            // player themselves.
            let is_summon = quest_killer != killer_oid;
            super::quests::notify_kill(world, quest_killer, npc_oid, npc_id, is_summon);
        }
    }

    // `Creature.doDie` → `stopMove(null)`: freeze the corpse at the death
    // spot on every client (Java broadcasts `StopMove` unconditionally, before
    // the StatusUpdate/Die below). A mob killed mid-chase otherwise keeps
    // sliding toward its last `MoveToPawn` destination client-side, since the
    // client never learns the movement ended.
    if let Some(pos) = maybe_position(world, npc_oid) {
        broadcast_near_region_in(
            world,
            region,
            instance,
            &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading),
        );
    }

    // `setCurrentHp(0)` broadcasts the final StatusUpdate before `Die` —
    // without it the target window keeps the last non-zero HP.
    broadcast_near_region_in(
        world,
        region,
        instance,
        &server_packets::status_update(
            npc_oid,
            &[
                (server_packets::status_update_type::MAX_HP, max_hp),
                (server_packets::status_update_type::CUR_HP, 0),
            ],
        ),
    );
    // `_isSweepable = isAttackable() && isSweepActive()` — a spoiled corpse
    // tells the client its loot can still be swept.
    let sweepable = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .is_some_and(|n| n.spoiler_object_id != 0);
    broadcast_near_region_in(
        world,
        region,
        instance,
        &server_packets::die(
            npc_oid,
            server_packets::DieOptions {
                sweepable,
                ..Default::default()
            },
        ),
    );

    // The mob stays *selected* while its corpse lasts — a player keeps it in
    // target so corpse actions (sweep/spoil, looting) can act on it. The
    // selection is dropped only when the corpse decays; see `handle_npc_decay`.
    let decay_at = world.tick + corpse_secs.max(0) as u64 * 10;
    if let Some(npc) = world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&npc_oid)
    {
        npc.decay_at_tick = decay_at;
    }
    world.scheduler.schedule(
        decay_at,
        ScheduledTask::NpcDecay {
            npc_object_id: npc_oid,
        },
    );
}

/// `DecayTaskManager` firing → `Npc.onDecay` + `Spawn.decreaseCount`: remove
/// the corpse from the world and schedule the respawn.
pub(crate) fn handle_npc_decay(world: &mut World, npc_oid: i32) {
    // A corpse revived in the meantime (admin `//res_monster`) is alive again;
    // its pending decay task is a no-op, mirroring Java `DecayTaskManager.cancel`
    // on revive.
    if world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_some_and(|v| !v.dead)
    {
        return;
    }
    // `Summon.onDecay` → `unSummon` + `Pet.deleteMe`: a pet's corpse decaying
    // **destroys the pet permanently**. Handled before the generic despawn
    // because it needs the pet's components, which drop with the entity.
    if world
        .objects
        .has_component::<crate::model::components::PetOf>(&npc_oid)
    {
        super::servitor::pet_decay(world, npc_oid);
    }

    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    // Gather the respawn bookkeeping before despawn (components drop with
    // the entity).
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .cloned()
    else {
        return;
    };
    // A `dbSave` boss's row is written from its *spawn* position, which the
    // despawn below drops along with the entity — so read it first.
    let db_saved = super::boss_respawn::is_db_saved(world, npc.spawn_ref);
    let corpse_pos = maybe_position(world, npc_oid);
    despawn_npc(world, npc_oid, region);

    // `Spawn.decreaseCount`: respawn only when the spawn line asked for it
    // (`_doRespawn = respawnMinDelay > 0`), with the ± random spread.
    if npc.respawn_secs > 0 {
        let mut min = (npc.respawn_secs - npc.respawn_random_secs).max(0);
        let mut max = npc.respawn_secs + npc.respawn_random_secs;
        // `DBSpawnManager.updateStatus` scales the window before rolling it:
        // `respawnMinDelay = getRespawnMinDelay() * RAID_MIN_RESPAWN_MULTIPLIER`
        // and the same for max. Only the DB-backed bosses go through that
        // manager — an ordinary spawn line's respawn is `Spawn.decreaseCount`,
        // which never sees the multipliers. Both are 1.0 on this dist.
        if db_saved {
            min = (f64::from(min) * world.cfg.npc.raid_min_respawn_multiplier) as i32;
            max = (f64::from(max) * world.cfg.npc.raid_max_respawn_multiplier) as i32;
            max = max.max(min);
        }
        let delay_secs = if max > min {
            min + world.roll(max - min + 1)
        } else {
            min
        };
        let (spawn_idx, group_idx, npc_idx) = npc.spawn_ref;
        world.scheduler.schedule(
            world.tick + delay_secs as u64 * 10,
            ScheduledTask::NpcRespawn {
                spawn_idx,
                group_idx,
                npc_idx,
            },
        );
        // `DBSpawnManager.updateStatus(npc, true)`: bank the absolute due time
        // so a restart inside the (up to 24 h + 12 h random) window resumes the
        // wait instead of handing the boss back immediately.
        if db_saved && let Some(pos) = corpse_pos {
            super::boss_respawn::persist_death_at(world, npc.npc_id, pos, delay_secs);
        }
    }
}

/// Remove an NPC from the world: despawn the entity, drop it from the region
/// index, broadcast `DeleteObject`, and clear it as a target for every player
/// still holding it (each gets its own `TargetUnselected` so the selection ring
/// clears — our client keeps a deleted target locked otherwise). Shared by
/// corpse decay and the admin `//delete` path.
pub(crate) fn despawn_npc(world: &mut World, npc_oid: i32, region: (i32, i32)) {
    // Read the instance before despawn drops the `InstanceId` component, or the
    // DeleteObject would fall back to the overworld and never reach the
    // instanced players who can see the NPC (G27).
    let instance = instance_of(world, npc_oid);
    let npc_id = npc_id_of(world, npc_oid);
    world.objects.despawn(&npc_oid);
    if let Some(npc_id) = npc_id
        && let Some(ids) = world.npcs_by_id.get_mut(&npc_id)
    {
        ids.retain(|&id| id != npc_oid);
    }
    if let Some(ids) = world.npc_regions.get_mut(&region) {
        ids.retain(|&id| id != npc_oid);
    }
    super::target::release_target_holders(world, npc_oid);
    broadcast_near_region_in(
        world,
        region,
        instance,
        &server_packets::delete_object(npc_oid),
    );
}

/// [`despawn_npc`] for callers that hold only the object id: look the region
/// cell up first, and do nothing if the NPC is already out of the world.
pub(crate) fn despawn_npc_by_oid(world: &mut World, npc_oid: i32) {
    if let Some(region) = region_cell_of(world, npc_oid) {
        despawn_npc(world, npc_oid, region);
    }
}

/// `RespawnTaskManager` firing → `Spawn.respawnNpc`: re-run the spawn line
/// and introduce the fresh NPC to nearby players.
pub(crate) fn handle_npc_respawn(
    world: &mut World,
    spawn_idx: usize,
    group_idx: usize,
    npc_idx: usize,
) {
    // A `dayTime`/`nightTime` mob killed near the end of its phase must not
    // climb back out during the other half of the day (Java's `despawnAll`
    // stops the spawn outright; here the scheduled task outlives the despawn).
    if !super::spawn_scripts::respawn_is_in_phase(world, spawn_idx, group_idx) {
        return;
    }
    let Some(object_id) = crate::model::npc::spawn_one(world, spawn_idx, group_idx, npc_idx) else {
        return;
    };
    introduce_npc(world, object_id);
}

/// Broadcast a freshly spawned NPC's `NpcInfo` to nearby players (Java
/// `Spawn.respawnNpc` → `npc.spawnMe()` visibility). Shared by respawn and the
/// admin `//spawn` path.
/// Move a live NPC to a new point, possibly across regions — Java
/// `Npc.teleToLocation`. Orfen's in-place `Position` mutation is safe only
/// within one region; this also re-indexes `npc_regions` and re-announces
/// (`DeleteObject` near the old region, `NpcInfo` near the new one), so a
/// cross-region teleport (Antharas entering his lair) neither ghosts nor
/// duplicates the NPC.
///
/// Java's `teleToLocation` runs `decayMe()` **before** the move, and
/// `World.removeVisibleObject` there clears the target of every creature in
/// the old 3×3 block that was holding this object (`setTarget(null)` →
/// `TargetUnselected` for players) and sends them all a `DeleteObject` —
/// unconditionally, not only when the region index changes. Both halves are
/// load-bearing on this client: without the `TargetUnselected` the ground
/// selection ring is left behind at the old spot (a leashed mob snapping home
/// used to strand one there, same failure family as [`despawn_npc`] and
/// `visibility.rs`), and without the delete/re-add pair the client can keep a
/// ghost of a same-region teleport.
pub(crate) fn relocate_npc(world: &mut World, npc_oid: i32, x: i32, y: i32, z: i32, heading: i32) {
    let Some(old_region) = region_cell_of(world, npc_oid) else {
        return;
    };
    let new_region = crate::world::region_of(x, y);
    // `decayMe()`: release every holder's selection, then un-spawn the NPC for
    // the players around its old position. NPCs hold no `TargetRef` here (an
    // NPC's "target" is its aggro list), so only players need the packet.
    super::target::release_target_holders(world, npc_oid);
    broadcast_near_region_in(
        world,
        old_region,
        instance_of(world, npc_oid),
        &server_packets::delete_object(npc_oid),
    );
    set_position_heading(world, npc_oid, (x, y, z), heading);
    if old_region != new_region {
        if let Some(ids) = world.npc_regions.get_mut(&old_region) {
            ids.retain(|&id| id != npc_oid);
        }
        world
            .npc_regions
            .entry(new_region)
            .or_default()
            .push(npc_oid);
        if let Some(r) = world.objects.get_component_mut::<RegionCell>(&npc_oid) {
            r.0 = new_region;
        }
    }
    introduce_npc(world, npc_oid);
}

pub(crate) fn introduce_npc(world: &mut World, object_id: i32) {
    let Some(v) = crate::model::npc::NpcView::of(&world.objects, object_id) else {
        return;
    };
    let Some(region) = region_cell_of(world, object_id) else {
        return;
    };
    let Some(t) = v.npc.template(world) else {
        return;
    };
    let visuals = super::abnormal::visual_effects(world, object_id);
    let clan = super::visibility::npc_clan_block(world, object_id);
    let pkt = server_packets::npc_info(&v, t, &world.cfg.npc, &world.cfg.champion, &visuals, clan);
    broadcast_near_region_in(world, region, instance_of(world, object_id), &pkt);
}
