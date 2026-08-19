//! Effect application — Java's `AbstractEffect` implementations.
//!
//! `apply_skill_effects` (the instant dispatch) lives here; its fattest arms
//! are one function apiece in the sibling [`instant`] module. The rest
//! is split by role and re-exported, so callers keep saying `effects::foo`:
//!
//! - `continuous` — `apply_continuous_effects`: buffs landing as an
//!   `ActiveBuff`, abnormal visuals, and restoring persisted buffs on login.
//! - `triggers` — the on-damage / on-magic / on-attack trigger-skill hooks.
//! - `control` — crowd control and its side effects: fear, mute, block-actions,
//!   fake death, confuse retargeting, overhit, plus `call_party`/`call_pc`.
//! - `support` — item grants and the magic-success roll.
//! - `gathering` — spoil, sweeper, sow, harvest, consume-body.
//! - `damage` — the damage arithmetic: shield/defence, attribute and trait
//!   modifiers, counter-attack, and `apply_skill_damage`.
//! - `ticks` — the damage-over-time beat and buff expiry.
//! - `traits` — attack/defence trait and skill-rate bookkeeping, the PvP/PvE
//!   bonus, MP cost and reuse time.

use crate::game_loop::bot_report;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::client_for_player;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::skill_by_id;
use crate::model::components::{BaseStats, Buffs, CombatStats, StatModifiers, Vitals};
use crate::model::formulas;
use crate::model::punishment::{PunishmentAffect, PunishmentType};
use crate::model::skill::{ActiveBuff, Skill, SkillEffect};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::instant;
use crate::game_loop::helpers::send_sm_to_player;
use crate::game_loop::helpers::send_to_player;

mod continuous;
pub(crate) mod control;
pub(crate) mod damage;
mod dispel;
mod gathering;
mod summoning;
mod support;
mod ticks;
mod traits;
mod triggers;

use crate::game_loop::npc::ai::force_attack_target;
pub(crate) use continuous::{
    apply_continuous_effects, broadcast_change_wait_type, refresh_abnormal_visuals,
    restore_persisted_buffs, set_collision_grown, stop_fake_death,
};
#[cfg(test)]
pub(crate) use control::creature_level_for_test;
#[cfg(test)]
pub(crate) use control::recharge_level_penalty;
pub(crate) use control::{
    ManaHealKind, add_hate, apply_block_actions_interrupt, apply_mute_interrupt, bluff,
    break_fake_death_on_damage, call_party, call_pc, call_pc_player, casting_resists_abnormal,
    confuse_chance_passes, cp, creature_level, creature_name, delete_hate, delete_hate_of_me,
    fake_death, fear_action, fear_can_start, hp_by_level, mana_heal, mp_restore, random_bystander,
    randomize_hate, rebalance_party_hp, record_overhit, retarget_onto, skill_turning,
    stop_effects_on_damage, target_me, teleport_to_target, try_break_stun,
};
pub(crate) use damage::{
    SkillHit, apply_buff_to_npc, apply_skill_damage, attribute_mod, broadcast_target_buffs,
    broadcast_vitals, caster_m_atk, defence_after_shield, dot_interval_ticks, dot_tick_damage,
    physical_attack, recompute_max_vitals, recompute_npc_buffed_stats, refresh_summon_info,
    schedule_dam_over_time, skill_trait_mod, target_m_def, target_p_def,
};
pub(crate) use dispel::{dispel_all, dispel_by_category, dispel_by_slot_myself};
pub(crate) use gathering::{
    apply_consume_body, apply_harvesting, apply_sow, apply_spoil, apply_sweeper,
};
#[cfg(test)]
pub(crate) use gathering::{calc_harvest_success, calc_sow_success};
pub(crate) use summoning::{betray, summon_npc};
pub(crate) use support::send_sm;
pub(crate) use support::{
    broadcast_social_action, change_appearance, focus_momentum, give_item, give_item_random,
    give_sp, grant_and_notify, magic_success_input, open_recipe_book, roll_magic_failure,
    send_system_message_to_clan,
};
pub(crate) use ticks::{
    expire_active_buffs, expire_buffs_where, handle_buff_expire, handle_dam_over_time_tick,
};
#[cfg(test)]
pub(crate) use traits::pvp_pve_bonus_for_test;
pub(crate) use traits::{
    buff_level, calc_attack_trait_bonus, calc_general_trait_bonus, calc_weakness_bonus,
    calc_weapon_trait_bonus, caster_display_name, caster_str_bonus, maybe_buff_level,
    merge_attack_traits, merge_defence_traits, merge_skill_rates, mp_consume_for,
    player_or_npc_level, pvp_pve_bonus, refresh_passive_skill_rates, remove_attack_traits,
    remove_defence_traits, remove_skill_rates, reuse_time_for,
};
pub(crate) use triggers::{
    fire_attack_triggers, fire_damage_received_triggers, fire_magic_type_triggers,
    fire_option_attack_triggers, fire_option_cast_triggers,
};

/// The `callSkill` → `activateSkill` → effect-handler chain for the effect
/// kinds ported so far. Continuous stat modifiers land as an `ActiveBuff` on
/// the target; `MagicalAttack`/`Heal` are instant.
/// Java's `damage *= getValue(Stat.PHYSICAL_SKILL_POWER, 1)` /
/// `MAGICAL_SKILL_POWER` — the last multiplier a skill's damage passes
/// through. The physical one is applied by every `PhysicalAttack`-family
/// *effect handler* rather than by the shared formula, and the magical one
/// inside `calcMagicDam`; both land at the same point in the arithmetic, so
/// they share this reader (G34 S4).
pub(crate) fn skill_power_mul(world: &World, caster_oid: i32, magic: bool) -> f64 {
    use crate::model::stats::Stat;
    let stat = if magic {
        Stat::MagicalSkillPower
    } else {
        Stat::PhysicalSkillPower
    };
    world
        .objects
        .get_component::<StatModifiers>(&caster_oid)
        .map(|m| {
            (1.0 + m.add.get(&stat).copied().unwrap_or(0.0))
                * m.mul.get(&stat).copied().unwrap_or(1.0)
        })
        .unwrap_or(1.0)
}

/// Snapshot the target's buff list through `f` (`None` filters an entry out),
/// collecting up front so the caller's loop can take `&mut World` — the
/// borrow-splitting idiom the dispel/expiry family shares. A missing `Buffs`
/// component is an empty snapshot.
pub(crate) fn buffs_snapshot<T>(
    world: &World,
    object_id: i32,
    f: impl FnMut(&ActiveBuff) -> Option<T>,
) -> Vec<T> {
    world
        .objects
        .get_component::<Buffs>(&object_id)
        .map(|buffs| buffs.0.iter().filter_map(f).collect())
        .unwrap_or_default()
}

