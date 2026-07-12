//! Effect application: instant damage/heal effects, continuous (buff)
//! effects, and buff expiry.

use crate::game_loop::helpers::client_for_player;
use crate::model::components::{BaseStats, Buffs, CombatStats, Speeds, StatModifiers, Vitals};
use crate::model::formulas;
use crate::model::skill::{abnormal_type_client_id, ActiveBuff, Skill, SkillEffect};
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

    for effect in &skill.effects {
        match *effect {
            SkillEffect::MagicalAttack { power } => {
                let (m_atk, caster_name) = {
                    let m_atk =
                        world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                    (m_atk, world.objects.get_component::<crate::model::Player>(&caster_oid).expect("player").name.clone())
                };
                let m_def = target_m_def(world, target_oid);
                let damage = formulas::calc_magic_dam(m_atk, m_def, power, mcrit);
                apply_magic_damage(world, caster_oid, target_oid, damage, mcrit, &caster_name);
            }
            SkillEffect::Heal { power } => {
                let m_atk = world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                let amount = formulas::calc_heal(power, m_atk, mcrit);
                if crate::game_loop::combat::is_npc_oid(target_oid) {
                    // Healing an NPC: clamp and update, no system messages
                    // (nobody to send them to).
                    if let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                        if !vitals.dead {
                            vitals.cur_hp = (vitals.cur_hp + amount).min(vitals.max_hp as f64);
                        }
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
                }
            }
            SkillEffect::StatModifier(_) => {} // collected below
        }
    }

    // Continuous effects → one ActiveBuff on the target (`applyEffects`).
    // NPCs have no effect list yet (their stats are template-derived), so
    // buffs on NPC targets are dropped — G9 monsters cast nothing anyway.
    if crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    let buff_effects = skill.stat_modifier_effects();
    if !buff_effects.is_empty() {
        let expires_at_tick = world.tick + (skill.abnormal_time.max(0) as u64) * 10;
        let buff = ActiveBuff {
            skill_id: skill.id,
            skill_level: skill.level,
            abnormal_type_client_id: abnormal_type_client_id(&skill.abnormal_type),
            expires_at_tick,
            effects: buff_effects,
        };
        if let Some((target, base, mut mods, mut buffs, mut speeds, mut combat)) = world
            .objects
            .get_many_mut::<(
                &mut crate::model::Player,
                &BaseStats,
                &mut StatModifiers,
                &mut Buffs,
                &mut Speeds,
                &mut CombatStats,
            )>(&target_oid)
        {
            target.apply_buff(&world.data, &base, &mut mods, &mut buffs, &mut speeds, &mut combat, buff);
        }
        world
            .scheduler
            .schedule(expires_at_tick, ScheduledTask::BuffExpire { player_object_id: target_oid, skill_id: skill.id });
        let now = world.tick;
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(buffs) = world.objects.get_component::<Buffs>(&target_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(crate::network::enter_world::abnormal_status_update(buffs, now));
                }
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
    if let Some((player, base, mut mods, mut buffs, mut speeds, mut combat)) = world
        .objects
        .get_many_mut::<(
            &mut crate::model::Player,
            &BaseStats,
            &mut StatModifiers,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&player_object_id)
    {
        player.remove_buff(&world.data, &base, &mut mods, &mut buffs, &mut speeds, &mut combat, skill_id);
    }
    let now = world.tick;
    let Some(client_id) = client_for_player(world, player_object_id) else { return };
    if let Some(buffs) = world.objects.get_component::<Buffs>(&player_object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::enter_world::abnormal_status_update(buffs, now));
        }
    }
}

