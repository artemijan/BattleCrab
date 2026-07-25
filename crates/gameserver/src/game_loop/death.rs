//! Death, decay/respawn, rewards (XP/SP/level-ups, drops), and the
//! die → "to village" → teleport → revive loop (G9).
//!
//! Java counterparts: `Creature.doDie`/`Npc.doDie`/`Player.doDie`,
//! `DecayTaskManager`/`RespawnTaskManager`/`Spawn.decreaseCount`,
//! `Attackable.calculateRewards` + `NpcTemplate.calculateDrops`,
//! `PlayerStat.addExpAndSp`/`addLevel`, `Player.calculateDeathExpPenalty`,
//! `RequestRestartPoint`/`Appearing`/`Player.doRevive`.

use crate::data::npc_data::{DropHolder, NpcTemplate};
use crate::model::components::{
    BaseStats, Buffs, CombatStats, Intent, Movement, PlayerVitals, Position, RegionCell, SkillBook,
    Speeds, StatModifiers, Vitals,
};
use crate::model::formulas;
use crate::model::inventory::Inventory;
use crate::model::Player;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::{regions_adjacent, World};

use super::helpers::{
    broadcast_including_self, broadcast_near_region_in, client_for_player, instance_of,
};

/// `Inventory.ADENA_ID`.
pub(crate) const ADENA_ID: i32 = 57;

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
        let max_hp = vitals.max_hp;
        drop((npc, vitals));
        world.objects.remove_component::<Movement>(&npc_oid);
        let corpse_secs = world
            .data
            .npc_data
            .get(npc_id)
            .and_then(|t| t.corpse_time)
            .unwrap_or(world.cfg.npc.default_corpse_time);
        (corpse_secs, max_hp)
    };
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
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
        let npc_id = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .map(|n| n.npc_id);
        if let Some(npc_id) = npc_id {
            // Quest kill credit also follows the acting player: a pet's kill
            // has to advance its owner's quest.
            let quest_killer = crate::game_loop::pvp::acting_player(world, killer_oid);
            super::quests::notify_kill(world, quest_killer, npc_oid, npc_id);
        }
    }

    // `Creature.doDie` → `stopMove(null)`: freeze the corpse at the death
    // spot on every client (Java broadcasts `StopMove` unconditionally, before
    // the StatusUpdate/Die below). A mob killed mid-chase otherwise keeps
    // sliding toward its last `MoveToPawn` destination client-side, since the
    // client never learns the movement ended.
    if let Some(pos) = world.objects.get_component::<Position>(&npc_oid).copied() {
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
    broadcast_near_region_in(
        world,
        region,
        instance,
        &server_packets::die(npc_oid, false),
    );

    // The mob stays *selected* while its corpse lasts — a player keeps it in
    // target so corpse actions (sweep/spoil, looting) can act on it. The
    // selection is dropped only when the corpse decays; see `handle_npc_decay`.
    world.scheduler.schedule(
        world.tick + corpse_secs.max(0) as u64 * 10,
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

    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
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
    let corpse_pos = world.objects.get_component::<Position>(&npc_oid).copied();
    despawn_npc(world, npc_oid, region);

    // `Spawn.decreaseCount`: respawn only when the spawn line asked for it
    // (`_doRespawn = respawnMinDelay > 0`), with the ± random spread.
    if npc.respawn_secs > 0 {
        let min = (npc.respawn_secs - npc.respawn_random_secs).max(0);
        let max = npc.respawn_secs + npc.respawn_random_secs;
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
        if db_saved {
            if let Some(pos) = corpse_pos {
                super::boss_respawn::persist_death_at(world, npc.npc_id, pos, delay_secs);
            }
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
    world.objects.despawn(&npc_oid);
    if let Some(ids) = world.npc_regions.get_mut(&region) {
        ids.retain(|&id| id != npc_oid);
    }
    broadcast_near_region_in(
        world,
        region,
        instance,
        &server_packets::delete_object(npc_oid),
    );

    let mut watchers: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &crate::model::components::TargetRef)>(|(p, t)| {
            if t.0 == Some(npc_oid) {
                watchers.push(p.object_id);
            }
        });
    for watcher_oid in watchers {
        if let Some(t) = world
            .objects
            .get_component_mut::<crate::model::components::TargetRef>(&watcher_oid)
        {
            t.0 = None;
        }
        if let (Some(client_id), Some(pos)) = (
            client_for_player(world, watcher_oid),
            world
                .objects
                .get_component::<Position>(&watcher_oid)
                .copied(),
        ) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::target_unselected(
                    watcher_oid,
                    pos.x,
                    pos.y,
                    pos.z,
                ));
            }
        }
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
pub(crate) fn relocate_npc(world: &mut World, npc_oid: i32, x: i32, y: i32, z: i32, heading: i32) {
    let Some(old_region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
        return;
    };
    let new_region = crate::world::region_of(x, y);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&npc_oid)
    {
        p.x = x;
        p.y = y;
        p.z = z;
        p.heading = heading;
    }
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
        broadcast_near_region_in(
            world,
            old_region,
            instance_of(world, npc_oid),
            &server_packets::delete_object(npc_oid),
        );
    }
    introduce_npc(world, npc_oid);
}

pub(crate) fn introduce_npc(world: &mut World, object_id: i32) {
    let Some(v) = crate::model::npc::NpcView::of(&world.objects, object_id) else {
        return;
    };
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&object_id)
        .map(|r| r.0)
    else {
        return;
    };
    let Some(t) = v.npc.template(world) else {
        return;
    };
    let pkt = server_packets::npc_info(&v, t, &world.cfg.npc);
    broadcast_near_region_in(world, region, instance_of(world, object_id), &pkt);
}

// ---------------------------------------------------------------------------
// Rewards (`Attackable.calculateRewards`)
// ---------------------------------------------------------------------------