/// The effects whose Java handler gates on `Formulas.calcSkillEvasion` in its
/// `calcSuccess` — i.e. the damage-dealing family. Kept as an explicit list
/// rather than "anything with a power" so adding a damage effect has to make a
/// deliberate choice about dodging.
fn is_damage_effect(effect: &SkillEffect) -> bool {
    matches!(
        effect,
        SkillEffect::MagicalAttack { .. }
            | SkillEffect::PhysicalAttack { .. }
            | SkillEffect::PhysicalAttackHpLink { .. }
            | SkillEffect::Blow { .. }
            | SkillEffect::EnergyAttack { .. }
            | SkillEffect::HpDrain { .. }
    )
}

/// `Formulas.calcSkillEvasion` — a flat per-`magicType` dodge chance
/// (`Rnd.get(100) < getSkillEvasionTypeValue(skill.getMagicType())`), granted
/// by `SkillEvasion` (Ultimate Evasion 111, Evasion 446 — both bucket 0, the
/// physical-skill one). Both sides get a message, which is what makes a dodge
/// legible rather than a silent miss.
fn skill_evasion_dodges(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
) -> bool {
    let chance = world
        .objects
        .get_component::<StatModifiers>(&target_oid)
        .and_then(|m| m.skill_evasion.get(&skill.magic_type).copied())
        .unwrap_or(0.0);
    if chance <= 0.0 || (world.roll(100) as f64) >= chance {
        return false;
    }
    let (caster_name, target_name) = (
        creature_name(world, caster_oid),
        creature_name(world, target_oid),
    );
    send_sm_to_player(
        world,
        caster_oid,
        server_packets::sm_ids::C1_DODGED_THE_ATTACK,
        &[server_packets::SmParam::Text(target_name)],
    );
    send_sm_to_player(
        world,
        target_oid,
        server_packets::sm_ids::YOU_HAVE_DODGED_C1_S_ATTACK,
        &[server_packets::SmParam::Text(caster_name)],
    );
    true
}

/// `Formulas.calcSkillMastery` — the proc that collapses a skill's cooldown to
/// 100 ms (Skill Mastery 330 / 331, Focus Skill Mastery 334).
///
/// `Stat.SKILL_MASTERY` holds the **ordinal of the `BaseStat`** that drives the
/// chance, not a magnitude, and absent (`-1`) means no mastery. The chance is
/// that stat's bonus times `SKILL_MASTERY_RATE`; Java then multiplies by a
/// per-class `Config.SKILL_MASTERY_CHANCE_MULTIPLIERS` table that defaults to
/// `1f` and which this dist does not populate, so it is left out.
pub(crate) fn calc_skill_mastery(world: &mut World, caster_oid: i32) -> bool {
    use crate::model::stats::{BaseStat, Stat};
    let Some(mods) = world.objects.get_component::<StatModifiers>(&caster_oid) else {
        return false;
    };
    let Some(ordinal) = mods.add.get(&Stat::SkillMastery).copied() else {
        return false;
    };
    let rate = mods
        .mul
        .get(&Stat::SkillMasteryRate)
        .copied()
        .unwrap_or(1.0);
    // The stored value is *this* enum's discriminant (see `BaseStat::from_name`
    // — Java's ordinal ordering differs and must not be copied across).
    let Some(base_stat) = BaseStat::from_ordinal(ordinal as i32) else {
        return false;
    };
    let Some(base) =
        world
            .objects
            .get_component::<BaseStats>(&caster_oid)
            .map(|b| match base_stat {
                BaseStat::Str => b.str_,
                BaseStat::Dex => b.dex,
                BaseStat::Con => b.con,
                BaseStat::Int => b.int_,
                BaseStat::Wit => b.wit,
                BaseStat::Men => b.men,
            })
    else {
        return false;
    };
    let chance = world.data.stat_bonus.bonus(base_stat, base) * rate;
    (world.roll(100) as f64) < chance
}

/// Java `CreatureStat.getMaxRecoverableHp()` / `getMaxRecoverableCp()` —
/// `getValue(MAX_RECOVERABLE_*, getMaxHp()/getMaxCp())`, the ceiling a **heal**
/// may restore to.
///
/// Identity is the full pool, so this only bites under Noblesse Harmony (1326)
/// / Symphony (1327), which grant it `PER −30` / `−40`: you can be healed back
/// to 70 % HP and 60 % CP and no further. Java uses it in every heal clamp, and
/// also as the cap on HP/MP *absorbed* by vampiric attacks.
pub(crate) fn max_recoverable(
    world: &World,
    object_id: i32,
    stat: crate::model::stats::Stat,
    base: f64,
) -> f64 {
    world
        .objects
        .get_component::<StatModifiers>(&object_id)
        .map(|m| crate::model::finalize(m, stat, base))
        .unwrap_or(base)
}

/// The inverse of `servitor::servitor_of` — given a servitor, who owns it.
/// The owner link lives on the servitor as `ServitorOf`, which is also what
/// makes Java's `canStart` (`effected.isSummon()`) expressible: no component,
/// not a servitor, no unsummon.
/// Java's ten-minute Force decay: `restartChargeTask` on every gain or partial
/// spend, `stopChargeTask` when the pool empties, `ResetChargesTask` clearing
/// it when the timer runs out.
///
/// Arming and the stale check both live here so the two halves cannot drift.
pub(crate) fn arm_charge_decay(world: &mut World, player_oid: i32) {
    /// `ThreadPool.schedule(new ResetChargesTask(this), 600000)`.
    const CHARGE_DECAY_TICKS: u64 = 6_000;

    let Some(seq) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
        .map(|p| {
            p.charges_seq = p.charges_seq.wrapping_add(1);
            p.charges_seq
        })
    else {
        return;
    };
    // The bump alone is `stopChargeTask`: an emptied pool invalidates the
    // pending task and arms nothing, so nothing fires.
    if world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_none_or(|p| p.charges <= 0)
    {
        return;
    }
    world.scheduler.schedule(
        world.tick + CHARGE_DECAY_TICKS,
        ScheduledTask::ResetCharges { player_oid, seq },
    );
}

/// `ResetChargesTask.run` — `clearCharges()` plus the `EtcStatusUpdate` that
/// makes the Force gauge empty on screen.
pub(crate) fn reset_charges(world: &mut World, player_oid: i32, seq: u64) {
    let stale = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_none_or(|p| p.charges_seq != seq);
    if stale {
        return;
    }
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
    {
        p.charges = 0;
    }
    if let Some(client_id) = client_for_player(world, player_oid) {
        crate::game_loop::helpers::send_etc_status_update(world, client_id, player_oid);
    }
}

pub(crate) fn servitor_owner_of(world: &World, servitor_oid: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::components::ServitorOf>(&servitor_oid)
        .map(|s| s.owner_object_id)
        .filter(|&owner| owner != 0)
}

