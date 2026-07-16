//! Effect application: instant damage/heal effects, continuous (buff)
//! effects, and buff expiry.

use crate::game_loop::helpers::client_for_player;
use crate::model::components::{BaseStats, Buffs, CombatStats, RegionCell, Speeds, StatModifiers, Vitals};
use crate::model::formulas;
use crate::model::skill::{abnormal_type_client_id, ActiveBuff, RestorationGroup, Skill, SkillEffect};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;


/// The `callSkill` → `activateSkill` → effect-handler chain for the effect
/// kinds ported so far. Continuous stat modifiers land as an `ActiveBuff` on
/// the target; `MagicalAttack`/`Heal` are instant.
pub(crate) fn apply_skill_effects(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    use server_packets::{sm_ids, SmParam};

    // Magic crit is rolled once per cast (Java rolls in each instant effect's
    // `instant()`; one roll covers the single instant effect skills have).
    let m_crit_rate = world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_crit_hit).unwrap_or(0.0);
    let crit_roll = world.roll(1000);
    let mcrit = skill.magic_type == 1 && formulas::calc_magic_crit(m_crit_rate, skill.is_bad(), crit_roll);

    // Spiritshots (magic skills only, `useSpiritShot() == _magic == 1`): read
    // the charged flag once per cast for the damage/heal bonus; the shot is
    // spent below after every effect has been applied (Java `Skill` uncharges
    // post-`applyEffects`). `caster_is_player` stands in for `isMageClass()` in
    // the heal static bonus — this fn's caster is always a player.
    let caster_is_player = world.objects.get_component::<crate::model::Player>(&caster_oid).is_some();
    let (sps, bss) = if skill.magic_type == 1 {
        world
            .objects
            .get_component::<crate::model::Player>(&caster_oid)
            .map(|p| {
                (
                    p.is_charged_shot(crate::model::ShotType::Spiritshots),
                    p.is_charged_shot(crate::model::ShotType::BlessedSpiritshots),
                )
            })
            .unwrap_or((false, false))
    } else {
        (false, false)
    };
    let magic_shots_bonus = if bss { 4.0 } else if sps { 2.0 } else { 1.0 };

    for effect in &skill.effects {
        match effect {
            SkillEffect::MagicalAttack { power } => {
                let power = *power;
                let (m_atk, caster_name) = {
                    let m_atk =
                        world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                    (m_atk, world.objects.get_component::<crate::model::Player>(&caster_oid).expect("player").name.clone())
                };
                let m_def = target_m_def(world, target_oid);
                let damage = formulas::calc_magic_dam(m_atk, m_def, power, mcrit, magic_shots_bonus);
                apply_magic_damage(world, caster_oid, target_oid, damage, mcrit, &caster_name);
            }
            SkillEffect::Heal { power } => {
                let power = *power;
                let m_atk = world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                let amount = formulas::calc_heal(power, m_atk, mcrit, sps, bss, skill.mp_consume, caster_is_player);
                if crate::game_loop::combat::is_npc_oid(target_oid) {
                    // Healing an NPC: clamp and update, no system messages
                    // (nobody to send them to).
                    let hp = {
                        let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid)
                        else {
                            continue;
                        };
                        if vitals.dead {
                            continue;
                        }
                        vitals.cur_hp = (vitals.cur_hp + amount).min(vitals.max_hp as f64);
                        (vitals.cur_hp as i32, vitals.max_hp)
                    };
                    // `broadcastStatusUpdate` — refresh the HP bar for everyone
                    // watching the mob; without this the server-side heal is
                    // invisible to clients (the bar never moves).
                    if let Some(region) = world
                        .objects
                        .get_component::<RegionCell>(&target_oid)
                        .map(|r| r.0)
                    {
                        crate::game_loop::helpers::broadcast_near_region(
                            world,
                            region,
                            &server_packets::status_update(
                                target_oid,
                                &[
                                    (server_packets::status_update_type::MAX_HP, hp.1),
                                    (server_packets::status_update_type::CUR_HP, hp.0),
                                ],
                            ),
                        );
                    }
                    continue;
                }
                let healed = {
                    let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else { continue };
                    // Overheal clamp (`Heal.java`).
                    let amount = amount.min((vitals.max_hp as f64 - vitals.cur_hp).max(0.0));
                    vitals.cur_hp += amount;
                    amount
                };
                let caster_name = world.objects.get_component::<crate::model::Player>(&caster_oid).expect("player").name.clone();
                if let Some(client_id) = client_for_player(world, target_oid) {
                    if let Some(cs) = world.clients.get(&client_id) {
                        if target_oid != caster_oid {
                            cs.send(server_packets::system_message_with(
                                sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1,
                                &[SmParam::PlayerName(caster_name), SmParam::Int(healed as i32)],
                            ));
                        } else {
                            cs.send(server_packets::system_message_with(
                                sm_ids::S1_HP_HAS_BEEN_RESTORED,
                                &[SmParam::Int(healed as i32)],
                            ));
                        }
                        let cur_hp = world
                            .objects
                            .get_component::<Vitals>(&target_oid)
                            .map(|v| v.cur_hp as i32)
                            .unwrap_or(0);
                        cs.send(server_packets::status_update(
                            target_oid,
                            &[(server_packets::status_update_type::CUR_HP, cur_hp)],
                        ));
                    }
                    crate::game_loop::party::notify_party_vitals(world, target_oid);
                }
            }
            SkillEffect::GiveItem { item_id, item_count, item_enchant_level } => {
                give_item(world, target_oid, *item_id, *item_count, *item_enchant_level);
            }
            SkillEffect::GiveItemRandom { groups } => {
                give_item_random(world, target_oid, groups);
            }
            SkillEffect::EscapeToTown => {
                // `Escape.instant()` → `teleToLocation(TeleportWhereType.TOWN)`:
                // the enclosing map region's town respawn, random point when
                // `RandomRespawnInTownEnabled` (players only — NPCs never carry
                // this effect).
                if world.objects.get_component::<crate::model::Player>(&target_oid).is_some() {
                    let pos = world
                        .objects
                        .get_component::<crate::model::components::Position>(&target_oid)
                        .copied();
                    if let Some(pos) = pos {
                        let pick = if world.cfg.character.random_respawn_in_town {
                            world.roll(64) as usize
                        } else {
                            0
                        };
                        if let Some((x, y, z)) = world.data.map_region.town_respawn(pos.x, pos.y, pick) {
                            crate::game_loop::death::teleport_player(world, target_oid, x, y, z);
                        }
                    }
                }
            }
            SkillEffect::StatModifier(_) => {} // collected below
        }
    }

    // Spend the spiritshot now that every effect has been applied (Java
    // `Skill`: `unchargeShot(isChargedShot(BLESSED_SPIRITSHOTS) ? BLESSED : SPIRITSHOTS)`).
    if skill.magic_type == 1 && (sps || bss) {
        let shot = if bss { crate::model::ShotType::BlessedSpiritshots } else { crate::model::ShotType::Spiritshots };
        if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&caster_oid) {
            p.uncharge_shot(shot);
        }
    }

    // Continuous effects → one ActiveBuff on the target (`applyEffects`).
    let buff_effects = skill.stat_modifier_effects();
    if buff_effects.is_empty() {
        return;
    }
    // Java `EffectList` only schedules a stop task when the effect's time is
    // positive; a toggle or a 0-`abnormalTime` buff (e.g. Super Haste 7029,
    // `operateType=T`) persists until it's toggled/removed. Model that as a
    // sentinel expiry with no `BuffExpire` schedule, else it would vanish the
    // same tick it lands.
    let permanent = skill.abnormal_time <= 0;
    let expires_at_tick = if permanent { u64::MAX } else { world.tick + skill.abnormal_time as u64 * 10 };
    let buff = ActiveBuff {
        skill_id: skill.id,
        skill_level: skill.level,
        abnormal_type_client_id: abnormal_type_client_id(&skill.abnormal_type),
        expires_at_tick,
        passive: false,
        effects: buff_effects,
    };
    // NPC target: buffs modify the mob's server-side stats (no buff icons —
    // those are self-only — and no NpcInfo re-broadcast, so a speed change
    // isn't reflected client-side until respawn; the combat math uses it now).
    if crate::game_loop::combat::is_npc_oid(target_oid) {
        apply_buff_to_npc(world, target_oid, buff, skill.id);
        if !permanent {
            world
                .scheduler
                .schedule(expires_at_tick, ScheduledTask::BuffExpire { player_object_id: target_oid, skill_id: skill.id });
        }
        return;
    }
    {
        if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) = world
            .objects
            .get_many_mut::<(
                &mut crate::model::Player,
                &BaseStats,
                &mut StatModifiers,
                &crate::model::inventory::Inventory,
                &mut Buffs,
                &mut Speeds,
                &mut CombatStats,
            )>(&target_oid)
        {
            target.apply_buff(&world.data, &base, &mut mods, &inventory, &mut buffs, &mut speeds, &mut combat, buff);
        }
        if !permanent {
            world
                .scheduler
                .schedule(expires_at_tick, ScheduledTask::BuffExpire { player_object_id: target_oid, skill_id: skill.id });
        }
        let now = world.tick;
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(buffs) = world.objects.get_component::<Buffs>(&target_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(crate::network::enter_world::abnormal_status_update(buffs, now));
                }
                }
        }
        // A stat buff changed pAtk/pDef/speed/…; Java's `recalculateStats(true)`
        // follows with `broadcastUserInfo()`. Without this the client shows the
        // buff icon but never the changed stats or movement speed (and other
        // players never see the speed change).
        crate::game_loop::party::broadcast_user_info(world, target_oid);
    }
}