/// XP/SP shares from the aggro list + drops to the top damage dealer.
/// Party members pool shares and split via `Party.distributeXpAndSp` (G10).
/// Narrowings: no overhit bonus (no overhit skills), no raid points, no
/// champion mods, no command channels.
fn calculate_rewards(world: &mut World, npc_oid: i32, killer_oid: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    let Some(t) = npc.template(world).cloned() else {
        return;
    };
    let Some(npc_region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
        return;
    };
    let (nx, ny, nz) = {
        let Some(pos) = world.objects.get_component::<Position>(&npc_oid) else {
            return;
        };
        (pos.x, pos.y, pos.z)
    };

    // Damage shares (players only, > 1 damage, still within reward range —
    // `Util.checkIfInRange(ALT_PARTY_RANGE, …)`).
    let reward_range = world.cfg.character.alt_party_range as f64;
    let mut shares: Vec<(i32, f64)> = Vec::new();
    let mut total_damage = 0.0;
    let mut max_dealer: Option<(i32, f64)> = None;
    // Java `calculateRewards`: `info.getAttacker().getActingPlayer()` — every
    // damage dealer is resolved to the player behind it, so a **summon's**
    // damage counts for its owner. Without this a summoner's pet did all the
    // work and the owner earned nothing.
    //
    // Collected first because resolving borrows the store while the aggro list
    // is still borrowed.
    let entries: Vec<(i32, f64)> = {
        let Some(aggro) = world
            .objects
            .get_component::<crate::model::npc::AggroList>(&npc_oid)
        else {
            return;
        };
        aggro
            .0
            .iter()
            .map(|(&oid, info)| (oid, info.damage))
            .collect()
    };
    for (dealer_oid, damage) in entries {
        let player_oid = crate::game_loop::pvp::acting_player(world, dealer_oid);
        // Only players earn. A summon resolves to its owner; a mob to itself,
        // which is not a player and so is skipped as before.
        if !world
            .objects
            .has_component::<crate::model::Player>(&player_oid)
        {
            continue;
        }
        // Range is measured from the **earner**, as Java does — a pet fighting
        // out of its owner's reward range earns them nothing.
        let Some(ppos) = world.objects.get_component::<Position>(&player_oid) else {
            continue;
        };
        if damage <= 1.0 {
            continue;
        }
        let dist = (((ppos.x - nx) as f64).powi(2) + ((ppos.y - ny) as f64).powi(2)).sqrt();
        if dist > reward_range {
            continue;
        }
        total_damage += damage;
        // A player who both hit the mob and had a pet hit it appears twice;
        // merge rather than double-counting them in the share split.
        if let Some(existing) = shares.iter_mut().find(|(id, _)| *id == player_oid) {
            existing.1 += damage;
        } else {
            shares.push((player_oid, damage));
        }
        let merged = shares
            .iter()
            .find(|(id, _)| *id == player_oid)
            .map(|(_, d)| *d)
            .unwrap_or(damage);
        if max_dealer.is_none_or(|(_, d)| merged > d) {
            max_dealer = Some((player_oid, merged));
        }
    }

    // Drops go to the top damage dealer (fall back to the killer); a looter
    // in a party routes every item through `Party.distributeItem`
    // (`Player.doAutoLoot`).
    // The killer fallback resolves too: a pet's killing blow loots for its
    // owner.
    let killer_oid = crate::game_loop::pvp::acting_player(world, killer_oid);

    // Raid points go to the same player the drops do — Java's
    // `maxDealer != null && isOnline() ? maxDealer : lastAttacker`.
    if let Some(earner) = max_dealer.map(|(id, _)| id).or(Some(killer_oid)) {
        award_raid_points(world, npc_oid, earner);
    }
    let looter = max_dealer.map(|(id, _)| id).or_else(|| {
        world
            .objects
            .has_component::<crate::model::Player>(&killer_oid)
            .then_some(killer_oid)
    });
    if let Some(looter) = looter {
        // `doItemDrop`: a mob that died spoiled rolls its `<spoil>` list into
        // the sweep loot (`DropType.SPOIL`), stashed on the corpse until a
        // `Sweeper` cast claims it. The level-gap gate uses the same main
        // damage dealer as the death drops (Java passes the same `player`).
        let spoiled = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .map(|n| n.spoiler_object_id != 0)
            .unwrap_or(false);
        if spoiled {
            let sweep = roll_spoil_drops(world, &t, looter);
            if let Some(npc) = world
                .objects
                .get_component_mut::<crate::model::npc::Npc>(&npc_oid)
            {
                // `_sweepItems.set(...)`: `null`/empty → `isSweepActive()` false.
                npc.sweep_items = if sweep.is_empty() { None } else { Some(sweep) };
            }
        }

        let drops = roll_drops(world, &t, looter);
        let party_id = world
            .objects
            .get_component::<crate::model::components::PartyRef>(&looter)
            .map(|r| r.0);
        let auto_loot = world.cfg.character.auto_loot;
        for (item_id, count) in drops {
            if !auto_loot {
                // Drop onto the ground for anyone to pick up (Java's owner-based
                // loot-window protection is simplified away).
                super::ground_items::spawn_ground_item(
                    world,
                    item_id,
                    count,
                    0,
                    nx,
                    ny,
                    nz,
                    npc_oid,
                    super::ground_items::DropSource::Npc,
                );
                continue;
            }
            match party_id {
                Some(pid) => {
                    super::party::distribute_item(world, pid, looter, item_id, count, (nx, ny))
                }
                None => give_item(world, looter, item_id, count),
            }
        }
    }

    if total_damage <= 0.0 {
        return;
    }
    // `calculateExpAndSp` per attacker: template reward × rate × damage
    // share × level-gap multiplier. Attackers in a party pool their shares
    // once (the Java party branch); the rest reward solo.
    let (rate_xp, rate_sp) = (world.cfg.rates.rate_xp, world.cfg.rates.rate_sp);
    let mut processed: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for &(player_oid, damage) in &shares {
        if processed.contains(&player_oid) {
            continue;
        }
        let party_id = world
            .objects
            .get_component::<crate::model::components::PartyRef>(&player_oid)
            .map(|r| r.0);
        let Some(party_id) = party_id else {
            // Solo branch (unchanged from G9).
            let Some(p) = world
                .objects
                .get_component::<crate::model::Player>(&player_oid)
            else {
                continue;
            };
            let Some(pregion) = world
                .objects
                .get_component::<RegionCell>(&player_oid)
                .map(|r| r.0)
            else {
                continue;
            };
            if !regions_adjacent(npc_region, pregion) {
                continue; // Java `isInSurroundingRegion(attacker)`.
            }
            let player_level = p.level;
            let gap = formulas::exp_sp_level_gap_multiplier(player_level, t.level);
            let mut exp = (t.exp * rate_xp * damage / total_damage * gap).max(0.0);
            // `Attackable.onKill`: the over-hit bonus rides on this attacker's
            // share, but only for whoever actually landed the killing blow.
            exp += overhit_bonus(world, npc_oid, player_oid, exp);
            let mut sp = (t.sp * rate_sp * damage / total_damage * gap).max(0.0);
            // `Attackable.onKill`: premium rates apply *before* the vitality /
            // skill bonus multiplier `addExpAndSp` folds in.
            if super::admin::premium::has_premium_status(world, player_oid) {
                exp *= world.cfg.premium.rate_xp;
                sp *= world.cfg.premium.rate_sp;
            }
            add_exp_and_sp(world, player_oid, exp, sp, true);
            // Java consumes vitality only when `exp > 0`, and keys the amount on
            // the *pre-bonus* exp — the same value it just handed to
            // `addExpAndSp`.
            if exp > 0.0 {
                consume_kill_vitality(world, player_oid, player_level, &t, exp);
            }
            continue;
        };

        // Party branch: pool every member's share; alive members within
        // `ALT_PARTY_RANGE` of the corpse are rewarded, the top rewarded
        // level keys the level-gap multiplier and the cutoff.
        let members = world
            .parties
            .get(&party_id)
            .map(|p| p.members.clone())
            .unwrap_or_default();
        let share_of: std::collections::HashMap<i32, f64> = shares.iter().copied().collect();
        let mut party_dmg = 0.0;
        let mut rewarded: Vec<(i32, i32)> = Vec::new();
        let mut party_lvl = 0;
        for &m in &members {
            let dead = world
                .objects
                .get_component::<Vitals>(&m)
                .map(|v| v.dead)
                .unwrap_or(true);
            if dead {
                continue; // their leftover share rewards nothing (Java parity)
            }
            let in_range = world
                .objects
                .get_component::<Position>(&m)
                .is_some_and(|p| {
                    let (dx, dy) = ((p.x - nx) as f64, (p.y - ny) as f64);
                    (dx * dx + dy * dy).sqrt() <= reward_range
                });
            if !in_range {
                continue;
            }
            if let Some(&share) = share_of.get(&m) {
                party_dmg += share;
                processed.insert(m);
            }
            rewarded.push((
                m,
                world
                    .objects
                    .get_component::<crate::model::Player>(&m)
                    .map(|p| p.level)
                    .unwrap_or(0),
            ));
            party_lvl = party_lvl.max(rewarded.last().unwrap().1);
        }
        processed.insert(player_oid);
        if party_dmg <= 0.0 || rewarded.is_empty() {
            continue;
        }
        // `calculateExpAndSp(partyLvl, partyDmg, totalDamage)` then
        // `exp *= partyMul` — Java applies the damage fraction twice when
        // outsiders contributed; kept for parity.
        let party_mul = if party_dmg < total_damage {
            party_dmg / total_damage
        } else {
            1.0
        };
        let gap = formulas::exp_sp_level_gap_multiplier(party_lvl, t.level);
        let exp = (t.exp * rate_xp * party_dmg / total_damage * gap).max(0.0) * party_mul;
        let sp = (t.sp * rate_sp * party_dmg / total_damage * gap).max(0.0) * party_mul;
        super::party::distribute_xp_and_sp(world, &rewarded, party_lvl, exp, sp, &t);
    }
}

/// `NpcTemplate.calculateDrops(DROP)` — grouped + ungrouped lists with the
/// level-gap gates, per-item rate multipliers, and the max-occurrence cap.
/// The temporary "replace highest-chance random drop" reshuffling Java does
/// when the cap is hit mid-list is simplified to a hard stop at the cap.
fn roll_drops(world: &mut World, t: &NpcTemplate, killer_oid: i32) -> Vec<(i32, i64)> {
    let Some(killer) = world
        .objects
        .get_component::<crate::model::Player>(&killer_oid)
    else {
        return Vec::new();
    };
    let level_diff = (t.level - killer.level) as f64;
    let r = &world.cfg.rates;
    let adena_gap_chance = formulas::map_range(
        level_diff,
        -(r.drop_adena_max_level_difference as f64),
        -(r.drop_adena_min_level_difference as f64),
        r.drop_adena_min_level_gap_chance,
        100.0,
    );
    let item_gap_chance = formulas::map_range(
        level_diff,
        -(r.drop_item_max_level_difference as f64),
        -(r.drop_item_min_level_difference as f64),
        r.drop_item_min_level_gap_chance,
        100.0,
    );
    let mut occurrences = r.drop_max_occurrences_normal;
    let chance_mult = r.death_drop_chance_multiplier;
    let amount_mult = r.death_drop_amount_multiplier;
    let by_id_chance = r.drop_chance_by_id.clone();
    let by_id_amount = r.drop_amount_by_id.clone();

    let mut out = Vec::new();
    let mut lists: Vec<(f64, &[DropHolder])> = Vec::new();
    for group in &t.drop_groups {
        lists.push((group.chance, &group.items));
    }
    if !t.drop_list_death.is_empty() {
        lists.push((100.0, &t.drop_list_death));
    }
    for (group_chance, items) in lists {
        for drop in items {
            if occurrences == 0 && drop.chance < 100.0 {
                continue;
            }
            // Level-gap gate.
            let gap = if drop.item_id == ADENA_ID {
                adena_gap_chance
            } else {
                item_gap_chance
            };
            if world.roll_f64() * 100.0 > gap {
                continue;
            }
            // Chance roll (grouped items fold the group chance in).
            let rate_chance = by_id_chance
                .get(&drop.item_id)
                .copied()
                .unwrap_or(chance_mult);
            let chance = drop.chance * (group_chance / 100.0) * rate_chance;
            if world.roll_f64() * 100.0 >= chance {
                continue;
            }
            // Amount.
            let rate_amount = by_id_amount
                .get(&drop.item_id)
                .copied()
                .unwrap_or(amount_mult);
            let base = if drop.max > drop.min {
                drop.min + world.roll((drop.max - drop.min + 1) as i32) as i64
            } else {
                drop.min
            };
            let count = ((base as f64) * rate_amount).round().max(1.0) as i64;
            if drop.chance < 100.0 {
                occurrences -= 1;
            }
            out.push((drop.item_id, count));
        }
    }
    out
}