pub(crate) fn apply_skill_effects(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
) {
    // Magic crit is rolled once per cast (Java rolls in each instant effect's
    // `instant()`; one roll covers the single instant effect skills have).
    let m_crit_rate = world
        .objects
        .get_component::<CombatStats>(&caster_oid)
        .map(|c| c.m_crit_hit)
        .unwrap_or(0.0);
    let crit_roll = world.roll(1000);
    let mcrit =
        skill.magic_type == 1 && formulas::calc_magic_crit(m_crit_rate, skill.is_bad(), crit_roll);

    // Spiritshots (magic skills only, `useSpiritShot() == _magic == 1`): read
    // the charged flag once per cast for the damage/heal bonus; the shot is
    // spent below after every effect has been applied (Java `Skill` uncharges
    // post-`applyEffects`). `caster_is_player` stands in for `isMageClass()` in
    // the heal static bonus — this fn's caster is always a player.
    let caster_is_player = world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .is_some();
    let (sps, bss) = if skill.magic_type != 1 {
        (false, false)
    } else if crate::game_loop::combat::is_npc_oid(caster_oid) {
        // A **summon** charges Beast Spiritshots from its owner, the magic
        // counterpart of the soulshot path. Spending is here rather than in the
        // attack loop because a summon's magic shot is consumed by the *cast*,
        // not by a swing. Blessed Beast Spiritshots do not exist on this dist,
        // so only the ×2 tier is reachable.
        (
            crate::game_loop::servitor::uncharge_spiritshot(world, caster_oid),
            false,
        )
    } else {
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
    };
    let magic_shots_bonus = if bss {
        4.0
    } else if sps {
        2.0
    } else {
        1.0
    };

    // Soulshots (physical/thrown skills, Java `useSoulShot() == !isMagic`):
    // charged flag read once for the ×2 physical-damage bonus; spent post-cast
    // like spiritshots. Blessed soulshots don't exist in Interlude.
    let ss = skill.magic_type != 1
        && world
            .objects
            .get_component::<crate::model::Player>(&caster_oid)
            .is_some_and(|p| p.is_charged_shot(crate::model::ShotType::Soulshots));

    let ctx = instant::CastCtx {
        caster_oid,
        target_oid,
        mcrit,
        ss,
        sps,
        bss,
        magic_shots_bonus,
        caster_is_player,
    };

    for effect in &skill.effects {
        // `Formulas.calcSkillEvasion`, which Java calls from the `calcSuccess`
        // of every *damage* effect handler (Backstab, DeathLink, EnergyAttack,
        // FatalBlow, HpDrain, MagicalAttack, PhysicalAttack, …) rather than
        // once per skill. Checking it per damage effect here keeps that shape:
        // a skill carrying a nuke *and* a debuff can have its nuke dodged
        // while the debuff still rolls its own landing chance (G34 S4).
        if is_damage_effect(effect) && skill_evasion_dodges(world, caster_oid, target_oid, skill) {
            continue;
        }
        match effect {
            // Pet food. Java branches on the *effected*: a pet's own bar is
            // filled through `servitor::apply_food_skill` (which targets the
            // pet, so it never arrives here), while a **player** — necessarily
            // a mounted one, since that is the only player-side food bar —
            // tops up the mount's gauge, by the `wyvern` param on a wyvern and
            // `ride` on anything else.
            SkillEffect::Feed { ride, wyvern, .. } => {
                let mount_type = world
                    .objects
                    .get_component::<crate::model::Player>(&target_oid)
                    .map_or(0, |p| p.mount_type);
                if mount_type != 0 {
                    // `MountType.WYVERN` is ordinal 2 (see `mounts::Mount`).
                    let amount = if mount_type == 2 { *wyvern } else { *ride };
                    let current = world
                        .objects
                        .get_component::<crate::model::Player>(&target_oid)
                        .map_or(0, |p| p.mount_feed);
                    crate::game_loop::admin::mounts::set_current_feed(
                        world,
                        target_oid,
                        current + amount,
                    );
                }
            }
            SkillEffect::SummonCubic { cubic_id, cubic_level } => {
                crate::game_loop::cubic::summon_cubic(world, target_oid, *cubic_id, *cubic_level);
            }
            SkillEffect::SummonNpc { npc_id, npc_count, despawn_delay } => {
                summon_npc(world, target_oid, skill, *npc_id, *npc_count, *despawn_delay);
            }
            SkillEffect::MagicalAttack { power } => instant::magical_attack(world, &ctx, skill, *power),
            SkillEffect::MagicalAttackRange { power, shield_def_percent } => {
                instant::magical_attack_range(world, &ctx, skill, *power, *shield_def_percent)
            }
            // The MP-restore family (`ManaHeal`, `ManaHealByLevel`,
            // `ManaHealPercent`, `Mp`) — see `control::mana_heal`/`restore_mp`.
            SkillEffect::ManaHeal { power } => {
                mana_heal(world, caster_oid, target_oid, skill, *power, ManaHealKind::Flat);
            }
            SkillEffect::ManaHealByLevel { power } => {
                mana_heal(world, caster_oid, target_oid, skill, *power, ManaHealKind::ByLevel);
            }
            SkillEffect::ManaHealPercent { power } => {
                mana_heal(world, caster_oid, target_oid, skill, *power, ManaHealKind::Percent);
            }
            SkillEffect::MpRestore { amount, percent } => {
                mp_restore(world, caster_oid, target_oid, *amount, *percent);
            }
            // `Resurrection.instant` → `Player.reviveRequest`: this does not
            // revive anyone. It *proposes* a revive and puts a `ConfirmDlg` on
            // the corpse's screen; the actual revive happens in the answer
            // handler (`death::handle_revive_answer`).
            SkillEffect::Resurrection { power, hp_percent, mp_percent, cp_percent } => {
                crate::game_loop::death::revive_request(
                    world,
                    caster_oid,
                    target_oid,
                    *power,
                    *hp_percent,
                    *mp_percent,
                    *cp_percent,
                    skill.id,
                    skill.affect_range,
                );
            }
            // `Summon.instant` — bring out a servitor. Java re-summons over any
            // existing one rather than stacking, which `summon_servitor`
            // handles.
            // `SummonPet.instant` — the collar is already parked on the player.
            SkillEffect::SummonPet => {
                crate::game_loop::servitor::summon_pet(world, target_oid);
            }
            SkillEffect::Summon { npc_id, life_time, consume_item_id, consume_item_count } => {
                crate::game_loop::servitor::summon_servitor(
                    world,
                    target_oid,
                    *npc_id,
                    skill.id,
                    *life_time,
                    *consume_item_id,
                    *consume_item_count,
                );
            }
            // `Confuse.instant` — the victim turns on a random bystander.
            //
            // Java sets the victim's target and `AI_INTENTION_ATTACK` directly.
            // The ported NPC AI derives its attack target fresh from
            // `AggroList::most_hated` each think tick (no cached "current
            // target" to override), so — exactly as `GetAgro` already does —
            // the faithful equivalent is making the chosen bystander's hate
            // dominant. A confused *player* just gets their target swapped;
            // Java's player-side gate lives behind the `CONFUSED` flag, which
            // is unreachable on this dist (see `effect_flag::CONFUSED`).
            SkillEffect::Confuse { chance } => {
                if !confuse_chance_passes(world, caster_oid, target_oid, skill, *chance) {
                    continue;
                }
                let Some(victim) = random_bystander(world, target_oid, caster_oid, false) else { continue };
                retarget_onto(world, target_oid, victim);
            }
            SkillEffect::RandomizeHate { chance } => {
                randomize_hate(world, caster_oid, target_oid, skill, *chance);
            }
            // `MagicalAttackMp.instant()` — drain the target's **MP**, not HP.
            // Distinct from `MagicalAttack` in three ways: its own
            // `calcManaDam` formula (target max MP is a multiplier), a
            // per-skill `criticalLimit` cap on a crit, and its own
            // `calcSuccess` gate (`calcMagicAffected`).
            SkillEffect::MagicalAttackMp { power, critical, critical_limit } => instant::magical_attack_mp(world, &ctx, skill, *power, *critical, *critical_limit),
            SkillEffect::PhysicalAttack { power, p_atk_mod, p_def_mod, critical_chance, ignore_shield_defence }
            // `PhysicalAttackHpLink` is the same formula with one extra
            // multiplier at the end, so it shares this arm rather than
            // duplicating forty lines of damage assembly.
            | SkillEffect::PhysicalAttackHpLink { power, p_atk_mod, p_def_mod, critical_chance, ignore_shield_defence } => {
                let hp_link = matches!(effect, SkillEffect::PhysicalAttackHpLink { .. });
                physical_attack(
                    world,
                    caster_oid,
                    target_oid,
                    skill,
                    ss,
                    *power,
                    *p_atk_mod,
                    *p_def_mod,
                    *critical_chance,
                    *ignore_shield_defence,
                    hp_link,
                );
            }
            // `PolearmSingleTarget` is a pure stat toggle: `onStart` sets
            // `PHYSICAL_POLEARM_TARGET_SINGLE` as a **fixed** 1 and `onExit`
            // removes it. Both halves ride the buff lifecycle in
            // `apply_continuous_effects`, so the instant pass does nothing.
            SkillEffect::PolearmSingleTarget => {}
            // `CallSkill.instant` — cast another skill outright, no cast time
            // and no cost (`SkillCaster.triggerCast`'s shape). Java's
            // self-reference guard is ported: a skill that calls itself at the
            // same level returns rather than looping.
            SkillEffect::CallSkill {
                skill_id,
                skill_level,
                chance,
            } => {
                if *skill_id == skill.id && *skill_level == skill.level {
                    continue;
                }
                if *chance < 100 && world.roll(100) > *chance {
                    continue;
                }
                let Some(called) = skill_by_id(world, *skill_id, *skill_level)
                else {
                    continue;
                };
                apply_skill_effects(world, caster_oid, target_oid, &called);
            }
            // `NightStatModify` grants nothing at instant time; the buff's
            // stored modifiers are (re)written by `night_stats::refresh_one`,
            // which runs here so a cast made *at* night takes effect at once
            // rather than at the next dawn.
            SkillEffect::NightStatModify { .. } => {
                let night = crate::game_loop::game_time::is_night_at(commons::util::now_millis());
                crate::game_loop::night_stats::refresh_one(world, target_oid, night);
            }
            // `ReduceDropPenalty` is a pure stat grant (`pump`), merged when
            // the buff lands. `ResurrectionSpecial` does nothing while it is
            // *up*: its whole mechanic is `onExit`, which death fires — see
            // `handle_buff_expire`.
            SkillEffect::ReduceDropPenalty { .. } | SkillEffect::ResurrectionSpecial { .. } => {}
            SkillEffect::Betray => betray(world, caster_oid, target_oid),
            // `ImmobilePetBuff.onStart` — root the effected summon. The root
            // itself is the `IMMOBILIZED` flag the landed buff carries, so
            // there is nothing to do at instant time.
            //
            // Java's `effector == effected || owner == effector` gate is
            // satisfied by construction here: Servitor Empowerment (1299) is
            // `targetType SUMMON`, which resolves to the caster's *own*
            // servitor, so there is no way to aim it at somebody else's pet.
            //
            // SKIP(census): the whole dist carries this effect on exactly one
            // skill — 1299, `SUMMON`/`SINGLE`. There is no wider carrier to
            // re-check, so porting the gate would guard a case no data can
            // produce.
            SkillEffect::ImmobilePetBuff => {}
            // `CallParty.instant` — Chant of Gate (1429). Every *other* party
            // member is pulled to the caster, each gated by CallPc's shared
            // `checkSummonTargetStatus`. Note there is no `ConfirmDlg` here:
            // unlike Summon Friend, Java teleports them outright.
            SkillEffect::CallParty => {
                call_party(world, caster_oid);
            }
            SkillEffect::Blow { power, chance_boost, critical_chance, backstab } => instant::blow(world, &ctx, skill, *power, *chance_boost, *critical_chance, *backstab),
            SkillEffect::Lethal { full_lethal, half_lethal } => instant::lethal(world, &ctx, skill, *full_lethal, *half_lethal),
            SkillEffect::HpDrain { power, percentage } => instant::hp_drain(world, &ctx, skill, *power, *percentage),
            // `CpHealPercent.instant` — a share of the target's **max CP**,
            // clamped by `getMaxRecoverableCp()`. Java bails on a dead target,
            // a door and an HP-blocked one (the last is not a typo: the CP heal
            // reads `isHpBlocked`).
            // `OpenDoor.instant` — the lock-picking half of Unlock (27).
            SkillEffect::OpenDoor { chance, is_item } => instant::open_door(world, &ctx, skill, *chance, *is_item),
            // `OpenChest.instant` — the treasure-box half of Unlock (27), and a
            // *level* check rather than a chance roll: within 6 levels (5 above
            // 77) the box opens, otherwise it turns on you. Opening it kills the
            // chest with `setSpecialDrop()` + `setMustRewardExpSp(false)`, so it
            // rolls its own drop list and pays no exp.
            SkillEffect::OpenChest => instant::open_chest(world, &ctx),
            SkillEffect::Bluff { chance } => {
                bluff(world, caster_oid, target_oid, skill, *chance);
            }
            // `Unsummon.instant` — Erase (1395). `canStart` requires the
            // *effected* to be a summon, so the skill is aimed at the pet
            // rather than its owner, and the chance defaults to **-1**
            // ("always") rather than 100.
            SkillEffect::Unsummon { chance } => instant::unsummon(world, &ctx, skill, *chance),
            // `DeathLink.instant` — Curse Death Link (1159). The power scales
            // with how close the **caster** is to death:
            // `power × (2 − 2·curHp/maxHp)` — ×2 at 0 HP, ×0 at full, so
            // casting it healthy does literally nothing.
            SkillEffect::DeathLink { power } => instant::death_link(world, &ctx, skill, *power),
            SkillEffect::CpHealPercent { power } => instant::cp_heal_percent(world, &ctx, *power),
            SkillEffect::HpByLevel { power } => hp_by_level(world, caster_oid, *power),
            SkillEffect::Heal { power } => instant::heal(world, &ctx, skill, *power),
            SkillEffect::HealPercent { power } => instant::heal_percent(world, &ctx, skill, *power),
            SkillEffect::FocusMomentum { amount, max_charges } => {
                focus_momentum(world, target_oid, *amount, *max_charges);
            }
            SkillEffect::EnergyAttack { power, critical_chance, p_def_mod, charge_consume, ignore_shield_defence } => instant::energy_attack(world, &ctx, skill, *power, *critical_chance, *p_def_mod, *charge_consume, *ignore_shield_defence),
            SkillEffect::GiveItem { item_id, item_count, item_enchant_level } => {
                give_item(world, target_oid, *item_id, *item_count, *item_enchant_level);
            }
            SkillEffect::GiveItemRandom { groups } => {
                give_item_random(world, target_oid, groups);
            }
            SkillEffect::GiveSp { sp } => give_sp(world, caster_oid, target_oid, *sp),
            // The appearance potions: one field, then `broadcastUserInfo`.
            SkillEffect::ChangeAppearance { part, value } => {
                change_appearance(world, target_oid, *part, *value);
            }
            // `SendSystemMessageToClan.instant` — the whole clan hears it.
            SkillEffect::SendSystemMessageToClan { message_id } => {
                send_system_message_to_clan(world, target_oid, *message_id);
            }
            // `Recovery.instant` is an empty body in Java; see the variant.
            SkillEffect::Recovery => {}
            // `SetSkill.instant` — `addSkill(skill, true)`: granted and stored,
            // exactly as if it had been learned from a trainer.
            SkillEffect::SetSkill {
                skill_id,
                skill_level,
            } => {
                set_skill(world, target_oid, *skill_id, *skill_level);
            }
            // `TeleportToTarget.instant` — the caster dashes behind the target.
            SkillEffect::TeleportToTarget => {
                teleport_to_target(world, caster_oid, target_oid);
            }
            // `Escape.instant()` → `teleToLocation(TeleportWhereType)`. Players
            // only — nothing else carries the effect.
            SkillEffect::Escape { dest } => {
                if world
                    .objects
                    .has_component::<crate::model::Player>(&target_oid)
                {
                    escape_to(world, target_oid, *dest);
                }
            }
            // `Teleport.instant` — `teleToLocation(loc, true, null)`. The
            // destination Scrolls of Escape; players only, since nothing else
            // carries the effect.
            SkillEffect::Teleport { x, y, z } => {
                if world
                    .objects
                    .has_component::<crate::model::Player>(&target_oid)
                {
                    crate::game_loop::death::teleport_player(world, target_oid, *x, *y, *z);
                }
            }
            // `Hp.instant` — a raw HP change. Java's guards are dead / door /
            // `isHpBlocked` / **isRaid**, the last of which the `Heal` family
            // does not have; the gain is clamped to the recoverable headroom
            // (`MAX_RECOVERABLE_HP`, so a Noblesse Harmony aura caps it), and a
            // `PER` amount is a share of **max** HP.
            //
            // Java also folds in `ADDITIONAL_POTION_HP` when the effect came
            // from a potion or elixir item; no skill on this dist grants that
            // stat, so the term is 1:1 with 0 and is not modelled.
            SkillEffect::Hp { amount, percent } => instant::hp(world, &ctx, *amount, *percent),
            SkillEffect::CallPc { item_id, item_count } => {
                call_pc_player(world, caster_oid, target_oid, *item_id, *item_count);
                call_pc(world, caster_oid, target_oid, skill);
            }
            SkillEffect::GiveRecommendation { amount } => {
                crate::game_loop::reco::apply_give_recommendation(world, caster_oid, target_oid, *amount);
            }
            SkillEffect::CreateHeadquarter { advanced } => {
                // `HeadquarterCreate.instant`: the effector (an attacker clan
                // leader) plants the HQ flag. All the siege/leader/attacker/
                // flag-cap checks live in the engine (mirrors the effect body +
                // `BuildCampSkillCondition`).
                crate::game_loop::siege::place_siege_flag(world, caster_oid, *advanced);
            }
            SkillEffect::OpenRecipeBook { dwarven } => {
                open_recipe_book(world, caster_oid, *dwarven);
            }
            SkillEffect::Spoil => {
                apply_spoil(world, caster_oid, target_oid, skill);
            }
            SkillEffect::Sweeper => {
                apply_sweeper(world, caster_oid, target_oid);
            }
            SkillEffect::Sow => {
                apply_sow(world, caster_oid, target_oid);
            }
            SkillEffect::Harvesting => {
                apply_harvesting(world, caster_oid, target_oid);
            }
            SkillEffect::ConsumeBody => {
                apply_consume_body(world, caster_oid, target_oid);
            }
            // `DamOverTime`'s magic-crit burst is **not** an instant effect:
            // Java puts it in `onStart`, which only runs once the effect is
            // added to the effect list — i.e. only if the debuff landed. It is
            // applied after `apply_continuous_effects` reports that, below.
            SkillEffect::DamOverTime { .. } => {}
            SkillEffect::DispelBySlotMyself { dispel } => {
                dispel_by_slot_myself(world, target_oid, dispel);
            }
            SkillEffect::DispelAll => dispel_all(world, target_oid),
            // `Grow` is a continuous-only pair of hooks (onStart/onExit);
            // both live on the buff apply/expire path, so the instant
            // dispatch has nothing to do for it.
            SkillEffect::Grow => {}
            SkillEffect::DispelBySlot { dispel } => instant::dispel_by_slot(world, &ctx, skill, dispel),
            SkillEffect::DispelBySlotProbability { dispel, rate } => instant::dispel_by_slot_probability(world, &ctx, skill, dispel, *rate),
            SkillEffect::DispelByCategory { slot, rate, max } => {
                dispel_by_category(world, target_oid, skill, slot, *rate, *max);
            }
            // The bot-report punishments (`BotReportTable.handleReport` casts
            // these on the reported character). Each is a Java `onStart` that
            // starts a punishment for the buff's life; the matching `onExit`
            // lives in `handle_buff_expire_inner`. Java passes expiration `0`
            // — "forever" — because the *buff* is the timer.
            SkillEffect::BlockChat => {
                start_bot_report_punishment(world, target_oid, PunishmentType::ChatBan);
            }
            SkillEffect::BlockParty => {
                start_bot_report_punishment(world, target_oid, PunishmentType::PartyBan);
            }
            SkillEffect::BlockAction { blocked_actions } => {
                // Java only turns *two* of the blocked ids into punishments;
                // the rest are enforced by `checkCondition` at the action's own
                // call site (trade, `-2`).
                if blocked_actions.contains(&bot_report::PARTY_ACTION_BLOCK_ID) {
                    start_bot_report_punishment(world, target_oid, PunishmentType::PartyBan);
                }
                if blocked_actions.contains(&bot_report::CHAT_BLOCK_ID) {
                    start_bot_report_punishment(world, target_oid, PunishmentType::ChatBan);
                }
            }
            // `Flag.onStart` → `updatePvPFlag(1)`: the reported character can
            // be attacked freely while the debuff is up.
            SkillEffect::PvpFlag => {
                crate::game_loop::pvp::update_pvp_flag(world, target_oid, 1);
            }
            SkillEffect::StatModifier(_) => {} // collected below
            // Blessing of Protection: no instant action — it lands purely as
            // the timed `PK_PROTECT` abnormal handled by the buff path below
            // (kept off the empty-`buff_effects` bail via `has_protection`);
            // the immunity itself is `pvp::protection_blessing_blocks`, run
            // by both intention paths.
            // Purely state-flag effects: nothing happens at application time
            // beyond the buff landing — the mechanic is the abnormal flag the
            // buff carries, read by the action gates (`game_loop::abnormal`).
            SkillEffect::BlockActions { .. }
            | SkillEffect::Root
            | SkillEffect::BlockAbnormalSlot { .. }
            // Pure state-flag CC: nothing happens on application beyond the
            // buff landing; the gates read the flag (`game_loop::abnormal`).
            | SkillEffect::Mute
            | SkillEffect::PhysicalMute
            | SkillEffect::DebuffBlock
            | SkillEffect::BlockControl
            // Stealth: the whole mechanic is the `SILENT_MOVE` flag the aggro
            // scan reads (`npc_ai::notices_target`).
            | SkillEffect::SilentMove
            // `Lucky` (194): nothing to do at application time — Java's
            // handler is empty and `isLucky()` reads the buff's presence.
            | SkillEffect::Lucky
            // G34 S3 — flag-only effects, all the same shape as `SilentMove`:
            // nothing happens at application time, the gate reads the flag off
            // the landed buff. `AbnormalShield` has no gate at all, in Java
            // either (see `effect_flag::ABNORMAL_SHIELD`).
            | SkillEffect::BuffBlock
            | SkillEffect::PhysicalShieldAngleAll
            | SkillEffect::Passive
            | SkillEffect::Untargetable
            | SkillEffect::DisableTargeting
            | SkillEffect::PhysicalAttackMute
            | SkillEffect::BlockResurrection
            | SkillEffect::BlockEscape
            | SkillEffect::AbnormalShield
            // `BlockMove`: the whole mechanic is the `IMMOBILIZED` flag the
            // movement gate reads.
            | SkillEffect::BlockMove
            // `ReflectSkill` is a `pump` — a passive stat contribution folded
            // into `StatModifiers` below, nothing to do at application time.
            | SkillEffect::ReflectSkill { .. }
            // A chance-on-hit trigger does nothing when the *carrying* skill is
            // applied; the attack path reads it off the attacker's skill book
            // (`fire_attack_triggers`).
            | SkillEffect::TriggerSkillByAttack { .. }
            // Same for the two other trigger shapes: the damage path
            // (`fire_damage_received_triggers`) and the cast path
            // (`fire_magic_type_triggers`) read them off the bearer's book.
            | SkillEffect::TriggerSkillByDamage { .. }
            | SkillEffect::TriggerSkillByMagicType { .. }
            // Noblesse Blessing: nothing at application time either — the death
            // path reads its `NOBLESS_BLESSING` flag off the landed buff.
            | SkillEffect::NoblesseBless
            // DamageBlock: nothing at application time either — the damage
            // choke point (`game_loop::combat::is_hp_blocked`) reads the
            // `HP_BLOCK` flag off the landed buff.
            | SkillEffect::DamageBlock { .. } => {}
            SkillEffect::FakeDeath { .. } => fake_death(world, target_oid),
            // `Fear.onStart` — the first shove, directly away from the caster.
            // The repeats come off the tick chain (`handle_dam_over_time_tick`),
            // which `schedule_dam_over_time` arms alongside the buff.
            SkillEffect::Fear { .. } => {
                if !fear_can_start(world, target_oid) {
                    continue;
                }
                fear_action(world, Some(caster_oid), target_oid);
            }
            // `TargetCancel.instant` — roll `chance`, then drop the victim's
            // target and abort whatever they were doing (Java also sets the AI
            // to IDLE; the ported AI reaches the same state once the intent is
            // cleared).
            SkillEffect::TargetCancel { chance } => instant::target_cancel(world, &ctx, skill, *chance),
            // `SkillEvasion.onStart` — `addSkillEvasionTypeValue(magicType,
            // amount)`. A per-bucket dodge chance, merged onto the *effected*
            // and unmerged by `handle_buff_expire`.
            SkillEffect::SkillEvasion { magic_type, amount } => {
                // A plain `if let … get_component_mut` would **silently
                // no-op** on a target that has no `StatModifiers` yet — NPCs
                // do not all carry one ([[l2r-conditional-writes-fail-open]]).
                // Insert-then-merge instead.
                let mut mods = world
                    .objects
                    .get_component::<StatModifiers>(&target_oid)
                    .cloned()
                    .unwrap_or_default();
                *mods.skill_evasion.entry(*magic_type).or_insert(0.0) += *amount;
                world.objects.add_components(&target_oid, mods);
            }
            SkillEffect::SkillTurning {
                chance,
                static_chance,
            } => {
                skill_turning(world, caster_oid, target_oid, skill, *chance, *static_chance);
            }
            SkillEffect::TargetMe => target_me(world, caster_oid, target_oid, skill, None),
            SkillEffect::TargetMeProbability { chance } => {
                target_me(world, caster_oid, target_oid, skill, Some(*chance));
            }
            // `GetAgro.instant` — the ported AI derives its attack target
            // fresh from `AggroList::most_hated` every think tick (no cached
            // "current target" field to force directly, unlike Java's AI
            // object), so the faithful equivalent of "force intend-attack the
            // caster" is making the caster's hate dominant: above the current
            // highest entry, not an arbitrary huge constant that would make
            // the taunt unbreakable. `NpcAi::intention` is set the same way
            // `minions::add_hate` does, waking a currently-idle target.
            SkillEffect::GetAgro => force_attack_target(world, target_oid, caster_oid),
            SkillEffect::AddHate { power } => add_hate(world, caster_oid, target_oid, *power),
            SkillEffect::DeleteHate { chance } => delete_hate(world, target_oid, *chance),
            SkillEffect::DeleteHateOfMe { chance } => {
                delete_hate_of_me(world, caster_oid, target_oid, *chance);
            }
            // Periodic effects do nothing on application; their work happens on
            // the tick chain armed by `schedule_dam_over_time`.
            SkillEffect::HealOverTime { .. } | SkillEffect::ManaDamOverTime { .. } | SkillEffect::ManaHealOverTime { .. } | SkillEffect::MpConsumePerLevel { .. } => {}
            // `Relax.onStart` — the toggle seats its holder. Java calls
            // `sitDown(false)`: the un-toggleable form, so the player cannot
            // stand straight back up while the effect is running.
            // `Relax.onStart` / `ChameleonRest.onStart` — both sit the holder
            // down. (Java's NPC branch sets `AI_INTENTION_REST`; no NPC on this
            // dist carries either skill, so there is nothing to route there.)
            SkillEffect::Relax { .. } | SkillEffect::ChameleonRest { .. } => {
                if world.objects.has_component::<crate::model::Player>(&target_oid)
                    && !crate::game_loop::sit_stand::is_sitting(world, target_oid)
                {
                    crate::game_loop::sit_stand::sit_down(world, target_oid);
                }
            }
            // `RebalanceHP.instant` — Balance Life (1043). Pool the HP of every
            // living party member (and their pets/servitors) inside the skill's
            // affect range, take the party's average HP *percentage*, and set
            // everyone to it. A redistribution, not a heal: the total is
            // unchanged, so it robs the healthy to save the dying.
            SkillEffect::RebalanceHp => {
                rebalance_party_hp(world, caster_oid, skill);
            }
            SkillEffect::Cp { amount, percent } => cp(world, target_oid, *amount, *percent),
            // `Transformation.instant` — the state mutation half of
            // `//transform` (display id, collision, granted transform skills,
            // recomputed speed); no broadcast here, since the buff landing
            // below sends `UserInfo`/`CharInfo` and the transform-specific
            // extras (self AVE + SkillList refresh). The cast-time gate in
            // `skills::cast` already refuses a second transform, so this is
            // never reached while `transform_id != 0`.
            SkillEffect::Transform { transformation_id } => {
                crate::game_loop::admin::transforms::apply_transform_state(world, target_oid, *transformation_id);
            }
            SkillEffect::ProtectionBlessing => {}
            // DefenceTrait (Mental Shield / Resist Shock), VampiricAttack
            // (Vampiric Rage), and AttackTrait ("Detect <Category> Weakness"):
            // no instant action. `DefenceTrait`'s resistances are merged when
            // the *buff* lands (`apply_continuous_effects`), not here — this is
            // the instant pass; `VampiricAttack` rides the ordinary stat
            // pipeline (`stat_modifier_effects`) and is read back by the melee
            // damage path; `AttackTrait`'s accumulator is read by
            // `calc_attack_trait_bonus` on every swing and physical skill.
            SkillEffect::DefenceTrait { .. }
            | SkillEffect::VampiricAttack { .. }
            | SkillEffect::AttackTrait { .. } => {}
            // `MagicMpCost`/`Reuse` have no *instant* action: their rates are
            // merged when the buff lands (`apply_continuous_effects`) and
            // unmerged at expiry, exactly like `DefenceTrait`. `DamageShield`
            // is a plain additive stat grant, read off the target when it takes
            // a hit.
            SkillEffect::MagicMpCost { .. }
            | SkillEffect::Reuse { .. }
            | SkillEffect::DamageShield { .. } => {}
        }
    }

    // Spend the spiritshot now that every effect has been applied (Java
    // `Skill`: `unchargeShot(isChargedShot(BLESSED_SPIRITSHOTS) ? BLESSED : SPIRITSHOTS)`).
    if skill.magic_type == 1 && (sps || bss) {
        let shot = if bss {
            crate::model::ShotType::BlessedSpiritshots
        } else {
            crate::model::ShotType::Spiritshots
        };
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&caster_oid)
        {
            p.uncharge_shot(shot);
        }
    }
    // Spend the soulshot on a physical/thrown skill (Java `unchargeShot(SOULSHOTS)`).
    if ss
        && let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&caster_oid)
    {
        p.uncharge_shot(crate::model::ShotType::Soulshots);
    }

    if apply_continuous_effects(world, caster_oid, target_oid, skill, None) {
        dam_over_time_crit_burst(world, caster_oid, target_oid, skill, mcrit);
        share_with_servitor(world, caster_oid, target_oid, skill);
    }
}