/// `handlers/effecthandlers/Restoration.java` — instant single-item grant.
/// Backs item-use skills wrapping a fixed pack/box reward (spiritshot packs,
/// jewelry boxes, …): the item's `<skills>` entry casts a skill with this
/// effect, and *that* is where the actual reward comes from — before this
/// was ported, such skills loaded with an empty effect list, so the item was
/// still consumed (`items::use_item_skills` destroys it once any skill
/// "lands") but granted nothing.
fn give_item(world: &mut World, target_oid: i32, item_id: i32, item_count: i64, item_enchant_level: i32) {
    use server_packets::sm_ids;

    if item_id <= 0 || item_count <= 0 {
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE, &[]));
            }
        }
        return;
    }
    // Java `Restoration`: `if (_itemEnchantmentLevel > 0) setEnchantLevel(...)`.
    grant_and_notify(world, target_oid, &[(item_id, item_count, item_enchant_level.max(0))]);
}

/// `handlers/effecthandlers/RestorationRandom.java` — one weighted roulette
/// pick among reward groups: walk `groups` accumulating `chance` until the
/// roll falls in a slice's `[chance_from, chance_from + chance)` range, then
/// grant every item in that slice's group together (Java: `100 *
/// Rnd.nextDouble()` against the raw 0-100 XML percentages).
fn give_item_random(world: &mut World, target_oid: i32, groups: &[RestorationGroup]) {
    use server_packets::sm_ids;

    let rnd_num = 100.0 * world.roll_f64();
    let mut chance_from = 0.0;
    let mut picked = None;
    for group in groups {
        if rnd_num >= chance_from && rnd_num <= chance_from + group.chance {
            picked = Some(&group.items);
            break;
        }
        chance_from += group.chance;
    }
    let Some(items) = picked else {
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE, &[]));
            }
        }
        return;
    };
    // Java `RestorationRandom`: roll `Rnd.get(minEnchant, maxEnchant)` (inclusive)
    // per created item when `maxEnchant > 0`, else no enchant.
    let grants: Vec<(i32, i64, i32)> = items
        .iter()
        .filter(|i| i.item_id > 0 && i.count > 0)
        .map(|i| {
            let enchant = if i.max_enchant > 0 {
                i.min_enchant + world.roll(i.max_enchant - i.min_enchant + 1)
            } else {
                0
            };
            (i.item_id, i.count, enchant)
        })
        .collect();
    grant_and_notify(world, target_oid, &grants);
}