/// `NpcTemplate.calculateDrops(DropType.SPOIL)` — the `<spoil>` list only
/// (never grouped, never adena), rolled with the spoil rate multipliers. Mirrors
/// `roll_drops`' ungrouped path but: the `SPOIL` branch of
/// `calculateUngroupedDrop` seeds `rateChance`/`rateAmount` from the spoil
/// multipliers and does **not** read the per-item `RATE_DROP_*_BY_ID` overrides.
/// The item-level-gap gate still applies (spoil items are never adena).
fn roll_spoil_drops(world: &mut World, t: &NpcTemplate, killer_oid: i32) -> Vec<(i32, i64)> {
    if t.drop_list_spoil.is_empty() {
        return Vec::new();
    }
    let Some(killer) = world
        .objects
        .get_component::<crate::model::Player>(&killer_oid)
    else {
        return Vec::new();
    };
    let level_diff = (t.level - killer.level) as f64;
    let r = &world.cfg.rates;
    let item_gap_chance = formulas::map_range(
        level_diff,
        -(r.drop_item_max_level_difference as f64),
        -(r.drop_item_min_level_difference as f64),
        r.drop_item_min_level_gap_chance,
        100.0,
    );
    let mut occurrences = r.drop_max_occurrences_normal;
    let chance_mult = r.spoil_drop_chance_multiplier;
    let amount_mult = r.spoil_drop_amount_multiplier;

    let mut out = Vec::new();
    for drop in &t.drop_list_spoil {
        if occurrences == 0 && drop.chance < 100.0 {
            continue;
        }
        // Level-gap gate (item gap — spoil never contains adena).
        if world.roll_f64() * 100.0 > item_gap_chance {
            continue;
        }
        // Chance roll.
        let chance = drop.chance * chance_mult;
        if world.roll_f64() * 100.0 >= chance {
            continue;
        }
        // Amount.
        let base = if drop.max > drop.min {
            drop.min + world.roll((drop.max - drop.min + 1) as i32) as i64
        } else {
            drop.min
        };
        let count = ((base as f64) * amount_mult).round().max(1.0) as i64;
        if drop.chance < 100.0 {
            occurrences -= 1;
        }
        out.push((drop.item_id, count));
    }
    out
}

/// `Player.addItem` (auto-loot path): stack or create, persist, notify.
/// Ground drops (`AutoLoot = False`) are not ported — with the dist config's
/// `AutoLoot = True` this is the live path.
pub(crate) fn give_item(world: &mut World, player_oid: i32, item_id: i32, count: i64) {
    if !world.cfg.character.auto_loot {
        return; // ground-drop path unported (see PROGRESS G9 notes).
    }
    let Some(changed_oids) = super::items::add_inventory_item(world, player_oid, item_id, count)
    else {
        tracing::warn!("give_item: object-id pool exhausted, dropping loot {item_id}×{count}");
        return;
    };

    // "You have obtained …" + InventoryUpdate + the weight/adena footers.
    let Some(client_id) = client_for_player(world, player_oid) else {
        return;
    };
    let Some(inventory) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player_oid)
    else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        let sm = if item_id == ADENA_ID {
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_OBTAINED_S1_ADENA,
                &[SmParam::Long(count)],
            )
        } else if count > 1 {
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_OBTAINED_S2_S1,
                &[SmParam::ItemName(item_id), SmParam::Long(count)],
            )
        } else {
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_OBTAINED_S1,
                &[SmParam::ItemName(item_id)],
            )
        };
        cs.send(sm);
        cs.send(crate::network::enter_world::inventory_update(
            inventory,
            &world.data,
            &changed_oids,
        ));
    }
}

// ---------------------------------------------------------------------------
// XP/SP gain and level-ups (`PlayableStat.addExp` / `PlayerStat.addLevel`)
// ---------------------------------------------------------------------------

/// `Player.onDieDropItem` — scatter part of the victim's inventory.
///
/// Two rate sets, per Java:
/// * a **playable** killer only triggers drops when the victim is a PK past
///   `MinimumPKRequiredToDrop` — this is the karma penalty, not a general
///   looting mechanic;
/// * a **monster** killer uses the (much gentler) player rates, which is why an
///   ordinary death to a mob can still cost an item.
///
/// Nothing drops inside a PVP zone when a player did the killing (arena deaths
/// are free), and GMs are exempt.
///
/// Not modelled: shadow / time-limited items (no ported source), pet control
/// items (G29), the `KarmaListNonDroppableItems` whitelists, and the clan-war
/// exemption (`TODO(G18)` — warring clans don't make each other drop).
fn on_die_drop_item(world: &mut World, victim_oid: i32, killer_oid: i32) {
    use crate::data::item_data;

    let killer_is_player = world
        .objects
        .has_component::<crate::model::Player>(&killer_oid);
    // Arena deaths cost nothing when another player did it.
    if killer_is_player
        && world
            .objects
            .get_component::<crate::model::components::ZoneFlags>(&victim_oid)
            .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Pvp))
    {
        return;
    }
    let Some(victim) = world
        .objects
        .get_component::<crate::model::Player>(&victim_oid)
    else {
        return;
    };
    if victim.is_gm(&world.data) {
        return;
    }
    let (reputation, pk_kills) = (victim.reputation, victim.pk_kills);

    let r = &world.cfg.rates;
    let (rate, item_pct, equip_pct, weapon_pct, limit) = if killer_is_player {
        // Karma drops only once the victim is a repeat PK.
        if reputation >= 0 || pk_kills < r.karma_pk_limit {
            return;
        }
        (
            r.karma_rate_drop,
            r.karma_rate_drop_item,
            r.karma_rate_drop_equip,
            r.karma_rate_drop_equip_weapon,
            r.karma_drop_limit,
        )
    } else if crate::game_loop::combat::is_npc_oid(killer_oid) {
        (
            r.player_rate_drop,
            r.player_rate_drop_item,
            r.player_rate_drop_equip,
            r.player_rate_drop_equip_weapon,
            r.player_drop_limit,
        )
    } else {
        return;
    };

    if rate <= 0 || world.roll(100) >= rate {
        return;
    }

    // Snapshot first: dropping mutates the inventory underneath us.
    let candidates: Vec<(i32, i32, i64, i32)> = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&victim_oid)
        .map(|inv| {
            inv.items()
                .iter()
                .map(|i| (i.object_id, i.item_id, i.count, i.enchant_level))
                .collect()
        })
        .unwrap_or_default();
    let Some(pos) = world
        .objects
        .get_component::<Position>(&victim_oid)
        .copied()
    else {
        return;
    };

    let mut dropped = 0;
    for (obj_id, item_id, count, enchant) in candidates {
        if limit > 0 && dropped >= limit {
            break;
        }
        let Some(t) = world.data.item_data.get(item_id) else {
            continue;
        };
        // Adena and quest items never drop.
        if item_id == item_data::ADENA_ID || t.is_quest_item || t.type2 == item_data::TYPE2_QUEST {
            continue;
        }
        let equipped = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&victim_oid)
            .is_some_and(|inv| inv.paperdoll_slot_of(obj_id).is_some());
        let chance = if equipped {
            if t.type2 == item_data::TYPE2_WEAPON {
                weapon_pct
            } else {
                equip_pct
            }
        } else {
            item_pct
        };
        if chance <= 0 || world.roll(100) >= chance {
            continue;
        }
        // Equipped items come off first (`unEquipItemInSlot`).
        if equipped {
            if let Some(inv) = world
                .objects
                .get_component_mut::<crate::model::inventory::Inventory>(&victim_oid)
            {
                inv.unequip_item(obj_id);
            }
        }
        if let Some(inv) = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&victim_oid)
        {
            inv.remove_item(item_id, count);
        }
        crate::game_loop::ground_items::spawn_ground_item(
            world,
            item_id,
            count,
            enchant,
            pos.x,
            pos.y,
            pos.z,
            victim_oid,
            crate::game_loop::ground_items::DropSource::Player,
        );
        dropped += 1;
    }
    if dropped > 0 {
        if let Some(client_id) = client_for_player(world, victim_oid) {
            if let Some(v) = crate::model::PlayerView::of(&world.objects, victim_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(crate::network::enter_world::item_list(
                        v.inventory,
                        &world.data,
                        false,
                    ));
                }
            }
        }
    }
}

/// `Attackable.calculateOverhitExp` — the bonus XP a killing `<overHit>` blow
/// earns, and the "over-hit!" notice that goes with it.
///
/// The bonus is the excess damage as a share of the victim's **max** HP,
/// **capped at 25 %**, applied to that attacker's exp share. Returns 0 for
/// anyone who didn't land the over-hit blow, and clears the record so a single
/// kill pays it once.
fn overhit_bonus(world: &mut World, npc_oid: i32, attacker_oid: i32, exp: f64) -> f64 {
    use crate::model::components::Overhit;
    let Some(oh) = world.objects.get_component::<Overhit>(&npc_oid).copied() else {
        return 0.0;
    };
    if oh.attacker != attacker_oid {
        return 0.0;
    }
    let max_hp = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .map(|v| v.max_hp as f64)
        .unwrap_or(0.0);
    if max_hp <= 0.0 {
        return 0.0;
    }
    world.objects.remove_component::<Overhit>(&npc_oid);
    let percentage = ((oh.damage * 100.0) / max_hp).min(25.0);
    if let Some(client_id) = client_for_player(world, attacker_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_ids::OVER_HIT, &[]));
        }
    }
    (percentage / 100.0) * exp
}

