//! The think state machine: intention dispatch, `thinkActive`
//! (aggro-range scan / most-hated pick / drift home / random walk) and
//! `thinkAttack` (chase, swing, cast ladder), plus mob-group control.

use super::*;
pub(super) fn think(world: &mut World, npc_oid: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    if is_dead(world, npc_oid) {
        return;
    }
    // A stunned/asleep/paralyzed mob does nothing at all — Java's `isDisabled()`
    // short-circuits `AttackableAI.onEvtThink`. A *rooted* one still thinks
    // (it can attack an adjacent target); the movement primitives refuse the
    // chase leg on their own.
    if abnormal::is_blocked_from_actions(world, npc_oid) {
        return;
    }
    // GM-controlled mobs run their own state machine (which itself reuses the
    // scan/attack/chase primitives below) rather than the wild AI.
    if let Some(group_id) = world
        .objects
        .get_component::<crate::model::mob_group::Controllable>(&npc_oid)
        .map(|c| c.group_id)
    {
        controllable_think(world, npc_oid, group_id);
        return;
    }
    // Only the Attackable subtree has this AI; the slice narrows to monsters —
    // plus town `Guard`s (Java `Guard extends Attackable`, so they run this same
    // AI; they're what hunts PKs) and stationed siege guards (`Defender`) while
    // their castle's siege runs, which use the same scan/attack/chase to defend
    // against attackers. Both facts are memoized on the `Npc` core — no
    // template lookup on the every-NPC-every-second path.
    if !npc.attackable_ai(world)
        && !(npc.is_defender(world) && siege::active_siege_guard_castle(world, npc_oid).is_some())
    {
        return;
    }
    // A servitor runs `SummonAI`, not `AttackableAI`: it trails its owner
    // instead of scanning for prey, and only fights what its owner points it
    // at. Once ordered, the ordinary attack think below drives it — "attack the
    // most-hated" is the right behaviour once the order has seeded the list.
    if world
        .objects
        .has_component::<crate::model::components::ServitorOf>(&npc_oid)
    {
        // A fetch errand outranks trailing the owner, so it thinks first and
        // suppresses the follow while it is running.
        if servitor::pet_pickup_think(world, npc_oid) {
            return;
        }
        servitor::servitor_follow_tick(world, npc_oid);
        if world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .is_some_and(|ai| ai.intention == NpcIntention::Attack)
        {
            think_attack(world, npc_oid);
        }
        return;
    }
    let Some(ai) = world.objects.get_component::<NpcAi>(&npc_oid) else {
        return;
    };
    match ai.intention {
        NpcIntention::Active => think_active(world, npc_oid),
        NpcIntention::Attack => think_attack(world, npc_oid),
        // `AttackableAI.onEvtThink`'s switch has no `AI_INTENTION_MOVE_TO`
        // case: a mob committed to a destination walk (today, a feared one)
        // thinks about nothing until it arrives. Without this arm the very
        // next think tick would re-issue a chase and cancel the flight.
        NpcIntention::MoveTo => {}
    }
}

/// How close a `Follow` member stays to its commander before it stops (Java's
/// `MobGroup` follow keeps ~offset spacing; a single range is enough here).
const FOLLOW_RANGE: f64 = 150.0;