/// Shared grant + `InventoryUpdate` + "You have obtained…" messaging tail for
/// `give_item`/`give_item_random` (Java: `Player.addItem` plus the
/// `sendMessage` helper both `Restoration` variants duplicate).
fn grant_and_notify(world: &mut World, target_oid: i32, grants: &[(i32, i64, i32)]) {
    use crate::model::inventory::Inventory;
    use server_packets::{sm_ids, SmParam};

    for &(item_id, amount, enchant) in grants {
        let Some(changed_oids) = crate::game_loop::items::add_inventory_item(world, target_oid, item_id, amount) else {
            continue;
        };
        // Stamp the rolled/fixed enchant onto the freshly created item(s). Only
        // non-stackable items carry an enchant; a stackable grant returns an
        // existing stack's oid, which must not be touched.
        if enchant > 0 && !world.data.item_data.get(item_id).map(|t| t.is_stackable).unwrap_or(false) {
            if let Some(inv) = world.objects.get_component_mut::<Inventory>(&target_oid) {
                for &oid in &changed_oids {
                    inv.set_item_enchant(oid, enchant);
                }
            }
        }
        let Some(inventory) = world.objects.get_component::<Inventory>(&target_oid) else { continue };
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                // Java `RestorationRandom.sendMessage`: count>1 → "obtained S2 S1";
                // single enchanted → "obtained a +S1 S2"; else "obtained S1".
                let sm = if amount > 1 {
                    server_packets::system_message_with(sm_ids::YOU_HAVE_OBTAINED_S2_S1, &[SmParam::ItemName(item_id), SmParam::Long(amount)])
                } else if enchant > 0 {
                    server_packets::system_message_with(sm_ids::YOU_HAVE_OBTAINED_A_S1_S2, &[SmParam::Int(enchant), SmParam::ItemName(item_id)])
                } else {
                    server_packets::system_message_with(sm_ids::YOU_HAVE_OBTAINED_S1, &[SmParam::ItemName(item_id)])
                };
                cs.send(sm);
                cs.send(crate::network::enter_world::inventory_update(inventory, &world.data, &changed_oids));
            }
        }
    }
}