/// The vitality half of `Attackable.onKill`'s reward block: charge the killer
/// for the kill (`updateVitalityPoints(getVitalityPoints(level, exp, isRaid),
/// true, false)`).
///
/// `RaidbossUseVitality = False` on this dist, so raid kills are skipped
/// outright — Java expresses the same thing through
/// `Config.RAIDBOSS_USE_VITALITY` gating `_isRaid` into the boss branch.
pub(crate) fn consume_kill_vitality(
    world: &mut World,
    player_oid: i32,
    player_level: i32,
    t: &NpcTemplate,
    exp: f64,
) {
    if !world.cfg.character.enable_vitality {
        return;
    }
    let is_boss = t.is_raid();
    if is_boss && !world.cfg.character.raidboss_use_vitality {
        return;
    }
    let delta =
        super::vitality::kill_vitality_delta(world, t.level, t.exp, player_level, exp, is_boss);
    super::vitality::update_vitality_points(world, player_oid, delta, true, false);
    // TODO(G16): Java also calls `PcCafePointsManager.givePcCafePoint(attacker,
    // exp)` right here (PC_CAFE_RETAIL_LIKE); the points store exists
    // (`//pccafepoints`) but the earn-per-kill manager is unported.
}

/// `PlayerStat.addExpAndSp(addToExp, addToSp, useBonuses)`.
///
/// `use_bonuses` is Java's third argument: the kill path passes
/// `Attackable.useVitalityRate()` (always true here — champion monsters aren't
/// ported), while quest rewards and `//add_exp_sp` go through the two-argument
/// overload, which passes **false**. When set, the vitality/skill exp bonus
/// multiplies the reward and the acquisition message reports the surplus in its
/// "bonus" slots — which is where the client's floating "+N XP bonus" comes
/// from.
///
/// Java's fishing-rod branch (`FANCY_FISHING_ROD_SKILL` → ×1.5) is not ported —
/// fishing is G32. Amounts stay `f64` until the final `Math.round`, as in Java,
/// so the bonus never compounds a rounding error.
pub(crate) fn add_exp_and_sp(
    world: &mut World,
    player_oid: i32,
    exp: f64,
    sp: f64,
    use_bonuses: bool,
) {
    let (bonus_exp, bonus_sp) = if use_bonuses {
        // Java reads the exp and sp multipliers separately; with BONUS_EXP /
        // BONUS_SP unmodelled they are the same value today.
        (
            super::vitality::exp_bonus_multiplier(world, player_oid),
            super::vitality::exp_bonus_multiplier(world, player_oid),
        )
    } else {
        (1.0, 1.0)
    };
    let (base_exp, base_sp) = (exp, sp);
    let (mut add_exp, mut add_sp) = (exp * bonus_exp, sp * bonus_sp);

    // Java `PlayerStat.addExpAndSp`: a nearby pet takes its cut **out of the
    // owner's award**, not on top of it — hunting with a pet costs the player
    // exp. The split happens after the bonuses, so the pet shares them.
    let (owner_ratio, pet_exp, pet_sp) =
        super::servitor::split_exp_with_pet(world, player_oid, add_exp, add_sp);
    if pet_exp > 0.0 || pet_sp > 0.0 {
        super::servitor::add_pet_exp(world, player_oid, pet_exp, pet_sp);
    }
    add_exp *= owner_ratio;
    add_sp *= owner_ratio;

    let (exp, sp) = (add_exp.round() as i64, add_sp.round() as i64);

    let max_level = world.data.experience.max_level as i32;
    let cap = world.data.experience.exp_for_level(max_level) - 1;
    let (old_level, new_exp) = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        p.exp = (p.exp + exp.max(0)).min(cap);
        p.sp = p.sp.saturating_add(sp.max(0));
        (p.level, p.exp)
    };
    if exp > 0 || sp > 0 {
        if let Some(client_id) = client_for_player(world, player_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4,
                    &[
                        SmParam::Long(exp),
                        SmParam::Long((add_exp - base_exp).round() as i64),
                        SmParam::Long(sp),
                        SmParam::Long((add_sp - base_sp).round() as i64),
                    ],
                ));
            }
        }
    }

    let new_level = level_for_exp(world, new_exp, max_level);
    if new_level != old_level {
        set_level(world, player_oid, new_level);
    } else if let Some(client_id) = client_for_player(world, player_oid) {
        // Exp bar refresh (`player.updateUserInfo()`).
        if let (Some(v), Some(cs)) = (
            crate::model::PlayerView::of(&world.objects, player_oid),
            world.clients.get(&client_id),
        ) {
            cs.send(crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                super::party::calculate_relation(world, v.p),
            ));
        }
    }
}

/// Java `Player.removeExpAndSp` — subtract exp/sp (each floored at 0) and
/// delevel if the exp total now falls under the current level's threshold. The
/// mirror of [`add_exp_and_sp`]; used by the `//remove_exp_sp` admin command.
pub(crate) fn remove_exp_and_sp(world: &mut World, player_oid: i32, exp: i64, sp: i64) {
    let max_level = world.data.experience.max_level as i32;
    let (old_level, new_exp) = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        p.exp = (p.exp - exp.max(0)).max(0);
        p.sp = (p.sp - sp.max(0)).max(0);
        (p.level, p.exp)
    };
    let new_level = level_for_exp(world, new_exp, max_level);
    if new_level != old_level {
        set_level(world, player_oid, new_level);
    } else if let Some(client_id) = client_for_player(world, player_oid) {
        // Exp bar refresh (`player.updateUserInfo()`), no level change.
        if let (Some(v), Some(cs)) = (
            crate::model::PlayerView::of(&world.objects, player_oid),
            world.clients.get(&client_id),
        ) {
            cs.send(crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                super::party::calculate_relation(world, v.p),
            ));
        }
    }
}

/// The `PlayableStat.addExp` level scan: highest level whose threshold the
/// exp total clears.
fn level_for_exp(world: &World, exp: i64, max_level: i32) -> i32 {
    let mut level = 1;
    for l in 1..=max_level {
        if exp >= world.data.experience.exp_for_level(l) {
            level = l;
        } else {
            break;
        }
    }
    level
}

/// `PlayerStat.addLevel` (up or down): recompute vitals/stats, grant new
/// autoGet skills, broadcast the level-up flourish.
pub(crate) fn set_level(world: &mut World, player_oid: i32, new_level: i32) {
    let leveled_up = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        let up = new_level > p.level;
        p.level = new_level;
        up
    };
    // Vitals follow the level tables (`getMaxHp` etc. read level).
    {
        let data = &world.data;
        let Some((p, mut vitals, mut pvitals, base, mods, inventory, mut speeds, mut combat)) =
            world.objects.get_many_mut::<(
                &mut crate::model::Player,
                &mut Vitals,
                &mut PlayerVitals,
                &BaseStats,
                &StatModifiers,
                &crate::model::inventory::Inventory,
                &mut Speeds,
                &mut crate::model::components::CombatStats,
            )>(&player_oid)
        else {
            return;
        };
        let t = data
            .player_templates
            .get(p.class_id)
            .or_else(|| data.player_templates.get(p.base_class_id))
            .cloned()
            .unwrap_or_default();
        vitals.max_hp = crate::model::calc_max_hp(data, &t, p.level, Some(inventory), mods) as i32;
        vitals.max_mp = crate::model::calc_max_mp(data, &t, p.level, Some(inventory), mods) as i32;
        pvitals.max_cp = crate::model::calc_max_cp(data, &t, p.level, mods) as i32;
        if leveled_up {
            // Classic level-up: all vitals refill (Mobius Java only refills
            // CP here, but retail Classic restores HP/MP too).
            vitals.cur_hp = vitals.max_hp as f64;
            vitals.cur_mp = vitals.max_mp as f64;
            pvitals.cur_cp = pvitals.max_cp as f64;
        } else {
            vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
            vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
            pvitals.cur_cp = pvitals.cur_cp.min(pvitals.max_cp as f64);
        }
        p.recalculate_stats(data, base, mods, inventory, &mut speeds, &mut combat);
    }

    // `rewardSkills`: grant the skills now reachable (autoGet only, or — with
    // `AutoLearnSkills` — every reachable class skill).
    reward_skills(world, player_oid);

    // `Player.checkPlayerSkills` (`PlayableStat.addLevel` on a delevel, and
    // inside `rewardSkills`): downgrade/remove any skill that now outranks the
    // level. No-op on a level-up (nothing sits above the higher level).
    check_player_skills(world, player_oid);

    if leveled_up {
        broadcast_including_self(
            world,
            player_oid,
            &server_packets::social_action(player_oid, server_packets::SOCIAL_ACTION_LEVEL_UP),
        );
    }
    // Status + full info refresh (`broadcastStatusUpdate` + `updateUserInfo`
    // + `SkillList`).
    let (Some(vitals), Some(pvitals)) = (
        world.objects.get_component::<Vitals>(&player_oid).copied(),
        world
            .objects
            .get_component::<PlayerVitals>(&player_oid)
            .copied(),
    ) else {
        return;
    };
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[
                (server_packets::status_update_type::MAX_HP, vitals.max_hp),
                (
                    server_packets::status_update_type::CUR_HP,
                    vitals.cur_hp as i32,
                ),
                (server_packets::status_update_type::MAX_MP, vitals.max_mp),
                (
                    server_packets::status_update_type::CUR_MP,
                    vitals.cur_mp as i32,
                ),
                (server_packets::status_update_type::MAX_CP, pvitals.max_cp),
                (
                    server_packets::status_update_type::CUR_CP,
                    pvitals.cur_cp as i32,
                ),
            ],
        ),
    );
    // Java `PlayerStat.addLevel` → `PartySmallWindowUpdate(this, true)`.
    super::party::notify_party_all(world, player_oid);
    if let Some(client_id) = client_for_player(world, player_oid) {
        if let (Some(v), Some(cs)) = (
            crate::model::PlayerView::of(&world.objects, player_oid),
            world.clients.get(&client_id),
        ) {
            if leveled_up {
                cs.send(server_packets::system_message_with(
                    sm_ids::YOUR_LEVEL_HAS_INCREASED,
                    &[],
                ));
            }
            cs.send(crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                super::party::calculate_relation(world, v.p),
            ));
            let Some(pkt) = super::helpers::skill_list_packet(world, player_oid) else {
                return;
            };
            cs.send(pkt);
        }
    }
}

