//! Effect application: instant damage/heal effects, continuous (buff)
//! effects, and buff expiry.

use crate::game_loop::helpers::{broadcast_including_self, client_for_player};
use crate::model::formulas;
use crate::model::skill::{abnormal_type_client_id, ActiveBuff, Skill, SkillEffect};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::cast::abort_cast;

/// The `callSkill` → `activateSkill` → effect-handler chain for the effect
/// kinds ported so far. Continuous stat modifiers land as an `ActiveBuff` on
/// the target; `MagicalAttack`/`Heal` are instant.
pub(crate) fn apply_skill_effects(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    use server_packets::{sm_ids, SmParam};

    // Magic crit is rolled once per cast (Java rolls in each instant effect's
    // `instant()`; one roll covers the single instant effect skills have).
    let m_crit_rate = world.players[&caster_oid].m_crit_hit as f64;
    let crit_roll = world.roll(1000);
    let mcrit = skill.magic_type == 1 && formulas::calc_magic_crit(m_crit_rate, skill.is_bad(), crit_roll);

    for effect in &skill.effects {
        match *effect {
            SkillEffect::MagicalAttack { power } => {
                let (m_atk, caster_name) = {
                    let c = &world.players[&caster_oid];
                    (c.m_atk as f64, c.name.clone())
                };
                let m_def = world.players[&target_oid].m_def as f64;
                let damage = formulas::calc_magic_dam(m_atk, m_def, power, mcrit);
                apply_magic_damage(world, caster_oid, target_oid, damage, mcrit, &caster_name);
            }
            SkillEffect::Heal { power } => {
                let m_atk = world.players[&caster_oid].m_atk as f64;
                let amount = formulas::calc_heal(power, m_atk, mcrit);
                let healed = {
                    let Some(target) = world.players.get_mut(&target_oid) else { continue };
                    // Overheal clamp (`Heal.java`).
                    let amount = amount.min((target.max_hp as f64 - target.cur_hp).max(0.0));
                    target.cur_hp += amount;
                    amount
                };
                let caster_name = world.players[&caster_oid].name.clone();
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
                        let cur_hp = world.players[&target_oid].cur_hp as i32;
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
        if let Some(target) = world.players.get_mut(&target_oid) {
            target.apply_buff(&world.data, buff);
        }
        world
            .scheduler
            .schedule(expires_at_tick, ScheduledTask::BuffExpire { player_object_id: target_oid, skill_id: skill.id });
        let now = world.tick;
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(target) = world.players.get(&target_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(crate::network::enter_world::abnormal_status_update(target, now));
                }
            }
        }
    }
}

/// Port of `Creature.doAttack` → `PlayerStatus.reduceHp` for magic skill
/// damage between players: CP absorbs first, then HP — clamped at 1.0
/// because there's no death system yet (TODO(G9 death): `doDie`). Also rolls
/// Java's `Formulas.calcAtkBreak` cast-break against a pre-launch cast on
/// the victim (SM 27 + `MagicSkillCanceled`).
pub(crate) fn apply_magic_damage(world: &mut World, caster_oid: i32, target_oid: i32, damage: f64, mcrit: bool, caster_name: &str) {
    use server_packets::{sm_ids, SmParam};

    let (target_name, cp_after, hp_after) = {
        let Some(target) = world.players.get_mut(&target_oid) else { return };
        let cp_absorb = damage.min(target.cur_cp);
        target.cur_cp -= cp_absorb;
        target.cur_hp = (target.cur_hp - (damage - cp_absorb)).max(1.0);
        (target.name.clone(), target.cur_cp as i32, target.cur_hp as i32)
    };
    let dmg_int = damage as i32;

    if let Some(client_id) = client_for_player(world, caster_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            if mcrit {
                cs.send(server_packets::system_message_with(sm_ids::M_CRITICAL, &[]));
            }
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
                &[
                    SmParam::PlayerName(caster_name.to_string()),
                    SmParam::PlayerName(target_name.clone()),
                    SmParam::Int(dmg_int),
                ],
            ));
        }
    }
    if let Some(client_id) = client_for_player(world, target_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2,
                &[
                    SmParam::PlayerName(target_name.clone()),
                    SmParam::PlayerName(caster_name.to_string()),
                    SmParam::Int(dmg_int),
                ],
            ));
        }
    }

    // Both sides see the victim's new CP/HP (`broadcastStatusUpdate`).
    broadcast_including_self(
        world,
        target_oid,
        &server_packets::status_update(
            target_oid,
            &[
                (server_packets::status_update_type::CUR_CP, cp_after),
                (server_packets::status_update_type::CUR_HP, hp_after),
            ],
        ),
    );

    // Cast break (`Formulas.calcAtkBreak`, `AltGameCancelByHit = cast`).
    let breakable = world
        .players
        .get(&target_oid)
        .is_some_and(|p| p.cast.as_ref().is_some_and(|c| !c.launched));
    if breakable {
        let men_bonus = {
            let t = &world.players[&target_oid];
            world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Men, t.men)
        };
        let break_roll = world.roll(100);
        if formulas::calc_atk_break(damage, men_bonus, break_roll) {
            abort_cast(world, target_oid);
            if let Some(client_id) = client_for_player(world, target_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED, &[]));
                }
            }
        }
    }
}

/// `BuffFinishTask`, fired when a buff's `abnormalTime` elapses
/// (`ScheduledTask::BuffExpire`). A buff already gone (re-cast/replaced) is a
/// no-op, matching the scheduler's dead-id contract.
pub(crate) fn handle_buff_expire(world: &mut World, player_object_id: i32, skill_id: i32) {
    let still_active = world
        .players
        .get(&player_object_id)
        .is_some_and(|p| p.buffs.iter().any(|b| b.skill_id == skill_id));
    if !still_active {
        return;
    }
    if let Some(player) = world.players.get_mut(&player_object_id) {
        player.remove_buff(&world.data, skill_id);
    }
    let now = world.tick;
    let Some(client_id) = client_for_player(world, player_object_id) else { return };
    if let Some(player) = world.players.get(&player_object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::enter_world::abnormal_status_update(player, now));
        }
    }
}