/// The target-side `mDef` for the magic damage formula — players through
/// their stat pipeline, NPCs through the `MDefenseFinalizer` shape
/// (base × MEN bonus × level mod).
fn target_m_def(world: &World, target_oid: i32) -> f64 {
    if let Some(cs) = world.objects.get_component::<CombatStats>(&target_oid) {
        return cs.m_def;
    }
    // NPCs: memoized at spawn through the same MDefenseFinalizer shape.
    world.objects.get_component::<CombatStats>(&target_oid).map(|cs| cs.m_def).unwrap_or(1.0)
}

/// Port of `Creature.doAttack` → `reduceCurrentHp` for magic skill damage:
/// the caster-side messages here, the victim-side application (CP soak,
/// death, NPC hate/AI wake) shared with the auto-attack path in
/// `combat::apply_physical_damage`'s per-kind receivers.
pub(crate) fn apply_magic_damage(world: &mut World, caster_oid: i32, target_oid: i32, damage: f64, mcrit: bool, caster_name: &str) {
    use server_packets::{sm_ids, SmParam};

    let target_param = if let Some(p) = world.objects.get_component::<crate::model::Player>(&target_oid) {
        SmParam::PlayerName(p.name.clone())
    } else if let Some(t) = world.objects.get_component::<crate::model::npc::Npc>(&target_oid).and_then(|n| n.template(world)) {
        SmParam::NpcName(t.id)
    } else {
        return;
    };
    let dmg_int = damage as i32;

    if let Some(client_id) = client_for_player(world, caster_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            if mcrit {
                cs.send(server_packets::system_message_with(sm_ids::M_CRITICAL, &[]));
            }
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
                &[SmParam::PlayerName(caster_name.to_string()), target_param, SmParam::Int(dmg_int)],
            ));
        }
    }

    // Victim-side application: CP soak/HP/death/cast-break for players
    // (including the C1_HAS_RECEIVED message), hate + AI wake + death for
    // NPCs — the same receivers the auto-attack hits go through.
    crate::game_loop::combat::apply_physical_damage(world, caster_oid, target_oid, damage);
}