/// Java `Player.rewardSkills` skill selection: with `AutoLearnSkills` on,
/// every class skill reachable at `level`; otherwise autoGet skills only.
/// Returns the `(id, level)` pairs that are new or an upgrade over `known`.
pub(crate) fn reward_skill_grants(
    data: &crate::data::GameData,
    cfg: &crate::config::CharacterConfig,
    class_id: i32,
    level: i32,
    known: &std::collections::HashMap<i32, i32>,
    is_gm: bool,
) -> Vec<(i32, i32)> {
    if cfg.auto_learn_skills {
        return data.skill_trees.all_available_skills(
            class_id,
            level,
            known,
            cfg.auto_learn_skills_without_items,
            cfg.auto_learn_divine_inspiration || is_gm,
        );
    }
    let mut granted = Vec::new();
    let mut seen: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    for learn in data.skill_trees.auto_get_skills(class_id, level) {
        let cur = seen
            .get(&learn.skill_id)
            .copied()
            .unwrap_or_else(|| known.get(&learn.skill_id).copied().unwrap_or(0));
        if learn.skill_level > cur {
            seen.insert(learn.skill_id, learn.skill_level);
            granted.push((learn.skill_id, learn.skill_level));
        }
    }
    granted
}

/// `Player.rewardSkills` for a live in-world player: grant the reachable
/// skills, persist them, and roll any upgrades into panel shortcuts. With
/// `AutoLearnSkills` it mirrors Java's `ShortCutInit` + "learned N skills"
/// notice.
pub(crate) fn reward_skills(world: &mut World, player_oid: i32) {
    let (class_id, level, known, is_gm) = {
        let Some(p) = world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        let skills = world
            .objects
            .get_component::<SkillBook>(&player_oid)
            .cloned()
            .unwrap_or_default();
        (p.class_id, p.level, skills.0, p.is_gm(&world.data))
    };
    let granted = reward_skill_grants(
        &world.data,
        &world.cfg.character,
        class_id,
        level,
        &known,
        is_gm,
    );
    if granted.is_empty() {
        return;
    }
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&player_oid) {
        for &(id, lvl) in &granted {
            book.0.insert(id, lvl);
        }
    }
    for &(id, lvl) in &granted {
        // Memory-first: the grant already landed in the `SkillBook`; it persists
        // on the next flush. `updateShortCuts` — panel slots holding the skill
        // pick up the level (also in-memory).
        super::shortcuts::update_skill_shortcuts(world, player_oid, id, lvl);
    }
    if world.cfg.character.auto_learn_skills {
        if let Some(client_id) = client_for_player(world, player_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                let count = granted
                    .iter()
                    .map(|&(id, _)| id)
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                if let Some(shortcuts) = world
                    .objects
                    .get_component::<crate::model::components::Shortcuts>(&player_oid)
                {
                    cs.send(server_packets::shortcut_init(shortcuts));
                }
                cs.send(server_packets::system_message_with(
                    sm_ids::S1_TEXT,
                    &[SmParam::Text(format!(
                        "You have learned {count} new skills."
                    ))],
                ));
            }
        }
    }
}

/// Java `Player.checkPlayerSkills` + `deacreaseSkillLevel`, as a reusable
/// filter: downgrade or remove the entries in `skills` that the character's
/// `level` no longer supports (config `StrictDelevelSkillRemoval` grace),
/// persisting each change to `character_skills`. Mutates `skills` in place and
/// returns the applied `(skill_id, Some(new_level) | None)` changes so the
/// caller can sync panel shortcuts and — for a live player — recompute passive
/// stats. Empty / no-op when `DecreaseSkillOnDelevel` is off.
///
/// The two call sites: character select (filtering the DB-loaded skill list
/// before the `Player` is built, so `from_char` folds the corrected passives)
/// and every level-down (`PlayerStat.addLevel`, via [`check_player_skills`]).
pub(crate) fn maybe_skill_remove_on_delevel(
    world: &World,
    char_id: i32,
    class_id: i32,
    level: i32,
    skills: &mut std::collections::HashMap<i32, i32>,
) -> Vec<(i32, Option<i32>)> {
    if !world.cfg.character.decrease_skill_level {
        return Vec::new();
    }
    let changes = world.data.skill_trees.delevel_skill_changes(
        class_id,
        level,
        skills,
        world.cfg.character.strict_delevel_skill_removal,
    );
    let _ = char_id; // memory-first: the changes below persist on the next flush.
    for &(skill_id, action) in &changes {
        match action {
            // `deacreaseSkillLevel` → `addSkill(getSkill(id, nextLevel))`.
            Some(new_level) => {
                skills.insert(skill_id, new_level);
            }
            // `deacreaseSkillLevel` → `removeSkill(skill, true)`.
            None => {
                skills.remove(&skill_id);
            }
        }
    }
    changes
}

/// `Player.checkPlayerSkills` for a live in-world player (a level-down):
/// [`maybe_skill_remove_on_delevel`] on the `SkillBook`, then roll the changes
/// into panel shortcuts and re-fold the passive stats (only passive skills move
/// `UserInfo` stats), broadcasting the fresh stats.
pub(crate) fn check_player_skills(world: &mut World, player_oid: i32) {
    let (class_id, level, mut known) = {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return;
        };
        let skills = world
            .objects
            .get_component::<SkillBook>(&player_oid)
            .cloned()
            .unwrap_or_default();
        (p.class_id, p.level, skills.0)
    };
    let changes = maybe_skill_remove_on_delevel(world, player_oid, class_id, level, &mut known);
    if changes.is_empty() {
        return;
    }
    // Write the filtered book back, then sync the panel shortcuts.
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&player_oid) {
        book.0 = known;
    }
    for &(skill_id, action) in &changes {
        match action {
            Some(new_level) => {
                super::shortcuts::update_skill_shortcuts(world, player_oid, skill_id, new_level)
            }
            None => super::shortcuts::remove_skill_shortcuts(world, player_oid, skill_id),
        }
    }
    recompute_passives_after_skill_change(world, player_oid, &changes);
}