/// Java `Skill.applyEffects`' **buff-sharing** branch: a continuous, non-debuff
/// buff that lands on a player is re-applied to each of their servitors.
///
/// The guard is Java's, clause for clause: `_isSharedWithSummon && effected
/// .isPlayer() && effected.hasServitors() && !isTransformation() &&
/// addContinuousEffects && isContinuous() && !_isDebuff`. `addContinuousEffects`
/// is the caller's `apply_continuous_effects` return — sharing only follows a
/// buff that actually landed, so a resisted one is not shared either.
///
/// **A pet is not a servitor here.** Java reads `getServitors()`, and `_pet` is
/// a separate field, so a wolf/hatchling receives nothing — only skill-summoned
/// servitors do. This port carries `ServitorOf` on pets too (they share the
/// owner/follow/AI relationship), which makes `servitor_of` — the `SummonRef
/// .servitor` link, pet excluded — the correct query rather than a component
/// scan that would sweep the pet in.
///
/// `SetSkill.instant` — `effected.getActingPlayer().addSkill(skill, true)`.
///
/// Java's `addSkill(skill, store = true)` writes the skill to the character and
/// persists it; the *client* only learns about it on the next `sendSkillList()`,
/// which Java leaves to whatever ran the effect. This port sends it here
/// instead of relying on a caller to remember, because the alternative is a
/// skill that is real server-side but invisible in the window until relog.
fn set_skill(world: &mut World, player_oid: i32, skill_id: i32, skill_level: i32) {
    if skill_id <= 0 {
        return;
    }
    let Some(book) = world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&player_oid)
    else {
        // Not a player — Java's `if (!effected.isPlayer()) return`.
        return;
    };
    // Java's `addSkill` replaces by id, so a lower-level grant would *downgrade*
    // a skill the player already has further. Nothing on this dist can hit that
    // (the Ancient Books each gate on `OpSkill` for the preceding level), and
    // the guard is cheap insurance against a re-read book undoing progress.
    if book
        .0
        .get(&skill_id)
        .is_some_and(|&have| have >= skill_level)
    {
        return;
    }
    book.0.insert(skill_id, skill_level);
    // A granted passive has to start contributing now, not at the next login.
    crate::game_loop::passive_skills::recompute_conditioned_passives(world, player_oid);
    if let Some(pkt) = crate::game_loop::helpers::skill_list_packet(world, player_oid) {
        send_to_player(world, player_oid, pkt);
    }
}