/// Drive one GM-controlled mob per its group's [`MobGroupState`], reusing the
/// wild AI's scan/attack/chase (`think_active`/`think_attack`) for the combat
/// states and a plain walk for follow/return. Java's `ControllableMobAI` is a
/// parallel state machine; this collapses it onto the existing primitives.
pub(super) fn controllable_think(world: &mut World, npc_oid: i32, group_id: i32) {
    use crate::model::mob_group::MobGroupState;
    let Some(state) = world.mob_groups.get(&group_id).map(|g| g.state) else {
        return;
    };
    match state {
        MobGroupState::Idle | MobGroupState::NoMove => {
            stop_npc(world, npc_oid);
            clear_aggro(world, npc_oid);
        }
        MobGroupState::Random => {
            // The wild aggressive AI: same dispatch the non-controllable path runs.
            match world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .map(|ai| ai.intention)
            {
                Some(NpcIntention::Attack) => think_attack(world, npc_oid),
                _ => think_active(world, npc_oid),
            }
        }
        MobGroupState::Attack(target) | MobGroupState::Cast(target) => {
            seed_attack(world, npc_oid, target);
        }
        MobGroupState::AttackGroup(other) => {
            let victim = nearest_group_member(world, npc_oid, other);
            if let Some(v) = victim {
                seed_attack(world, npc_oid, v);
            } else {
                stop_npc(world, npc_oid);
            }
        }
        MobGroupState::Follow(commander) => {
            let Some((cx, cy, cz)) = pos_of(world, commander) else {
                return;
            };
            let dist = distance_2d(world, npc_oid, cx, cy);
            if dist > FOLLOW_RANGE && world.objects.get_component::<Movement>(&npc_oid).is_none() {
                move_npc_to(world, npc_oid, cx, cy, cz);
            } else if dist <= FOLLOW_RANGE {
                stop_npc(world, npc_oid);
            }
        }
        MobGroupState::Return(commander) => {
            if let Some((cx, cy, cz)) = pos_of(world, commander)
                && world.objects.get_component::<Movement>(&npc_oid).is_none()
            {
                move_npc_to(world, npc_oid, cx, cy, cz);
            }
        }
    }
}

/// Make the mob attack `target`: seed dominant hate and enter the attack loop
/// (reuses `think_attack`, so chase + swing are the wild AI's).
pub(crate) fn seed_attack(world: &mut World, npc_oid: i32, target: i32) {
    let target_alive = world
        .objects
        .get_component::<Vitals>(&target)
        .is_some_and(|v| !v.dead);
    if !target_alive {
        stop_npc(world, npc_oid);
        return;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.entry(target).or_default().hate = 1_000_000.0;
    }
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.intention = NpcIntention::Attack;
        ai.attack_timeout_tick = u64::MAX; // commanded attacks don't time out
    }
    think_attack(world, npc_oid);
}

/// The nearest live member of `group_id` to `npc_oid` (for `//mobgroup_attackgrp`).
fn nearest_group_member(world: &World, npc_oid: i32, group_id: i32) -> Option<i32> {
    let (nx, ny, _) = pos_of(world, npc_oid)?;
    world.mob_groups.get(&group_id).and_then(|g| {
        g.members
            .iter()
            .filter(|&&m| {
                world
                    .objects
                    .get_component::<Vitals>(&m)
                    .is_some_and(|v| !v.dead)
            })
            .min_by_key(|&&m| {
                pos_of(world, m)
                    .map(|(x, y, _)| ((x - nx) as i64).pow(2) + ((y - ny) as i64).pow(2))
                    .unwrap_or(i64::MAX)
            })
            .copied()
    })
}

pub(super) fn distance_2d(world: &World, oid: i32, x: i32, y: i32) -> f64 {
    world
        .objects
        .get_component::<Position>(&oid)
        .map(|p| (((p.x - x) as f64).powi(2) + ((p.y - y) as f64).powi(2)).sqrt())
        .unwrap_or(f64::MAX)
}