/// Re-derive a live player's passive-skill stat contributions after a delevel
/// skill change: drop the removed skills' passive buffs, then re-fold the
/// armor-conditioned passives (a downgraded passive re-applies at its new
/// level). Only passive skills carry stat modifiers, so removing/downgrading an
/// active skill leaves the stats untouched here. Updates the stat components in
/// place but sends no packet — the caller (`set_level`) already broadcasts a
/// fresh `UserInfo` for the level change, so this avoids a redundant second one.
fn recompute_passives_after_skill_change(
    world: &mut World,
    player_oid: i32,
    changes: &[(i32, Option<i32>)],
) {
    let removed: Vec<i32> = changes
        .iter()
        .filter_map(|&(id, action)| action.is_none().then_some(id))
        .collect();
    if !removed.is_empty() {
        if let Some((player, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) =
            world.objects.get_many_mut::<(
                &Player,
                &BaseStats,
                &mut StatModifiers,
                &Inventory,
                &mut Buffs,
                &mut Speeds,
                &mut CombatStats,
            )>(&player_oid)
        {
            for skill_id in &removed {
                player.remove_buff(
                    &world.data,
                    base,
                    &mut mods,
                    inventory,
                    &mut buffs,
                    &mut speeds,
                    &mut combat,
                    *skill_id,
                );
            }
        }
    }
    // Re-fold conditioned passives from the corrected book (handles downgrades),
    // component-only — no send.
    super::passive_skills::recompute_conditioned_passives(world, player_oid);
}

// ---------------------------------------------------------------------------
// Player death + revive
// ---------------------------------------------------------------------------

/// `Player.doDie`: mark dead, stop everything, apply the XP penalty,
/// broadcast `Die` with the to-village flag.
pub(crate) fn player_do_die(world: &mut World, player_oid: i32, killer_oid: i32) {
    // Every consequence of this death is Java's `killer.getActingPlayer()`, not
    // "the killer object": a kill landed by someone's **summon** carries the
    // same PK counter, karma, clan-war credit and exp-penalty relief as one
    // they landed themselves.
    //
    // Resolved once at the top rather than at each site. It was previously
    // shadowed part-way down, which happened to cover everything below it —
    // but any code added *above* that point would silently have used the raw
    // id, and there is no signal when that goes wrong.
    let killer_oid = super::pvp::acting_player(world, killer_oid);
    {
        let Some((p, mut vitals)) = world
            .objects
            .get_many_mut::<(&mut crate::model::Player, &mut Vitals)>(&player_oid)
        else {
            return;
        };
        if vitals.dead {
            return;
        }
        vitals.dead = true;
        vitals.cur_hp = 0.0;
        drop((p, vitals));
        world.objects.remove_component::<Movement>(&player_oid);
        world.objects.remove_component::<Intent>(&player_oid);
        world
            .objects
            .remove_component::<crate::model::components::QueuedAction>(&player_oid);
        if let Some(t) = world
            .objects
            .get_component_mut::<crate::model::components::TargetRef>(&player_oid)
        {
            t.0 = None;
        }
    }
    // Any cast dies with the caster (`abortCast`; also stops pre-launch
    // packets via the seq mismatch).
    super::skills::cast::abort_cast(world, player_oid);

    // `Playable.doDie`'s buff block: death normally strips everything, unless
    // Noblesse Blessing is up — then only the blessing goes.
    stop_effects_on_death(world, player_oid);

    // `Player.doDie`'s reputation block: a player killer takes the PvP/PK
    // consequences (counters, karma) for this death.
    if world
        .objects
        .has_component::<crate::model::Player>(&killer_oid)
    {
        super::pvp::on_kill_update_pvp_reputation(world, killer_oid, player_oid);
    }

    // `onDieDropItem` — a PK (or anyone a monster killed) can scatter part of
    // their inventory on the ground. Runs before the XP penalty, as in Java.
    on_die_drop_item(world, player_oid, killer_oid);

    // Clan-war kill bookkeeping (Java `Player.doDie` → `ClanWar.onKill`):
    // only outside PVP/siege zones, killer and victim both clanned players.
    // (Java also runs an AntiFeed check and exempts academy members —
    // AntiFeedManager unported, academy TODO(G18.6).)

    // Death XP penalty — Java skips it entirely when the victim died inside a
    // PVP or siege zone (`!isLucky() && !insidePvpZone && !isOnEvent()`).
    // Arena and siege deaths are free.
    let in_free_death_zone = world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&player_oid)
        .is_some_and(|f| {
            f.contains(crate::data::zone_data::ZoneKind::Pvp)
                || f.contains(crate::data::zone_data::ZoneKind::Siege)
        });
    if !in_free_death_zone
        && world
            .objects
            .has_component::<crate::model::Player>(&killer_oid)
    {
        super::clans::clan_war_on_kill(world, killer_oid, player_oid);
    }
    if !in_free_death_zone {
        // Java `calculateDeathExpPenalty(killer)` quarters the loss when the
        // killer is a clan-war enemy (`atWarWith`, any war state).
        let at_war = {
            let kc = world
                .objects
                .get_component::<crate::model::Player>(&killer_oid)
                .map(|p| p.clan_id)
                .unwrap_or(0);
            let vc = world
                .objects
                .get_component::<crate::model::Player>(&player_oid)
                .map(|p| p.clan_id)
                .unwrap_or(0);
            super::clans::at_war_between(world, kc, vc)
        };
        apply_death_exp_penalty_ex(world, player_oid, at_war);
    }

    broadcast_including_self(world, player_oid, &server_packets::die(player_oid, true));
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[(server_packets::status_update_type::CUR_HP, 0)],
        ),
    );
}

/// `Playable.doDie`'s effect block.
///
/// Java: a `NOBLESS_BLESSING` (or `RESURRECTION_SPECIAL`) holder stops *only*
/// that effect and keeps the rest of its buffs through death and the following
/// resurrection; everyone else runs
/// `stopAllEffectsExceptThoseThatLastThroughDeath`, which strips every active
/// buff whose skill isn't `<stayAfterDeath>`.
///
/// `RESURRECTION_SPECIAL` has no ported source yet (the self-res effect is
/// TODO(G22)), so only the blessing can spare the buff list here.
///
/// Passive entries are skipped: Java's sweep runs over `EffectList._actives`
/// only, while this port parks the grade-penalty passives in the same `Buffs`
/// vec — dropping those would silently unwind a passive's stat pump on death.
fn stop_effects_on_death(world: &mut World, player_oid: i32) {
    use crate::model::skill::effect_flag;

    let blessed = super::abnormal::flags_of(world, player_oid) & effect_flag::NOBLESS_BLESSING != 0;
    let Some(buffs) = world.objects.get_component::<Buffs>(&player_oid) else {
        return;
    };
    let to_stop: Vec<i32> = buffs
        .0
        .iter()
        .filter(|b| !b.passive)
        .filter(|b| {
            if blessed {
                // `stopEffects(EffectFlag.NOBLESS_BLESSING)` — the blessing and
                // nothing else.
                b.effect_flags & effect_flag::NOBLESS_BLESSING != 0
            } else {
                !world
                    .data
                    .skill_data
                    .get(b.skill_id, b.skill_level)
                    .is_some_and(|s| s.stay_after_death)
            }
        })
        .map(|b| b.skill_id)
        .collect();
    for skill_id in to_stop {
        super::skills::effects::handle_buff_expire(world, player_oid, skill_id);
    }
}

/// `Player.calculateDeathExpPenalty` + `PlayableStat.removeExp` (with the
/// `Delevel`/`DelevelMinimum` clamping) + the SM 539 notice.
pub(crate) fn apply_death_exp_penalty(world: &mut World, player_oid: i32) {
    apply_death_exp_penalty_ex(world, player_oid, false);
}

/// The killer-aware variant: `at_war_with_killer` quarters the loss (Java's
/// `lostExp /= 4` for a clan-war death).
pub(crate) fn apply_death_exp_penalty_ex(
    world: &mut World,
    player_oid: i32,
    at_war_with_killer: bool,
) {
    let (level, exp) = {
        let Some(p) = world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        (p.level, p.exp)
    };
    let max_level = world.data.experience.max_level as i32;
    let percent = world.data.xp_lost.xp_percent(level);
    let (lo, hi) = if level < max_level {
        (
            world.data.experience.exp_for_level(level),
            world.data.experience.exp_for_level(level + 1),
        )
    } else {
        (
            world.data.experience.exp_for_level(max_level - 1),
            world.data.experience.exp_for_level(max_level),
        )
    };
    let mut lost = (((hi - lo) as f64) * percent / 100.0).round() as i64;
    if at_war_with_killer {
        lost /= 4;
    }

    // `removeExp`'s delevel clamp: without delevel (or at/below the floor)
    // exp can't drop below the current level's threshold.
    let can_delevel =
        world.cfg.character.player_delevel && level > world.cfg.character.delevel_minimum;
    if !can_delevel {
        lost = lost.min(exp - world.data.experience.exp_for_level(level));
    }
    lost = lost.min(exp - 1).max(0);
    if lost == 0 {
        return;
    }

    let new_exp = exp - lost;
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
    {
        p.exp = new_exp;
        // Java keeps `_expBeforeDeath` and subtracts; the difference is the
        // only thing a resurrection reads, so record that directly.
        p.lost_exp_on_death = lost;
    }
    if let Some(client_id) = client_for_player(world, player_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::YOUR_XP_HAS_DECREASED_BY_S1,
                &[SmParam::Long(lost)],
            ));
        }
    }
    let new_level = level_for_exp(world, new_exp, max_level);
    if new_level != level {
        set_level(world, player_oid, new_level);
    }
}

/// Port of `clientpackets/RequestRestartPoint`: pick the respawn point for the
/// requested restart type — the siege "to castle"/"to siege HQ" cases when the
/// dead player is a participant, else the map-region town respawn — and start
/// the teleport; the revive itself lands on `Appearing`.
pub(crate) fn handle_request_restart_point(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestRestartPoint::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    let (px, py, pz, dead) = {
        let Some(pos) = world.objects.get_component::<Position>(&object_id) else {
            return;
        };
        let Some(vitals) = world.objects.get_component::<Vitals>(&object_id) else {
            return;
        };
        (pos.x, pos.y, pos.z, vitals.dead)
    };
    if !dead {
        return;
    }
    let race = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .and_then(|p| crate::enums::Race::from_ordinal(p.race))
        .unwrap_or(crate::enums::Race::Human);
    let pick = if world.cfg.character.random_respawn_in_town {
        world.roll(64) as usize
    } else {
        0
    };
    // The siege restart cases (Java `RequestRestartPoint.portPlayer`); everything
    // else, and a non-participant, falls through to the map-region town respawn.
    let siege_spawn = siege_restart_location(world, object_id, pkt.point_type, pick);
    let Some((x, y, z)) =
        siege_spawn.or_else(|| world.data.map_region.town_respawn(px, py, pz, race, pick))
    else {
        return;
    };
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&object_id)
    {
        p.pending_revive = true;
    }
    teleport_player(world, object_id, x, y, z);
}

