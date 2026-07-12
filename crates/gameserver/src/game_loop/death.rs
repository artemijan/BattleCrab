//! Death, decay/respawn, rewards (XP/SP/level-ups, drops), and the
//! die → "to village" → teleport → revive loop (G9).
//!
//! Java counterparts: `Creature.doDie`/`Npc.doDie`/`Player.doDie`,
//! `DecayTaskManager`/`RespawnTaskManager`/`Spawn.decreaseCount`,
//! `Attackable.calculateRewards` + `NpcTemplate.calculateDrops`,
//! `PlayerStat.addExpAndSp`/`addLevel`, `Player.calculateDeathExpPenalty`,
//! `RequestRestartPoint`/`Appearing`/`Player.doRevive`.

use crate::data::npc_data::{DropHolder, NpcTemplate};
use crate::model::components::{BaseStats, Intent, Movement, PlayerVitals, Position, RegionCell, SkillBook, Speeds, StatModifiers, Vitals};
use crate::model::formulas;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::{regions_adjacent, World};

use super::helpers::{broadcast_including_self, broadcast_near_region, client_for_player};

/// `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

// ---------------------------------------------------------------------------
// NPC death → decay → respawn
// ---------------------------------------------------------------------------

/// `Npc/Attackable.doDie`: mark dead, hand out rewards, broadcast `Die`,
/// schedule the decay task.
pub(crate) fn npc_do_die(world: &mut World, npc_oid: i32, killer_oid: i32) {
    let (corpse_secs, max_hp) = {
        let Some((npc, mut vitals)) =
            world.objects.get_many_mut::<(&mut crate::model::npc::Npc, &mut Vitals)>(&npc_oid)
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
    let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) else { return };

    calculate_rewards(world, npc_oid, killer_oid);

    // `setCurrentHp(0)` broadcasts the final StatusUpdate before `Die` —
    // without it the target window keeps the last non-zero HP.
    broadcast_near_region(
        world,
        region,
        &server_packets::status_update(
            npc_oid,
            &[
                (server_packets::status_update_type::MAX_HP, max_hp),
                (server_packets::status_update_type::CUR_HP, 0),
            ],
        ),
    );
    broadcast_near_region(world, region, &server_packets::die(npc_oid, false));
    world
        .scheduler
        .schedule(world.tick + corpse_secs.max(0) as u64 * 10, ScheduledTask::NpcDecay { npc_object_id: npc_oid });
}

/// `DecayTaskManager` firing → `Npc.onDecay` + `Spawn.decreaseCount`: remove
/// the corpse from the world and schedule the respawn.
pub(crate) fn handle_npc_decay(world: &mut World, npc_oid: i32) {
    let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) else { return };
    // Gather the respawn bookkeeping before despawn (components drop with
    // the entity).
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).cloned() else { return };
    world.objects.despawn(&npc_oid);
    if let Some(ids) = world.npc_regions.get_mut(&region) {
        ids.retain(|&id| id != npc_oid);
    }
    broadcast_near_region(world, region, &server_packets::delete_object(npc_oid));
    world.objects.for_each_mut::<&mut crate::model::components::TargetRef>(|mut t| {
        if t.0 == Some(npc_oid) {
            t.0 = None;
        }
    });

    // `Spawn.decreaseCount`: respawn only when the spawn line asked for it
    // (`_doRespawn = respawnMinDelay > 0`), with the ± random spread.
    if npc.respawn_secs > 0 {
        let min = (npc.respawn_secs - npc.respawn_random_secs).max(0);
        let max = npc.respawn_secs + npc.respawn_random_secs;
        let delay_secs = if max > min { min + world.roll(max - min + 1) } else { min };
        let (spawn_idx, group_idx, npc_idx) = npc.spawn_ref;
        world
            .scheduler
            .schedule(world.tick + delay_secs as u64 * 10, ScheduledTask::NpcRespawn { spawn_idx, group_idx, npc_idx });
    }
}