/// `AttackableAI.thinkActive`: tick `_globalAggro` toward 0, scan the aggro
/// range, pick the most hated, or drift back home.
pub(super) fn think_active(world: &mut World, npc_oid: i32) {
    let (aggressive, aggro_range) = {
        let npc_id = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .expect("caller checked")
            .npc_id;
        let ai = world
            .objects
            .get_component_mut::<NpcAi>(&npc_oid)
            .expect("caller checked");
        if ai.global_aggro != 0 {
            ai.global_aggro += if ai.global_aggro < 0 { 1 } else { -1 };
        }
        let t = world.data.npc_data.get(npc_id);
        // Java `npc.isAggressive()`: the explicit flag, not aggroRange —
        // nearly every passive mob in the datapack has an aggroRange too.
        //
        // `is_monster()` is the other half of the gate and it is load-bearing.
        // Java runs this scan for `isAggressive() || instanceof Guard`, but every
        // candidate then has to clear `isAggressiveTowards` → `isAutoAttackable`,
        // and `Player.isAutoAttackable` only returns true for an NPC attacker via
        // `attacker.isMonster()` — a `Guard` is an `Attackable`, not a `Monster`,
        // so it falls through the playable branches and returns false. The only
        // thing that makes a guard aggro a player is the `reputation < 0`
        // early-return in `isAggressiveTowards`, handled by `guard_aggro_scan`.
        // Town guards *are* `isAggressive="true"` in the datapack (all 186 of
        // them), so without this check every guard seeds hate on every lawful
        // player inside its 450-unit aggroRange and murders them.
        // `AttackableAI.autoAttackCondition`'s last gate before the
        // auto-attackable/line-of-sight test: `if (me.isChampion() &&
        // Config.CHAMPION_PASSIVE) return false`. With `ChampionPassive = True`
        // on this dist, a champion never seeds hate from the scan — it stands
        // where it spawned until something hits it, which is what stops a 10×-HP
        // mob from ambushing a passer-by.
        let champion_passive = world.cfg.champion.enable
            && world.cfg.champion.passive
            && world
                .objects
                .get_component::<crate::model::npc::Npc>(&npc_oid)
                .is_some_and(|n| n.champion);
        (
            t.map(|t| t.is_monster() && t.is_aggressive && t.aggro_range > 0)
                .unwrap_or(false)
                && !champion_passive
                // Java `Monster.isAggressive()`'s second term: a monster under
                // the `PASSIVE` flag (Veil 106, Requiem 1049) stops aggroing
                // whatever its template says (G34 S3).
                && !abnormal::is_pacified(world, npc_oid),
            t.map(|t| t.aggro_range).unwrap_or(0),
        )
    };
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };

    // `thinkActive` reads `_globalAggro` once, after the tick above, and wraps
    // *both* the aggro scan and the most-hated/attack decision in the same
    // `if (_globalAggro >= 0)`. While the counter is negative the mob is calm:
    // it neither seeds hate from proximity nor acts on hate it already carries
    // — the latter matters because hate can arrive without ever clearing the
    // counter (a faction call, a minion's master relaying, a script seeding the
    // list), and with only the scan gated a mob holding >10 hate would charge
    // straight out of its calm window. Java's `return` also sits *inside* that
    // block, so a calm mob falls through to the idle branches below and keeps
    // drifting/random-walking home rather than standing frozen over its list.
    let global_aggro = world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .map(|ai| ai.global_aggro)
        .unwrap_or(0);

    // Aggro-range scan (`isAggressiveTowards` narrowed: alive, in range,
    // geodata-visible; invisibility/silent-move/GM states don't exist).
    if aggressive && global_aggro >= 0 {
        let (nx, ny, nz) = {
            let pos = position(world, npc_oid);
            (pos.x, pos.y, pos.z)
        };
        let mut in_range = players_in_range_los(world, region, nx, ny, nz, aggro_range as f64);
        // Stealth / fake death (`isAggressiveTowards`).
        in_range.retain(|&pid| notices_target(world, npc_oid, pid));
        let mut newly_seen: Vec<i32> = Vec::new();
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
            for player_oid in in_range {
                // `addDamageHate(t, 0, 0)` → first sight seeds 1 hate.
                let entry = aggro.0.entry(player_oid).or_default();
                if entry.hate == 0.0 {
                    entry.hate = 1.0;
                    newly_seen.push(player_oid);
                }
            }
        }
        // `onAggroRangeEnter` for the scripts that registered this monster
        // (the Primeval Isle Tyrannosaurus's curiosity pause).
        if !newly_seen.is_empty() {
            let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
            for player_oid in newly_seen {
                crate::game_loop::quests::notify_aggro_range_enter(
                    world, npc_oid, npc_id, player_oid,
                );
            }
        }
    }

    // Town guards hunt PKs (`isAggressiveTowards`, the `me instanceof Guard`
    // branch): a guard aggros a player with **negative reputation** inside a
    // *hardcoded* 500 units — Java uses the literal, not the template's
    // `aggroRange` (which is 450 on the stock guards), and does it regardless of
    // the `isAggressive` flag. A lawful player is ignored, which is what makes
    // this a PK-hunting rule rather than general aggression — and it is the
    // *only* way a guard aggros a player, since the generic scan above is
    // monster-only (see the `is_monster()` note there).
    if world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .is_some_and(|n| n.is_guard(world))
    {
        guard_aggro_scan(world, npc_oid, region);
    }

    // Siege guards (`Defender`) defend the castle: they aggro their employer's
    // enemies within aggro range regardless of the `isAggressive` flag (Java
    // `SiegeGuardAI` — the guard's own aggro scan). Reuses the hate → attack →
    // chase machinery below. `aggro_range` comes from the template (1000 for the
    // stock guards); the enemy filter (anyone but a defender of this castle) is
    // `attackable_siege_guard`.
    if aggro_range > 0
        && let Some(_castle) = siege::active_siege_guard_castle(world, npc_oid)
    {
        let (nx, ny, nz) = {
            let pos = position(world, npc_oid);
            (pos.x, pos.y, pos.z)
        };
        let mut in_range = players_in_range_los(world, region, nx, ny, nz, aggro_range as f64);
        // Keep only actual enemies (attackers / non-defenders).
        in_range.retain(|&pid| siege::attackable_siege_guard(world, npc_oid, pid));
        in_range.retain(|&pid| notices_target(world, npc_oid, pid));
        set_hate_for(world, npc_oid, in_range);
    }

    // Chose a target from the aggro list (`getMostHated`, after the
    // per-entry `checkHate` liveness/region test). Inside the calm gate, as in
    // Java — `return` included.
    if global_aggro >= 0 {
        check_hate(world, npc_oid);
        let hated = world
            .objects
            .get_component::<AggroList>(&npc_oid)
            .and_then(AggroList::most_hated);
        if let Some(target) = hated {
            let aggro_list = world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .expect("checked");
            let aggro = aggro_list.0.get(&target).map(|a| a.hate).unwrap_or(0.0);
            if aggro + global_aggro as f64 > 0.0 {
                let became_running = {
                    let ai = world
                        .objects
                        .get_component_mut::<NpcAi>(&npc_oid)
                        .expect("checked");
                    ai.intention = NpcIntention::Attack;
                    ai.attack_timeout_tick = world.tick + ATTACK_TIMEOUT_TICKS;
                    let speeds = world
                        .objects
                        .get_component_mut::<Speeds>(&npc_oid)
                        .expect("checked");
                    let flip = !speeds.running;
                    speeds.running = true;
                    flip
                };
                if became_running {
                    broadcast_near_region_in(
                        world,
                        region,
                        instance_of(world, npc_oid),
                        &server_packets::change_move_type(npc_oid, true),
                    );
                }
            }
            return;
        }
    }

    // No target: either return to the spawn anchor when drifted too far
    // (`Config.MAX_DRIFT_RANGE`), or — while inside that radius — take an
    // occasional random walk (`AttackableAI.thinkActive`'s two idle branches).
    let max_drift = world.cfg.npc.max_drift_range as f64;
    let (x, y, z, spawn, moving, can_move, random_walk) = {
        let npc = &world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .expect("npc");
        let pos = world
            .objects
            .get_component::<Position>(&npc_oid)
            .expect("caller checked");
        let t = npc.template(world);
        let can_move = t.map(|t| t.can_move).unwrap_or(false);
        // Java `isRandomWalkingEnabled()`: the template flag (minions/walking-
        // route targets that clear it at runtime aren't in the monster slice).
        let random_walk = t.map(|t| t.random_walk).unwrap_or(false);
        // `ai/others/Spawns/NoRandomActivity` can clear it per NPC.
        let random_walk = spawn_scripts::random_walk_enabled(world, npc_oid, random_walk);
        (
            pos.x,
            pos.y,
            pos.z,
            npc.spawn_loc,
            world.objects.has_component::<Movement>(&npc_oid),
            can_move,
            random_walk,
        )
    };
    if !can_move || moving {
        return;
    }
    let dist_sq = ((spawn.0 - x) as f64).powi(2) + ((spawn.1 - y) as f64).powi(2);
    if dist_sq > max_drift * max_drift {
        // Drifted out of range with nothing to chase: walk back home.
        move_npc_to(world, npc_oid, spawn.0, spawn.1, spawn.2);
    } else if random_walk && world.roll(RANDOM_WALK_RATE) == 0 {
        random_walk_move(world, npc_oid, (x, y, z), spawn);
    }
}