/// The siege restart-point cases of Java `RequestRestartPoint.portPlayer` /
/// `MapRegionManager.getTeleToLocation` we can honor at a castle under an active
/// siege:
/// - **to castle** (type 2): a *defender* (the owner or a registered defender
///   clan) respawns inside the castle at the residence `getSpawnLoc`.
/// - **to siege HQ** (type 4): an *attacker* respawns at their planted HQ flag
///   (`getFlag`), if one still stands.
///
/// `None` (→ the caller's town respawn) for every other type/role. Note the
/// castle respawn is *not* gated on the control-tower count: in Interlude
/// Classic that count has no respawn/resurrection outcome at all (it only picks
/// a rejection message for a normal res skill during a siege — see
/// `Siege.control_tower_count`). The attacker respawn delay
/// (`getAttackerRespawnDelay`) is deferred (TODO(G24)).
fn siege_restart_location(
    world: &World,
    player_oid: i32,
    point_type: i32,
    pick: usize,
) -> Option<(i32, i32, i32)> {
    use crate::model::siege::SiegeClanType;
    let clan_id = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)?
        .clan_id;
    if clan_id == 0 {
        return None;
    }
    let pos = world.objects.get_component::<Position>(&player_oid)?;
    let castle_id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z)?;
    let siege = world.sieges.get(&castle_id)?;
    if !siege.in_progress {
        return None;
    }
    let role = siege
        .clans
        .iter()
        .find(|c| c.clan_id == clan_id)
        .map(|c| c.kind);
    // `checkIsDefender` covers the castle owner even if it holds no `siege_clans`
    // row, so fold in castle ownership.
    let is_defender = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.castle_id == castle_id)
        || matches!(role, Some(SiegeClanType::Owner | SiegeClanType::Defender));
    match point_type {
        2 if is_defender => {
            let pts = world.data.castle_restart_points.get(&castle_id)?;
            (!pts.is_empty()).then(|| pts[pick % pts.len()])
        }
        4 if role == Some(SiegeClanType::Attacker) => {
            let flag_oid = siege.flag_of(clan_id)?;
            world
                .objects
                .get_component::<Position>(&flag_oid)
                .map(|p| (p.x, p.y, p.z))
        }
        _ => None,
    }
}

/// `Creature.teleToLocation`: stop moving, vanish from the old neighborhood
/// (`decayMe` → `DeleteObject`), push the new position, and wait for the
/// client's `Appearing` before becoming visible again.
pub(crate) fn teleport_player(world: &mut World, player_oid: i32, x: i32, y: i32, z: i32) {
    if world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_none()
    {
        return;
    }
    // Java grounds the z on the geodata (`GeoEngine.getHeight`, non-flying)
    // and then lifts it "a bit" (`z += 5`).
    let z = world.geo.get_height(x, y, z) + 5;
    world.objects.remove_component::<Movement>(&player_oid);
    world.objects.remove_component::<Intent>(&player_oid);
    world
        .objects
        .remove_component::<crate::model::components::QueuedAction>(&player_oid);
    // The rest of `teleToLocation`'s prologue, in Java's order: cancel the
    // client's pending action, `abortCast()`, then `setTarget(null)` — all
    // before `decayMe`. The abort is what tells the client to stop drawing
    // the cast animation; a skill that teleports on landing (`/unstuck`'s
    // Escape, Recall) would otherwise leave the FX playing at the destination
    // for the client's own skill duration.
    if let Some(cs) = client_for_player(world, player_oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::action_failed());
    }
    super::skills::cast::abort_cast_on_teleport(world, player_oid);
    super::target::drop_target_notify(world, player_oid);
    let Some(heading) = world
        .objects
        .get_component::<Position>(&player_oid)
        .map(|p| p.heading)
    else {
        return;
    };
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::teleport_to_location(player_oid, x, y, z, heading),
    );
    // `decayMe`: DeleteObject to everyone who could see the old position
    // (also drops their dangling targets).
    super::visibility::on_leave_world(world, player_oid);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
    {
        p.teleporting = true;
    }
    if let Some(pos) = world.objects.get_component_mut::<Position>(&player_oid) {
        pos.x = x;
        pos.y = y;
        pos.z = z;
    }
    if let Some(region) = world.objects.get_component_mut::<RegionCell>(&player_oid) {
        region.0 = crate::world::region_of(x, y);
    }
    // "Send teleport finished packet to player" (Java, right after `setXYZ`):
    // the client sits on the black loading screen until this arrives, then
    // loads the destination and answers with `Appearing`.
    if let Some(cs) = client_for_player(world, player_oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::ex_teleport_to_location_activate(
            player_oid, x, y, z, heading,
        ));
    }
}

/// Port of `clientpackets/Appearing`: the client finished loading after a
/// teleport — `onTeleported` (spawnMe → mutual CharInfo/NpcInfo, pending
/// revive resolves, fresh `UserInfo`).
pub(crate) fn handle_appearing(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    if !world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .is_some_and(|p| p.teleporting)
    {
        return;
    }
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&object_id)
    {
        p.teleporting = false;
    }
    if world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .is_some_and(|p| p.pending_revive)
    {
        do_revive(world, object_id);
    }
    // `spawnMe`-equivalent visibility exchange at the new position.
    super::visibility::on_enter_world(world, client_id, object_id);
    // Java `onTeleported` → `revalidateZone(true)`.
    super::zones::revalidate_zone(world, object_id, true);
    if let (Some(v), Some(cs)) = (
        crate::model::PlayerView::of(&world.objects, object_id),
        world.clients.get(&client_id),
    ) {
        cs.send(crate::network::user_info::user_info(
            &v,
            &world.data,
            &world.cfg.character,
            super::party::calculate_relation(world, v.p),
        ));
    }
}

/// `Formulas.calculateSkillResurrectRestorePercent` — the reviver's WIT scales
/// how much of the lost XP their resurrection gives back.
///
/// ```java
/// if (base == 0 || base == 100) return base;
/// restore = base * WIT.calcBonus(caster);
/// if ((restore - base) > 20.0) restore += 20.0;
/// return min(max(restore, base), 90.0);
/// ```
///
/// Note the quirk on the third line: a bonus that already exceeds +20 gets a
/// *further* flat +20, so high-WIT revivers jump rather than scale smoothly.
/// Ported as written.
pub(crate) fn resurrect_restore_percent(base: f64, wit_bonus: f64) -> f64 {
    if base == 0.0 || base == 100.0 {
        return base;
    }
    let mut restore = base * wit_bonus;
    if (restore - base) > 20.0 {
        restore += 20.0;
    }
    restore.max(base).min(90.0)
}

/// `Player.reviveRequest` — propose a resurrection to a dead player.
///
/// Nothing is restored here: the corpse gets a `ConfirmDlg` and decides.
/// A second proposal while one is outstanding is refused with Java's
/// "Resurrection has already been proposed" notice, which is what stops two
/// clerics from racing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn revive_request(
    world: &mut World,
    reviver_oid: i32,
    target_oid: i32,
    power: i32,
    hp_percent: i32,
    mp_percent: i32,
    cp_percent: i32,
) {
    use crate::network::server_packets::sm_ids;
    // `isResurrectionBlocked()` — Java also ORs `isInvul()`; the flag is the
    // ported half (`BlockResurrection` has no learnable source on this dist).
    if crate::game_loop::abnormal::flags_of(world, target_oid)
        & crate::model::skill::effect_flag::BLOCK_RESURRECTION
        != 0
    {
        return;
    }
    // Java `Resurrection` calls `effected.getActingPlayer().reviveRequest(…,
    // effected.isPet(), …)`: casting on a dead **pet** puts the dialog in front
    // of its **owner**, who is the one who answers. So resolve the corpse to
    // the player who will be asked, and remember which of the two is dying.
    let is_pet = world
        .objects
        .has_component::<crate::model::components::PetOf>(&target_oid);
    let corpse_oid = target_oid;
    let target_oid = if is_pet {
        match world
            .objects
            .get_component::<crate::model::components::ServitorOf>(&corpse_oid)
        {
            Some(s) => s.owner_object_id,
            None => return,
        }
    } else {
        target_oid
    };

    let Some(target) = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
    else {
        return;
    };
    if world
        .objects
        .get_component::<Vitals>(&corpse_oid)
        .is_none_or(|v| !v.dead)
    {
        return;
    }
    if target.revive_request.is_some() {
        if let Some(cid) = client_for_player(world, reviver_oid) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(server_packets::system_message_with(
                    sm_ids::RESURRECTION_HAS_ALREADY_BEEN_PROPOSED,
                    &[],
                ));
            }
        }
        return;
    }
    // `calculateSkillResurrectRestorePercent(power, reviver)`.
    let wit_bonus = world
        .objects
        .get_component::<crate::model::components::BaseStats>(&reviver_oid)
        .map(|b| {
            world
                .data
                .stat_bonus
                .bonus(crate::model::stats::BaseStat::Wit, b.wit)
        })
        .unwrap_or(1.0);
    let restore_percent = resurrect_restore_percent(power as f64, wit_bonus);

    let lost = if is_pet {
        // A pet's restorable exp is the gap the death penalty opened.
        world
            .objects
            .get_component::<crate::model::components::PetOf>(&corpse_oid)
            .map(|p| (p.exp_before_death - p.exp).max(0))
            .unwrap_or(0)
    } else {
        world
            .objects
            .get_component::<crate::model::Player>(&target_oid)
            .map(|p| p.lost_exp_on_death)
            .unwrap_or(0)
    };
    let restore_exp = ((lost as f64 * restore_percent) / 100.0).round() as i64;

    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    {
        p.revive_request = Some(crate::model::ReviveRequest {
            reviver: reviver_oid,
            restore_percent,
            hp_percent,
            mp_percent,
            cp_percent,
            is_pet,
        });
    }
    // Java's `ConfirmDlg(C1_IS_ATTEMPTING_TO_DO_A_RESURRECTION_THAT_RESTORES_S2_S3_XP_ACCEPT)`.
    // This port has only the generic text dialog, so the message is rendered
    // rather than composed from the client's string table.
    let reviver_name = world
        .objects
        .get_component::<crate::model::Player>(&reviver_oid)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    if let Some(cid) = client_for_player(world, target_oid) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(server_packets::confirm_dlg_text(&format!(
                "{reviver_name} is attempting to resurrect you, restoring {restore_exp} XP ({restore_percent:.0}%). Accept?"
            )));
        }
    }
}