/// `RespawnTaskManager` firing → `Spawn.respawnNpc`: re-run the spawn line
/// and introduce the fresh NPC to nearby players.
pub(crate) fn handle_npc_respawn(world: &mut World, spawn_idx: usize, group_idx: usize, npc_idx: usize) {
    let Some(object_id) = crate::model::npc::spawn_one(world, spawn_idx, group_idx, npc_idx) else { return };
    let Some(v) = crate::model::npc::NpcView::of(&world.objects, object_id) else { return };
    let Some(region) = world.objects.get_component::<RegionCell>(&object_id).map(|r| r.0) else { return };
    let Some(t) = v.npc.template(world) else { return };
    let pkt = server_packets::npc_info(&v, t);
    broadcast_near_region(world, region, &pkt);
}

// ---------------------------------------------------------------------------
// Rewards (`Attackable.calculateRewards`)
// ---------------------------------------------------------------------------

/// XP/SP shares from the aggro list + drops to the top damage dealer.
/// Narrowings: no parties (each attacker is rewarded solo), no overhit bonus
/// (no overhit skills), no raid points, no champion mods.
fn calculate_rewards(world: &mut World, npc_oid: i32, killer_oid: i32) {
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid) else { return };
    let Some(t) = npc.template(world).cloned() else { return };
    let Some(npc_region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) else { return };
    let (nx, ny) = {
        let Some(pos) = world.objects.get_component::<Position>(&npc_oid) else { return };
        (pos.x, pos.y)
    };

    // Damage shares (players only, > 1 damage, still within reward range —
    // `Util.checkIfInRange(ALT_PARTY_RANGE, …)`).
    let reward_range = world.cfg.character.alt_party_range as f64;
    let mut shares: Vec<(i32, f64)> = Vec::new();
    let mut total_damage = 0.0;
    let mut max_dealer: Option<(i32, f64)> = None;
    let Some(aggro) = world.objects.get_component::<crate::model::npc::AggroList>(&npc_oid) else { return };
    for (&player_oid, info) in &aggro.0 {
        let Some(ppos) = world.objects.get_component::<Position>(&player_oid) else { continue };
        if info.damage <= 1.0 {
            continue;
        }
        let dist = (((ppos.x - nx) as f64).powi(2) + ((ppos.y - ny) as f64).powi(2)).sqrt();
        if dist > reward_range {
            continue;
        }
        total_damage += info.damage;
        shares.push((player_oid, info.damage));
        if max_dealer.is_none_or(|(_, d)| info.damage > d) {
            max_dealer = Some((player_oid, info.damage));
        }
    }

    // Drops go to the top damage dealer (fall back to the killer).
    let looter = max_dealer.map(|(id, _)| id).or_else(|| world.objects.has_component::<crate::model::Player>(&killer_oid).then_some(killer_oid));
    if let Some(looter) = looter {
        let drops = roll_drops(world, &t, looter);
        for (item_id, count) in drops {
            give_item(world, looter, item_id, count);
        }
    }

    if total_damage <= 0.0 {
        return;
    }
    // `calculateExpAndSp` per attacker: template reward × rate × damage
    // share × level-gap multiplier.
    let (rate_xp, rate_sp) = (world.cfg.rates.rate_xp, world.cfg.rates.rate_sp);
    for (player_oid, damage) in shares {
        let Some(p) = world.objects.get_component::<crate::model::Player>(&player_oid) else { continue };
        let Some(pregion) = world.objects.get_component::<RegionCell>(&player_oid).map(|r| r.0) else { continue };
        if !regions_adjacent(npc_region, pregion) {
            continue; // Java `isInSurroundingRegion(attacker)`.
        }
        let gap = formulas::exp_sp_level_gap_multiplier(p.level, t.level);
        let exp = (t.exp * rate_xp * damage / total_damage * gap).max(0.0);
        let sp = (t.sp * rate_sp * damage / total_damage * gap).max(0.0);
        add_exp_and_sp(world, player_oid, exp.round() as i64, sp.round() as i64);
    }
}