/// `MapRegionManager.getTeleToLocation(player, where)` for the destinations
/// [`SkillEffect::Escape`] can name.
///
/// The structure is Java's, and the important part of it is that the residence
/// branches **fall through**: `getTeleToLocation` only returns early when it
/// resolves a residence, so a Scroll of Escape: Castle used by a clanless
/// player is not a wasted scroll — it is a town escape. Only the *blessed*
/// scrolls refuse, and they refuse in their `OpHome` condition before the
/// effect ever runs.
fn escape_to(world: &mut World, player_oid: i32, dest: crate::model::skill::EscapeDest) {
    use crate::model::skill::EscapeDest;

    if let Some((x, y, z)) = match dest {
        EscapeDest::Town => None,
        EscapeDest::ClanHall => clan_hall_escape(world, player_oid),
        EscapeDest::Castle => castle_escape(world, player_oid),
    } {
        crate::game_loop::death::teleport_player(world, player_oid, x, y, z);
        return;
    }

    // `TeleportWhereType.TOWN`: the enclosing map region's respawn, a random
    // point when `RandomRespawnInTownEnabled`.
    // The position check is `teleport_to_town`'s too, but bailing here keeps a
    // positionless caster from consuming a roll off the world RNG.
    if maybe_position(world, player_oid).is_none() {
        return;
    }
    let pick = if world.cfg.character.random_respawn_in_town {
        world.roll(64) as usize
    } else {
        0
    };
    crate::game_loop::death::teleport_to_town(world, player_oid, pick);
}