/// `Player.reviveAnswer` — the corpse's `ConfirmDlg` reply.
///
/// Returns `true` when a pending proposal was consumed, so the shared
/// `DlgAnswer` dispatch knows this reply was ours and not the admin flow's.
pub(crate) fn handle_revive_answer(world: &mut World, player_oid: i32, accepted: bool) -> bool {
    let Some(request) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
        .and_then(|p| p.revive_request.take())
    else {
        return false;
    };
    // Java re-checks the corpse is still dead — it may have used "to village"
    // while the dialog sat on screen.
    // The corpse to revive: the pet when this was a pet proposal, else the
    // answering player themselves.
    let corpse_oid = if request.is_pet {
        match crate::game_loop::servitor::pet_of(world, player_oid) {
            Some(oid) => oid,
            None => return true, // the pet went away while the dialog sat open
        }
    } else {
        player_oid
    };
    if world
        .objects
        .get_component::<Vitals>(&corpse_oid)
        .is_none_or(|v| !v.dead)
    {
        return true;
    }
    if !accepted {
        return true;
    }
    if request.is_pet {
        revive_pet(
            world,
            player_oid,
            corpse_oid,
            request.restore_percent,
            request.hp_percent,
            request.mp_percent,
        );
        return true;
    }
    let crate::model::ReviveRequest {
        restore_percent,
        hp_percent,
        mp_percent,
        cp_percent,
        ..
    } = request;
    do_revive_with(
        world,
        player_oid,
        hp_percent,
        mp_percent,
        cp_percent,
        restore_percent,
    );
    true
}

/// `Player.doRevive(double revivePower)` — revive with the skill's own
/// percentages rather than the config respawn ones, and give back
/// `revivePower`% of the XP the death cost.
pub(crate) fn do_revive_with(
    world: &mut World,
    player_oid: i32,
    hp_percent: i32,
    mp_percent: i32,
    cp_percent: i32,
    restore_percent: f64,
) {
    do_revive(world, player_oid);
    let restored = {
        let Some((mut p, mut vitals, mut pvitals)) =
            world
                .objects
                .get_many_mut::<(&mut crate::model::Player, &mut Vitals, &mut PlayerVitals)>(
                    &player_oid,
                )
        else {
            return;
        };
        // The skill's percentages override `do_revive`'s config defaults. A
        // zero means "leave what the config gave", matching Java's
        // `if (reviveHp > 0)` guards.
        if hp_percent > 0 {
            vitals.cur_hp =
                (vitals.max_hp as f64 * hp_percent as f64 / 100.0).min(vitals.max_hp as f64);
        }
        if mp_percent > 0 {
            vitals.cur_mp =
                (vitals.max_mp as f64 * mp_percent as f64 / 100.0).min(vitals.max_mp as f64);
        }
        if cp_percent > 0 {
            pvitals.cur_cp =
                (pvitals.max_cp as f64 * cp_percent as f64 / 100.0).min(pvitals.max_cp as f64);
        }
        let restored = ((p.lost_exp_on_death as f64 * restore_percent) / 100.0).round() as i64;
        p.exp += restored;
        p.lost_exp_on_death = 0;
        restored
    };
    let _ = restored;
    crate::game_loop::party::broadcast_user_info(world, player_oid);
}

/// `Player.doRevive`: restore the configured percentages (`RespawnRestoreHP`
/// = 65% on the stock config) and broadcast `Revive`.
pub(crate) fn do_revive(world: &mut World, player_oid: i32) {
    {
        let Some((mut p, mut vitals, mut pvitals)) =
            world
                .objects
                .get_many_mut::<(&mut crate::model::Player, &mut Vitals, &mut PlayerVitals)>(
                    &player_oid,
                )
        else {
            return;
        };
        vitals.dead = false;
        p.pending_revive = false;
        let c = &world.cfg.character;
        if c.respawn_restore_hp > 0.0 {
            vitals.cur_hp =
                (vitals.max_hp as f64 * c.respawn_restore_hp / 100.0).min(vitals.max_hp as f64);
        }
        if c.respawn_restore_mp > 0.0 {
            vitals.cur_mp =
                (vitals.max_mp as f64 * c.respawn_restore_mp / 100.0).min(vitals.max_mp as f64);
        }
        if c.respawn_restore_cp > 0.0 {
            pvitals.cur_cp =
                (pvitals.max_cp as f64 * c.respawn_restore_cp / 100.0).min(pvitals.max_cp as f64);
        }
    }
    broadcast_including_self(world, player_oid, &server_packets::revive(player_oid));
    super::party::notify_party_vitals(world, player_oid);
    let (Some(vitals), Some(pvitals)) = (
        world.objects.get_component::<Vitals>(&player_oid).copied(),
        world
            .objects
            .get_component::<PlayerVitals>(&player_oid)
            .copied(),
    ) else {
        return;
    };
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[
                (
                    server_packets::status_update_type::CUR_HP,
                    vitals.cur_hp as i32,
                ),
                (
                    server_packets::status_update_type::CUR_MP,
                    vitals.cur_mp as i32,
                ),
                (
                    server_packets::status_update_type::CUR_CP,
                    pvitals.cur_cp as i32,
                ),
            ],
        ),
    );
}

/// `Pet.doRevive(revivePower)` — restore a share of the exp the death penalty
/// took, then bring the pet back.
///
/// Java's pet revive restores HP/MP by the skill's percentages like a player's,
/// but there is no CP on a pet.
fn revive_pet(
    world: &mut World,
    owner_oid: i32,
    pet_oid: i32,
    restore_percent: f64,
    hp_percent: i32,
    mp_percent: i32,
) {
    // `restoreExp` runs *before* `doRevive`, and consumes the record.
    crate::game_loop::servitor::pet_restore_exp(world, pet_oid, restore_percent);

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid) {
        v.dead = false;
        v.cur_hp = (v.max_hp as f64 * hp_percent as f64 / 100.0).max(1.0);
        v.cur_mp = v.max_mp as f64 * mp_percent as f64 / 100.0;
    }
    // The food clock stopped when the pet died; start it again.
    crate::game_loop::servitor::start_feed(world, pet_oid);
    crate::game_loop::servitor::send_pet_info(
        world,
        owner_oid,
        pet_oid,
        crate::game_loop::servitor::PetInfoKind::Default,
    );
    crate::game_loop::servitor::broadcast_summon_info(world, pet_oid, false);
    // The revived state is what should persist if the owner logs out now.
    crate::game_loop::servitor::sync_pet_row(world, owner_oid);
}

/// `calculateDistance3D(this) < ALT_PARTY_RANGE` — measured from the corpse.
fn in_range_of(world: &World, from: i32, to: i32, range: f64) -> bool {
    let (Some(a), Some(b)) = (
        world.objects.get_component::<Position>(&from),
        world.objects.get_component::<Position>(&to),
    ) else {
        return false;
    };
    let (dx, dy, dz) = ((a.x - b.x) as f64, (a.y - b.y) as f64, (a.z - b.z) as f64);
    (dx * dx + dy * dy + dz * dz).sqrt() < range
}

/// `Attackable.calculateRewards`' raid-point block.
///
/// Raid points are a separate currency from exp: they go to the **top damage
/// dealer** (or the last attacker if that player has gone), and when that
/// player is in a party they are **split among party members in range**, each
/// getting at least 1.
///
/// Two conditions that are easy to lose:
/// - `!_isRaidMinion` — a boss's adds award nothing, only the boss itself.
/// - the party split uses `ALT_PARTY_RANGE` from the **corpse**, so a member
///   who hung back out of range earns nothing.
fn award_raid_points(world: &mut World, npc_oid: i32, earner_oid: i32) {
    use crate::network::server_packets::{sm_ids, SmParam};

    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    let Some(t) = world.data.npc_data.get(npc.npc_id) else {
        return;
    };
    // Only a real raid boss, never its minions.
    if !matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss") || t.raid_points <= 0.0 {
        return;
    }
    if world
        .objects
        .has_component::<crate::game_loop::minions::MinionOf>(&npc_oid)
    {
        return;
    }
    let total = (t.raid_points * world.cfg.rates.rate_raidboss_points) as i32;

    // `broadcastPacket(CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL)` — everyone
    // present hears it, not just the earner.
    if let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::system_message_with(
                sm_ids::CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL,
                &[],
            ),
        );
    }

    // Party members within range of the corpse, else the earner alone.
    let range = world.cfg.character.alt_party_range as f64;
    let members: Vec<i32> = match world
        .objects
        .get_component::<crate::model::components::PartyRef>(&earner_oid)
        .map(|r| r.0)
        .and_then(|pid| world.parties.get(&pid))
    {
        Some(party) => party
            .members
            .iter()
            .copied()
            .filter(|m| in_range_of(world, npc_oid, *m, range))
            .collect(),
        None => vec![earner_oid],
    };
    if members.is_empty() {
        return;
    }
    // `Math.max(points / size, 1)` — a split never rounds anyone down to zero.
    let each = (total / members.len() as i32).max(1);
    for m in members {
        if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&m) {
            p.raidboss_points += each;
        }
        if let Some(cs) = client_for_player(world, m).and_then(|c| world.clients.get(&c)) {
            cs.send(server_packets::system_message_with(
                sm_ids::YOU_HAVE_EARNED_S1_RAID_POINTS,
                &[SmParam::Int(each)],
            ));
        }
    }
}