/// `NpcTemplate.calculateDrops(DROP)` — grouped + ungrouped lists with the
/// level-gap gates, per-item rate multipliers, and the max-occurrence cap.
/// The temporary "replace highest-chance random drop" reshuffling Java does
/// when the cap is hit mid-list is simplified to a hard stop at the cap.
fn roll_drops(world: &mut World, t: &NpcTemplate, killer_oid: i32) -> Vec<(i32, i64)> {
    let Some(killer) = world.objects.get_component::<crate::model::Player>(&killer_oid) else { return Vec::new() };
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
            let gap = if drop.item_id == ADENA_ID { adena_gap_chance } else { item_gap_chance };
            if world.roll_f64() * 100.0 > gap {
                continue;
            }
            // Chance roll (grouped items fold the group chance in).
            let rate_chance = by_id_chance.get(&drop.item_id).copied().unwrap_or(chance_mult);
            let chance = drop.chance * (group_chance / 100.0) * rate_chance;
            if world.roll_f64() * 100.0 >= chance {
                continue;
            }
            // Amount.
            let rate_amount = by_id_amount.get(&drop.item_id).copied().unwrap_or(amount_mult);
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

/// `Player.addItem` (auto-loot path): stack or create, persist, notify.
/// Ground drops (`AutoLoot = False`) are not ported — with the dist config's
/// `AutoLoot = True` this is the live path.
pub(crate) fn give_item(world: &mut World, player_oid: i32, item_id: i32, count: i64) {
    if !world.cfg.character.auto_loot {
        return; // ground-drop path unported (see PROGRESS G9 notes).
    }
    let stackable = world.data.item_data.get(item_id).map(|t| t.is_stackable).unwrap_or(false);
    let existing_stack = stackable
        .then(|| {
            world
                .objects
                .get_component::<crate::model::inventory::Inventory>(&player_oid)
                .and_then(|inv| inv.items().iter().find(|i| i.item_id == item_id).map(|i| i.object_id))
        })
        .flatten();

    let changed_oid = if let Some(stack_oid) = existing_stack {
        let new_count = {
            let inv = world
                .objects
                .get_component_mut::<crate::model::inventory::Inventory>(&player_oid)
                .expect("checked");
            inv.add_item(&world.data.item_data, stack_oid, item_id, count);
            inv.items().iter().find(|i| i.object_id == stack_oid).map(|i| i.count).unwrap_or(count)
        };
        let _ = world.db.send(crate::db::DbCommand::UpdateItemCount { object_id: stack_oid, count: new_count });
        stack_oid
    } else {
        let Some(new_oid) = world.alloc_object_id() else {
            tracing::warn!("give_item: object-id pool exhausted, dropping loot {item_id}×{count}");
            return;
        };
        let Some(inv) = world.objects.get_component_mut::<crate::model::inventory::Inventory>(&player_oid) else {
            return;
        };
        inv.add_item(&world.data.item_data, new_oid, item_id, count);
        let _ = world.db.send(crate::db::DbCommand::InsertItem { owner_id: player_oid, object_id: new_oid, item_id, count });
        new_oid
    };

    // "You have obtained …" + InventoryUpdate + the weight/adena footers.
    let Some(client_id) = client_for_player(world, player_oid) else { return };
    let Some(inventory) = world.objects.get_component::<crate::model::inventory::Inventory>(&player_oid) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        let sm = if item_id == ADENA_ID {
            server_packets::system_message_with(sm_ids::YOU_HAVE_OBTAINED_S1_ADENA, &[SmParam::Long(count)])
        } else if count > 1 {
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_OBTAINED_S2_S1,
                &[SmParam::ItemName(item_id), SmParam::Long(count)],
            )
        } else {
            server_packets::system_message_with(sm_ids::YOU_HAVE_OBTAINED_S1, &[SmParam::ItemName(item_id)])
        };
        cs.send(sm);
        cs.send(crate::network::enter_world::inventory_update(inventory, &world.data, &[changed_oid]));
    }
}

// ---------------------------------------------------------------------------
// XP/SP gain and level-ups (`PlayableStat.addExp` / `PlayerStat.addLevel`)
// ---------------------------------------------------------------------------