/// `ClanHallData.getClanHallByClan(clan).getOwnerLocation()` — the hall's
/// `<ownerRestartPoint>`. `None` (→ town) when the player has no clan or the
/// clan holds no hall.
fn clan_hall_escape(world: &World, player_oid: i32) -> Option<(i32, i32, i32)> {
    let clan_id = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)?
        .clan_id;
    if clan_id == 0 {
        return None;
    }
    world
        .clan_halls
        .values()
        .find(|h| h.owner_id == clan_id)
        .map(|h| h.owner_restart)
}

/// `CastleManager.getCastleByOwner(clan)`, falling back to "standing on the
/// ground of a castle my clan is *defending* in a live siege" — Java accepts
/// both. The point itself is the residence zone's `getSpawnLoc()`, or
/// `getChaoticSpawnLoc()` for a player with negative reputation.
fn castle_escape(world: &mut World, player_oid: i32) -> Option<(i32, i32, i32)> {
    use crate::model::siege::SiegeClanType;

    let (clan_id, reputation) = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .map(|p| (p.clan_id, p.reputation))?;
    if clan_id == 0 {
        return None;
    }
    let owned = world
        .clans
        .get(&clan_id)
        .map(|c| c.castle_id)
        .filter(|id| *id > 0);
    let castle_id = match owned {
        Some(id) => id,
        None => {
            // Not the owner: only a defender standing on castle ground during
            // a live siege qualifies.
            let pos = world
                .objects
                .get_component::<crate::model::components::Position>(&player_oid)?;
            let id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z)?;
            let siege = world.sieges.get(&id)?;
            let defending = siege.in_progress
                && siege.clans.iter().any(|c| {
                    c.clan_id == clan_id
                        && matches!(c.kind, SiegeClanType::Owner | SiegeClanType::Defender)
                });
            defending.then_some(id)?
        }
    };
    let pick = world.roll(64) as usize;
    world
        .data
        .castle_restart_points
        .get(&castle_id)?
        .pick(reputation < 0, pick)
}