/// Land a buff on an NPC: store it (a re-cast of the same skill replaces the
/// old instance, like `EffectList`'s per-skill slot), recompute its stats, and
/// refresh the buff row in the target window of anyone watching it.
fn apply_buff_to_npc(world: &mut World, target_oid: i32, buff: ActiveBuff, skill_id: i32) {
    match world.objects.get_component_mut::<Buffs>(&target_oid) {
        Some(b) => {
            b.0.retain(|x| x.skill_id != skill_id);
            b.0.push(buff);
        }
        None => return,
    }
    recompute_npc_buffed_stats(world, target_oid);
    broadcast_target_buffs(world, target_oid);
}

/// Push a creature's current buffs to every player who has it targeted (Java
/// `EffectList.updateEffectIcons` → `ExAbnormalStatusUpdateFromTarget` to the
/// status listeners) — this is what draws the buff icons under a target's HP
/// bar. Used for NPC targets; players get their own self bar separately.
pub(crate) fn broadcast_target_buffs(world: &mut World, target_oid: i32) {
    let now = world.tick;
    let pkt = match world.objects.get_component::<Buffs>(&target_oid) {
        Some(buffs) => {
            crate::network::enter_world::ex_abnormal_status_update_from_target(target_oid, buffs, now)
        }
        None => return,
    };
    let mut observers: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &crate::model::components::TargetRef)>(|(p, t)| {
            if t.0 == Some(target_oid) {
                observers.push(p.object_id);
            }
        });
    for oid in observers {
        if let Some(cid) = client_for_player(world, oid) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(pkt.clone());
            }
        }
    }
}

/// Rebuild an NPC's combat stats from its template + current buffs (see
/// `model::recompute_npc_stats_from_buffs`). `world.data` and `world.objects`
/// are disjoint fields, so the template ref and the mutable component borrow
/// coexist.
fn recompute_npc_buffed_stats(world: &mut World, target_oid: i32) {
    let Some(npc_id) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .map(|n| n.npc_id)
    else {
        return;
    };
    let Some(t) = world.data.npc_data.get(npc_id) else { return };
    let sb = &world.data.stat_bonus;
    let caps = &world.data.combat_caps;
    if let Some((buffs, mut combat, mut speeds)) = world
        .objects
        .get_many_mut::<(&Buffs, &mut CombatStats, &mut Speeds)>(&target_oid)
    {
        crate::model::recompute_npc_stats_from_buffs(t, sb, caps, buffs, &mut combat, &mut speeds);
    }
}

/// `BuffFinishTask`, fired when a buff's `abnormalTime` elapses
/// (`ScheduledTask::BuffExpire`). A buff already gone (re-cast/replaced) is a
/// no-op, matching the scheduler's dead-id contract.
pub(crate) fn handle_buff_expire(world: &mut World, player_object_id: i32, skill_id: i32) {
    let still_active = world
        .objects
        .get_component::<Buffs>(&player_object_id)
        .is_some_and(|b| b.0.iter().any(|b| b.skill_id == skill_id));
    if !still_active {
        return;
    }
    // NPC: drop the buff and recompute from the template (no icons/broadcast).
    if crate::game_loop::combat::is_npc_oid(player_object_id) {
        if let Some(b) = world.objects.get_component_mut::<Buffs>(&player_object_id) {
            b.0.retain(|x| x.skill_id != skill_id);
        }
        recompute_npc_buffed_stats(world, player_object_id);
        broadcast_target_buffs(world, player_object_id);
        return;
    }
    if let Some((player, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) = world
        .objects
        .get_many_mut::<(
            &mut crate::model::Player,
            &BaseStats,
            &mut StatModifiers,
            &crate::model::inventory::Inventory,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&player_object_id)
    {
        player.remove_buff(&world.data, &base, &mut mods, &inventory, &mut buffs, &mut speeds, &mut combat, skill_id);
    }
    let now = world.tick;
    // Removing the buff reverted its stat contribution — rebroadcast so the
    // client (and nearby players, for speed) see the stats return to normal.
    crate::game_loop::party::broadcast_user_info(world, player_object_id);
    let Some(client_id) = client_for_player(world, player_object_id) else { return };
    if let Some(buffs) = world.objects.get_component::<Buffs>(&player_object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::enter_world::abnormal_status_update(buffs, now));
        }
    }
}