/// `PlayerStat.addExpAndSp` (no bonus multipliers — vitality/rods are later
/// milestones, so bonus == base and the SM shows 0 bonus like Java does
/// then).
pub(crate) fn add_exp_and_sp(world: &mut World, player_oid: i32, exp: i64, sp: i64) {
    let max_level = world.data.experience.max_level as i32;
    let cap = world.data.experience.exp_for_level(max_level) - 1;
    let (old_level, new_exp) = {
        let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&player_oid) else { return };
        p.exp = (p.exp + exp.max(0)).min(cap);
        p.sp = p.sp.saturating_add(sp.max(0));
        (p.level, p.exp)
    };
    if exp > 0 || sp > 0 {
        if let Some(client_id) = client_for_player(world, player_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4,
                    &[SmParam::Long(exp), SmParam::Long(0), SmParam::Long(sp), SmParam::Long(0)],
                ));
            }
        }
    }

    let new_level = level_for_exp(world, new_exp, max_level);
    if new_level != old_level {
        set_level(world, player_oid, new_level);
    } else if let Some(client_id) = client_for_player(world, player_oid) {
        // Exp bar refresh (`player.updateUserInfo()`).
        if let (Some(v), Some(cs)) =
            (crate::model::PlayerView::of(&world.objects, player_oid), world.clients.get(&client_id))
        {
            cs.send(crate::network::user_info::user_info(&v, &world.data));
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
fn set_level(world: &mut World, player_oid: i32, new_level: i32) {
    let leveled_up = {
        let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&player_oid) else { return };
        let up = new_level > p.level;
        p.level = new_level;
        up
    };
    // Vitals follow the level tables (`getMaxHp` etc. read level).
    {
        let data = &world.data;
        let Some((p, mut vitals, mut pvitals, base, mods, mut speeds, mut combat)) = world
            .objects
            .get_many_mut::<(
                &mut crate::model::Player,
                &mut Vitals,
                &mut PlayerVitals,
                &BaseStats,
                &StatModifiers,
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
        vitals.max_hp = crate::model::calc_max_hp(data, &t, p.level) as i32;
        vitals.max_mp = crate::model::calc_max_mp(data, &t, p.level) as i32;
        pvitals.max_cp = crate::model::calc_max_cp(data, &t, p.level) as i32;
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
        p.recalculate_stats(data, &base, &mods, &mut speeds, &mut combat);
    }

    // `rewardSkills`: grant every autoGet skill now reachable.
    let (class_id, level, mut known) = {
        let p = &world.objects.get_component::<crate::model::Player>(&player_oid).expect("player");
        let skills = world.objects.get_component::<SkillBook>(&player_oid).cloned().unwrap_or_default();
        (p.class_id, p.level, skills.0)
    };
    let mut granted = Vec::new();
    for learn in world.data.skill_trees.auto_get_skills(class_id, level) {
        let known_level = known.get(&learn.skill_id).copied().unwrap_or(0);
        if learn.skill_level > known_level {
            known.insert(learn.skill_id, learn.skill_level);
            granted.push((learn.skill_id, learn.skill_level));
        }
    }
    if !granted.is_empty() {
        if let Some(book) = world.objects.get_component_mut::<SkillBook>(&player_oid) {
            for &(id, lvl) in &granted {
                book.0.insert(id, lvl);
            }
        }
        for (id, lvl) in granted {
            let _ = world.db.send(crate::db::DbCommand::UpsertSkill { char_id: player_oid, skill_id: id, skill_level: lvl });
            // `updateShortCuts` — panel slots holding the skill pick up the
            // auto-granted level.
            super::shortcuts::update_skill_shortcuts(world, player_oid, id, lvl);
        }
    }

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
        world.objects.get_component::<PlayerVitals>(&player_oid).copied(),
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
                (server_packets::status_update_type::CUR_HP, vitals.cur_hp as i32),
                (server_packets::status_update_type::MAX_MP, vitals.max_mp),
                (server_packets::status_update_type::CUR_MP, vitals.cur_mp as i32),
                (server_packets::status_update_type::MAX_CP, pvitals.max_cp),
                (server_packets::status_update_type::CUR_CP, pvitals.cur_cp as i32),
            ],
        ),
    );
    if let Some(client_id) = client_for_player(world, player_oid) {
        if let (Some(v), Some(cs)) =
            (crate::model::PlayerView::of(&world.objects, player_oid), world.clients.get(&client_id))
        {
            if leveled_up {
                cs.send(server_packets::system_message_with(sm_ids::YOUR_LEVEL_HAS_INCREASED, &[]));
            }
            cs.send(crate::network::user_info::user_info(&v, &world.data));
            let Some(skills) = world.objects.get_component::<SkillBook>(&player_oid) else { return };
            cs.send(crate::network::enter_world::skill_list(skills, &world.data));
        }
    }
}