/// The recursion terminates: the servitor is an NPC, so the re-applied cast
/// fails this function's `isPlayer()` clause and shares no further.
fn share_with_servitor(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    if !skill.shared_with_summon || skill.is_debuff || !skill.is_continuous {
        return;
    }
    // Java `isTransformation()` — a transform is not shared onto the summon.
    if skill.abnormal_type == "TRANSFORM" || skill.abnormal_type == "CHANGEBODY" {
        return;
    }
    // `effected.isPlayer()`: only a buff landing on a *player* shares onward.
    if !world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
    {
        return;
    }
    let Some(servitor) = crate::game_loop::servitor::servitor_of(world, target_oid) else {
        return;
    };
    apply_skill_effects(world, caster_oid, servitor, skill);
}

/// `DamOverTime.onStart` — a magic (non-toggle) DoT bursts for `power * 10` on
/// a magic-crit ("Tests show that 10 times HP DOT is taken during magic
/// critical"), clamped to leave the target alive unless `canKill`.
///
/// **Only on a debuff that landed.** `onStart` is driven by
/// `EffectList.add(info)`, which `Skill.applyEffects` reaches only when
/// `addContinuousEffects` — i.e. `calcEffectSuccess` — was true. Java carries
/// an inline note that `M.Crit can occur even if this skill is resisted` at that
/// very spot, but that is aspirational: the shipped code does not burst on a
/// resist, and neither does this.
///
/// (Java re-rolls the crit inside `onStart` rather than reusing the cast's;
/// the port passes the cast's roll through, which keeps one roll per cast.)
fn dam_over_time_crit_burst(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    mcrit: bool,
) {
    if skill.magic_type != 1 || !mcrit {
        return;
    }
    for effect in &skill.effects {
        let SkillEffect::DamOverTime {
            power,
            ticks,
            can_kill,
        } = effect
        else {
            continue;
        };
        if *ticks <= 0 {
            continue;
        }
        let mut damage = *power * 10.0;
        if !*can_kill {
            let cur_hp = world
                .objects
                .get_component::<Vitals>(&target_oid)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0);
            if damage >= cur_hp - 1.0 {
                damage = cur_hp - 1.0;
            }
        }
        if damage > 0.0 {
            let caster_name = player_name_or_empty(world, caster_oid);
            apply_skill_damage(
                world,
                caster_oid,
                target_oid,
                SkillHit {
                    damage,
                    crit: true,
                    is_magic: true,
                    caster_name: &caster_name,
                    over_hit: skill.over_hit,
                    skill_id: skill.id,
                    ..Default::default()
                },
            );
        }
    }
}