/// `AttackableAI.thinkActive`'s random-walk branch: pick a point within
/// `MAX_DRIFT_RANGE` of the spawn anchor, geo-clamp the straight line to it,
/// and walk there — but only if the clamped spot is still within drift range.
fn random_walk_move(world: &mut World, npc_oid: i32, cur: (i32, i32, i32), spawn: (i32, i32, i32)) {
    let drift = world.cfg.npc.max_drift_range;
    // Java: deltaX ∈ [0, 2·drift); deltaY ∈ [deltaX, 2·drift] (Rnd.get(min,max)
    // is inclusive of max); then deltaY = √(deltaY² − deltaX²) so the offset
    // lands on a quarter arc of the drift circle around the spawn point.
    let delta_x = world.roll(drift * 2);
    let delta_y = delta_x + world.roll(drift * 2 - delta_x + 1);
    let delta_y = (((delta_y as f64).powi(2) - (delta_x as f64).powi(2)).max(0.0)).sqrt() as i32;
    let x1 = (delta_x + spawn.0) - drift;
    let y1 = (delta_y + spawn.1) - drift;
    let z1 = cur.2; // Java uses the NPC's current z, not the spawn z.

    let (vx, vy, vz) = world
        .geo
        .get_valid_location(cur.0, cur.1, cur.2, x1, y1, z1);
    // `Util.calculateDistance(spawn, moveLoc) <= MAX_DRIFT_RANGE`.
    let from_spawn_sq = ((vx - spawn.0) as f64).powi(2) + ((vy - spawn.1) as f64).powi(2);
    if from_spawn_sq <= (drift as f64) * (drift as f64) {
        move_npc_to(world, npc_oid, vx, vy, vz);
    }
}