// ---------------------------------------------------------------------------
// Player death + revive
// ---------------------------------------------------------------------------

/// `Player.doDie`: mark dead, stop everything, apply the XP penalty,
/// broadcast `Die` with the to-village flag.
pub(crate) fn player_do_die(world: &mut World, player_oid: i32, killer_oid: i32) {
    {
        let Some((p, mut vitals)) =
            world.objects.get_many_mut::<(&mut crate::model::Player, &mut Vitals)>(&player_oid)
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
        world.objects.remove_component::<crate::model::components::QueuedAction>(&player_oid);
        if let Some(t) = world.objects.get_component_mut::<crate::model::components::TargetRef>(&player_oid) {
            t.0 = None;
        }
    }
    // Any cast dies with the caster (`abortCast`; also stops pre-launch
    // packets via the seq mismatch).
    super::skills::cast::abort_cast(world, player_oid);

    // Death XP penalty (`calculateDeathExpPenalty` — raid/pvp-zone modifiers
    // don't apply: the only killers are plain monsters).
    let _ = killer_oid;
    apply_death_exp_penalty(world, player_oid);

    broadcast_including_self(world, player_oid, &server_packets::die(player_oid, true));
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(player_oid, &[(server_packets::status_update_type::CUR_HP, 0)]),
    );
}

/// `Player.calculateDeathExpPenalty` + `PlayableStat.removeExp` (with the
/// `Delevel`/`DelevelMinimum` clamping) + the SM 539 notice.
fn apply_death_exp_penalty(world: &mut World, player_oid: i32) {
    let (level, exp) = {
        let Some(p) = world.objects.get_component::<crate::model::Player>(&player_oid) else { return };
        (p.level, p.exp)
    };
    let max_level = world.data.experience.max_level as i32;
    let percent = world.data.xp_lost.xp_percent(level);
    let (lo, hi) = if level < max_level {
        (world.data.experience.exp_for_level(level), world.data.experience.exp_for_level(level + 1))
    } else {
        (world.data.experience.exp_for_level(max_level - 1), world.data.experience.exp_for_level(max_level))
    };
    let mut lost = (((hi - lo) as f64) * percent / 100.0).round() as i64;

    // `removeExp`'s delevel clamp: without delevel (or at/below the floor)
    // exp can't drop below the current level's threshold.
    let can_delevel = world.cfg.character.player_delevel && level > world.cfg.character.delevel_minimum;
    if !can_delevel {
        lost = lost.min(exp - world.data.experience.exp_for_level(level));
    }
    lost = lost.min(exp - 1).max(0);
    if lost == 0 {
        return;
    }

    let new_exp = exp - lost;
    if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&player_oid) {
        p.exp = new_exp;
    }
    if let Some(client_id) = client_for_player(world, player_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_ids::YOUR_XP_HAS_DECREASED_BY_S1, &[SmParam::Long(lost)]));
        }
    }
    let new_level = level_for_exp(world, new_exp, max_level);
    if new_level != level {
        set_level(world, player_oid, new_level);
    }
}

/// Port of `clientpackets/RequestRestartPoint`, TO_VILLAGE only (every other
/// restart type needs clans/sieges): pick the town respawn and start the
/// teleport; the revive itself lands on `Appearing`.
pub(crate) fn handle_request_restart_point(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(_pkt) = cp::RequestRestartPoint::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let (px, py, dead) = {
        let Some(pos) = world.objects.get_component::<Position>(&object_id) else { return };
        let Some(vitals) = world.objects.get_component::<Vitals>(&object_id) else { return };
        (pos.x, pos.y, vitals.dead)
    };
    if !dead {
        return;
    }
    let pick = if world.cfg.character.random_respawn_in_town { world.roll(64) as usize } else { 0 };
    let Some((x, y, z)) = world.data.map_region.town_respawn(px, py, pick) else { return };
    if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&object_id) {
        p.pending_revive = true;
    }
    teleport_player(world, object_id, x, y, z);
}