/// Java's `BlockChat`/`BlockParty`/`BlockAction` `onStart`: a punishment with
/// expiration `0` — "forever" — because the *buff* is the timer. The matching
/// `onExit` in `handle_buff_expire_inner` stops it.
pub(crate) fn start_bot_report_punishment(
    world: &mut World,
    player_oid: i32,
    ptype: PunishmentType,
) {
    crate::game_loop::punishment::start_punishment(
        world,
        player_oid.to_string(),
        PunishmentAffect::Character,
        ptype,
        0,
        "block action debuff".to_string(),
        "system".to_string(),
    );
}

/// The `onExit` twin.
pub(crate) fn stop_bot_report_punishment(
    world: &mut World,
    player_oid: i32,
    ptype: PunishmentType,
) {
    crate::game_loop::punishment::stop_punishment(
        world,
        &player_oid.to_string(),
        PunishmentAffect::Character,
        ptype,
    );
}

#[cfg(test)]
mod manor_calc_tests {
    use super::{calc_harvest_success, calc_sow_success};

    #[test]
    fn sow_success_is_level_scaled() {
        // A well-matched sow (seed lvl 10, mob lvl 10, player lvl 10): base 90%.
        // roll 0 succeeds, roll 89 succeeds, roll 90 fails.
        assert!(calc_sow_success(10, false, 10, 10, 0));
        assert!(calc_sow_success(10, false, 10, 10, 89));
        assert!(!calc_sow_success(10, false, 10, 10, 90));
        // The alternative seed's base is only 20%.
        assert!(calc_sow_success(10, true, 10, 10, 19));
        assert!(!calc_sow_success(10, true, 10, 10, 20));
        // Java quirk: a big mismatch is NOT floored at 1% — a mob 20 levels over
        // the seed's band drives the chance ≤0, so even roll 0 fails.
        assert!(!calc_sow_success(10, false, 10, 40, 0));
    }

    #[test]
    fn harvest_success_is_floored_at_one_percent() {
        // Matched levels: 100% (any roll 0..98 succeeds).
        assert!(calc_harvest_success(10, 10, 98));
        // A large gap is clamped to 1% (unlike sow): roll 0 still succeeds.
        assert!(calc_harvest_success(10, 90, 0));
        assert!(!calc_harvest_success(10, 90, 1));
    }
}