/// `AttackableAI.thinkAttack`: validate the hated target, time out, chase,
/// swing.
pub(super) fn think_attack(world: &mut World, npc_oid: i32) {
    let now = world.tick;

    // `thinkAttack`'s very first line: `if ((npc == null) || npc.isCastingNow())
    // return;`. A mob mid-cast does nothing else — no faction call, no chase,
    // no swing — until the cast resolves. It went missing twice over, and each
    // time a different tail of the think ran anyway: the 1 s think landing
    // inside a 2 s cast fell through to the **swing** tail and the mob attacked
    // while casting, and it fell through to the **range** tail and re-issued
    // `chase()` every second, so the mob sprinted at its target with the cast
    // bar still up. Note `try_cast` above does refuse a second concurrent cast,
    // but it reports that as `false` = "no cast this think", which is exactly
    // what lets the caller carry on into both tails.
    if world.objects.has_component::<Casting>(&npc_oid) {
        return;
    }

    // Chase leash (`AttackableAI.thinkAttack` `AGGRO_DISTANCE_CHECK`): a monster
    // dragged farther than the configured range from its spawn drops all aggro,
    // heals to full and teleports home with its escort. On (2000/4000 units) on
    // this dist. Guards/defenders, route walkers and grand bosses are exempt.
    if world.cfg.npc.aggro_distance_check_enabled && npc_leash_return_home(world, npc_oid) {
        return;
    }

    check_hate(world, npc_oid);
    let target = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(AggroList::most_hated);
    let Some(target_oid) = target else {
        set_active(world, npc_oid);
        return;
    };

    // Target dead or gone → stop hating it (next think re-evaluates).
    let target_alive = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_some_and(|v| !v.dead);
    if !target_alive {
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
            aggro.0.remove(&target_oid);
        }
        return;
    }

    // Attack timeout (`thinkAttack`): give up the hunt — back to the scan
    // loop at walking speed. Java does *not* clear the aggro list here (the
    // `checkHate` region test is what ultimately forgets a vanished target);
    // instead a monster still mid-combat — or one nobody is left watching —
    // teleports straight back to its spawn
    // (`npc.teleToLocation(npc.getSpawn(), false)`).
    if world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .is_some_and(|ai| ai.attack_timeout_tick < now)
    {
        set_active(world, npc_oid);
        let Some(npc) = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
        else {
            return;
        };
        let spawn = npc.spawn_loc;
        let is_monster = npc.template(world).is_some_and(|t| t.is_monster());
        // `npc.isInCombat()` = `AttackStanceTaskManager.hasAttackStanceTask`.
        // Being *attacked* re-arms the attack timeout, so at this point only
        // the mob's own recent swings can still hold the stance.
        let in_combat = combat::has_attack_stance(world, npc_oid);
        let players_visible = {
            let Some(region) = region_cell_of(world, npc_oid) else {
                return;
            };
            // Index-derived (≤9 cells); like the sweep it replaced, unattended
            // shops count — a mob doesn't teleport home over an offline store.
            world.players_visible_from(region).next().is_some()
        };
        if is_monster && instance_of(world, npc_oid) == 0 && (in_combat || !players_visible) {
            let heading = world
                .objects
                .get_component::<Position>(&npc_oid)
                .map(|p| p.heading)
                .unwrap_or(0);
            death::relocate_npc(world, npc_oid, spawn.0, spawn.1, spawn.2, heading);
        }
        return;
    }

    // Mid-swing. In Java this is **not** a `thinkAttack` gate: the guard lives
    // in `Creature.doAutoAttack` (`isAttackDisabled()` = `isAttackingNow() ||
    // isDisabled()`), so a mob whose swing is still winding down keeps
    // thinking — it calls its faction, walks, and above all *casts*; only the
    // next swing is refused.
    //
    // Returning here instead cost the cast ladder most of its rolls. The AI
    // thinks once a second, plus once at each swing's end
    // (`ScheduledTask::NpcAttackReady`), but with this gate on top every
    // periodic think inside the swing window died before the ladder — leaving
    // exactly one `hasSkillChance()` roll per swing. At Porta's (20213) 253
    // atk. spd. that is one roll per ~2 s against Java's one per second, and
    // since the roll is only ~11 %, opportunities came ~18 s apart while its
    // Stun (4073) cooled down in 6 — so the SHORT_RANGE rung always had Stun
    // ready and the GENERAL rung that holds Summon (4161) was never reached.
    // Measured over 300 s of melee: 11 stuns, 0 summons.
    let mid_swing = world
        .objects
        .get_component::<AttackState>(&npc_oid)
        .is_some_and(|st| st.attack_end_tick > now);

    // "Actor should be able to see target" (`thinkAttack`'s geodata gate): a
    // sight line cut by a wall or a tower floor means no faction call, no
    // cast, no swing and — crucially — no straight-line chase. Java issues
    // `moveTo(target)`, an ordinary geo-validated walk that clamps at the
    // last walkable cell and falls back to the path worker (the stairs
    // route), then returns. Without this gate a mob whose hated target
    // climbed to another level engages straight through the geometry.
    {
        let (Some(npos), Some(tpos)) = (
            maybe_position(world, npc_oid),
            maybe_position(world, target_oid),
        ) else {
            return;
        };
        if !world
            .geo
            .can_see_target(npos.x, npos.y, npos.z, tpos.x, tpos.y, tpos.z)
        {
            let can_move = npc_template(world, npc_oid).is_some_and(|t| t.can_move);
            if can_move {
                move_npc_to(world, npc_oid, tpos.x, tpos.y, tpos.z);
            }
            return;
        }
    }

    // Call the faction for help before anything else this think (Java runs the
    // block right after the geodata check, ahead of the cast ladder).
    faction_call(world, npc_oid, target_oid);

    // The three movement blocks Java runs *between* the faction call and the
    // cast ladder. Each one ends the think when it fires.
    if shuffle_off_a_stacked_mob(world, npc_oid, target_oid) {
        return;
    }
    if archer_backs_off(world, npc_oid, target_oid) {
        return;
    }
    // Raid/minion target chaos can swap the target out from under the rest of
    // this think, so it is re-read afterwards rather than reusing `target_oid`.
    if raid_target_chaos(world, npc_oid) {
        return;
    }

    // Cast before closing distance — Java's "Cast skills" block sits between
    // the target checks and the range/move tail, so a caster that launched a
    // spell this think neither chases nor swings.
    if crate::game_loop::npc::cast::try_cast(world, npc_oid, target_oid) {
        return;
    }

    let Some(attacker) = combat::combatant(world, npc_oid) else {
        return;
    };
    let Some(victim) = combat::combatant(world, target_oid) else {
        return;
    };
    // `int range = npc.getPhysicalAttackRange() + combinedCollision; if
    // (getAiType() == ARCHER) range = 850 + combinedCollision;` — an archer
    // mob's *engagement* range is the flat bow range, not its template
    // `<attack range>` (40 on most of them). Without the override all 220
    // ARCHER templates on this dist walked into melee before shooting.
    let combined_collision = attacker.collision_radius + victim.collision_radius;
    let reach = if ai_type_of(world, npc_oid) == AiType::Archer {
        NPC_BOW_RANGE as f64 + combined_collision
    } else {
        attacker.atk_range as f64 + combined_collision
    };
    let dist_sq =
        ((victim.x - attacker.x) as f64).powi(2) + ((victim.y - attacker.y) as f64).powi(2);

    let can_move = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .expect("npc")
        .template(world)
        .map(|t| t.can_move)
        .unwrap_or(false);
    // Out of range: close, or — when the target has become unreachable — pick
    // another. `checkTarget` only fails for a dead target or an *immobilised*
    // mob that can't close (Java gates its range/LOS test on
    // `npc.isMovementDisabled()`), which is precisely the case that used to
    // leave a rooted mob standing still forever instead of switching to
    // whoever else was hitting it.
    //
    // Java then falls straight through to `doAutoAttack` with the new pick,
    // without re-testing the range — so does this.
    let mut target_oid = target_oid;
    if dist_sq > reach * reach {
        if can_move && check_target(world, npc_oid, target_oid) {
            chase(world, npc_oid, target_oid, reach);
            return;
        }
        match target_reconsider(world, npc_oid) {
            Some(t) => target_oid = t,
            None => return,
        }
    }

    // `Creature.doAutoAttack`'s `isAttackDisabled()` refusal — the swing that
    // is still running blocks the next one, and nothing else.
    if mid_swing {
        return;
    }

    // In reach: stop and swing.
    if world.objects.has_component::<Movement>(&npc_oid) {
        world.objects.remove_component::<Movement>(&npc_oid);
        let (Some(pos), Some(region)) = (
            maybe_position(world, npc_oid),
            region_cell_of(world, npc_oid),
        ) else {
            return;
        };
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading),
        );
    }
    combat::do_auto_attack(world, npc_oid, target_oid);
}