/// `Creature.teleToLocation`: stop moving, vanish from the old neighborhood
/// (`decayMe` → `DeleteObject`), push the new position, and wait for the
/// client's `Appearing` before becoming visible again.
pub(crate) fn teleport_player(world: &mut World, player_oid: i32, x: i32, y: i32, z: i32) {
    if world.objects.get_component::<crate::model::Player>(&player_oid).is_none() {
        return;
    }
    world.objects.remove_component::<Movement>(&player_oid);
    world.objects.remove_component::<Intent>(&player_oid);
    world.objects.remove_component::<crate::model::components::QueuedAction>(&player_oid);
    let Some(heading) = world.objects.get_component::<Position>(&player_oid).map(|p| p.heading) else { return };
    broadcast_including_self(world, player_oid, &server_packets::teleport_to_location(player_oid, x, y, z, heading));
    // `decayMe`: DeleteObject to everyone who could see the old position
    // (also drops their dangling targets).
    super::visibility::on_leave_world(world, player_oid);
    if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&player_oid) {
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
}

/// Port of `clientpackets/Appearing`: the client finished loading after a
/// teleport — `onTeleported` (spawnMe → mutual CharInfo/NpcInfo, pending
/// revive resolves, fresh `UserInfo`).
pub(crate) fn handle_appearing(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    if !world.objects.get_component::<crate::model::Player>(&object_id).is_some_and(|p| p.teleporting) {
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&object_id) {
        p.teleporting = false;
    }
    if world.objects.get_component::<crate::model::Player>(&object_id).is_some_and(|p| p.pending_revive) {
        do_revive(world, object_id);
    }
    // `spawnMe`-equivalent visibility exchange at the new position.
    super::visibility::on_enter_world(world, client_id, object_id);
    if let (Some(v), Some(cs)) =
        (crate::model::PlayerView::of(&world.objects, object_id), world.clients.get(&client_id))
    {
        cs.send(crate::network::user_info::user_info(&v, &world.data));
    }
}

/// `Player.doRevive`: restore the configured percentages (`RespawnRestoreHP`
/// = 65% on the stock config) and broadcast `Revive`.
fn do_revive(world: &mut World, player_oid: i32) {
    {
        let Some((mut p, mut vitals, mut pvitals)) = world
            .objects
            .get_many_mut::<(&mut crate::model::Player, &mut Vitals, &mut PlayerVitals)>(&player_oid)
        else {
            return;
        };
        vitals.dead = false;
        p.pending_revive = false;
        let c = &world.cfg.character;
        if c.respawn_restore_hp > 0.0 {
            vitals.cur_hp = (vitals.max_hp as f64 * c.respawn_restore_hp / 100.0).min(vitals.max_hp as f64);
        }
        if c.respawn_restore_mp > 0.0 {
            vitals.cur_mp = (vitals.max_mp as f64 * c.respawn_restore_mp / 100.0).min(vitals.max_mp as f64);
        }
        if c.respawn_restore_cp > 0.0 {
            pvitals.cur_cp = (pvitals.max_cp as f64 * c.respawn_restore_cp / 100.0).min(pvitals.max_cp as f64);
        }
    }
    broadcast_including_self(world, player_oid, &server_packets::revive(player_oid));
    let (Some(vitals), Some(pvitals)) = (
        world.objects.get_component::<Vitals>(&player_oid).copied(),
        world.objects.get_component::<PlayerVitals>(&player_oid).copied(),
    ) else {
        return;
    };
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[
                (server_packets::status_update_type::CUR_HP, vitals.cur_hp as i32),
                (server_packets::status_update_type::CUR_MP, vitals.cur_mp as i32),
                (server_packets::status_update_type::CUR_CP, pvitals.cur_cp as i32),
            ],
        ),
    );
}
