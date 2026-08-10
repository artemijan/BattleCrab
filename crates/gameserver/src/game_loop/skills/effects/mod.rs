//! Effect application — Java's `AbstractEffect` implementations.
//!
//! `apply_skill_effects` (the instant dispatch) lives here; its fattest arms
//! are one function apiece in the sibling [`super::instant`] module. The rest
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
use crate::game_loop::guard::position;
use crate::game_loop::helpers::client_for_player;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::pos_of;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::skill_by_id;
use crate::model::components::{BaseStats, Buffs, CombatStats, Speeds, StatModifiers, Vitals};
use crate::model::formulas;
use crate::model::punishment::{PunishmentAffect, PunishmentType};
use crate::model::skill::{
    ActiveBuff, BuffSlot, DispelSlot, RestorationGroup, Skill, SkillEffect, abnormal_type_client_id,
};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::instant;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::send_sm_to_player;
use crate::game_loop::helpers::send_to_player;
use crate::game_loop::helpers::{send_sm_bare_to_client, send_sm_to_client};

mod continuous;
pub(crate) mod control;
pub(crate) mod damage;
mod gathering;
mod support;
mod ticks;
mod traits;
mod triggers;

pub(crate) use continuous::*;
pub(crate) use control::*;
pub(crate) use damage::*;
pub(crate) use gathering::*;
pub(crate) use support::*;
pub(crate) use ticks::*;
pub(crate) use traits::*;
pub(crate) use triggers::*;

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
        crate::network::server_packets::sm_ids::C1_DODGED_THE_ATTACK,
        &[crate::network::server_packets::SmParam::Text(target_name)],
    );
    send_sm_to_player(
        world,
        target_oid,
        crate::network::server_packets::sm_ids::YOU_HAVE_DODGED_C1_S_ATTACK,
        &[crate::network::server_packets::SmParam::Text(caster_name)],
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
    let base_stat = match ordinal as i32 {
        0 => BaseStat::Str,
        1 => BaseStat::Dex,
        2 => BaseStat::Con,
        3 => BaseStat::Int,
        4 => BaseStat::Wit,
        5 => BaseStat::Men,
        _ => return false,
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
        crate::scheduler::ScheduledTask::ResetCharges { player_oid, seq },
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
    use server_packets::{SmParam, sm_ids};

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
            // `SummonNpc.instant` — the `EffectPoint` branch drops the symbol
            // totems (PLAN_G19_SYMBOLS.md); every other template type takes
            // Java's **default** plain-spawn branch (the Holiday Trees and
            // Squash/Watermelon seeds — item-cast carriers, so "learnable"
            // was never the right reachability test here). SKIP(G19): the
            // `Decoy` branch — no reachable skill on this dist summons a
            // template of type `Decoy` (Decoy 525 has no tree row or item
            // grant; item 13769's "Life-size Decoy" 32544 is type `Folk`;
            // verified 2026-08-06).
            SkillEffect::SummonNpc { npc_id, npc_count, despawn_delay } => {
                // Java: effected must be a live player (dead/observer gated).
                let effected_alive_player = world.objects.has_component::<crate::model::Player>(&target_oid)
                    && world
                        .objects
                        .get_component::<Vitals>(&target_oid)
                        .is_some_and(|v| !v.dead);
                if !effected_alive_player {
                    continue;
                }
                // `if (player.isMounted()) return;`
                if world
                    .objects
                    .get_component::<crate::model::Player>(&target_oid)
                    .is_some_and(|p| p.is_mounted())
                {
                    continue;
                }
                // GROUND skills spawn at the stored world position; everything
                // else at the effected creature (`SummonNpc.instant`).
                let fallback = pos_of(world, target_oid)
                    .unwrap_or((0, 0, 0));
                let (x, y, z) = if skill.target_type == crate::model::skill::TargetType::Ground {
                    world
                        .objects
                        .get_component::<crate::model::components::GroundSkillTarget>(&target_oid)
                        .map(|g| (g.x, g.y, g.z))
                        .unwrap_or(fallback)
                } else {
                    fallback
                };
                let is_effect_point = world
                    .data
                    .npc_data
                    .get(*npc_id)
                    .is_some_and(|t| t.type_name == "EffectPoint");
                for _ in 0..(*npc_count).max(1) {
                    if is_effect_point {
                        crate::game_loop::effect_point::spawn_effect_point(
                            world,
                            target_oid,
                            *npc_id,
                            x,
                            y,
                            z,
                            *despawn_delay,
                        );
                    } else {
                        crate::game_loop::effect_point::spawn_plain_summon(
                            world,
                            target_oid,
                            *npc_id,
                            x,
                            y,
                            z,
                            *despawn_delay,
                        );
                    }
                }
            }
            SkillEffect::MagicalAttack { power } => instant::magical_attack(world, &ctx, skill, *power),
            SkillEffect::MagicalAttackRange { power, shield_def_percent } => {
                instant::magical_attack_range(world, &ctx, skill, *power, *shield_def_percent)
            }
            // The MP-restore family (`ManaHeal`, `ManaHealByLevel`,
            // `ManaHealPercent`, `Mp`). Four Java handlers, four amount
            // formulas, one shared apply path — see `restore_mp`.
            SkillEffect::ManaHeal { power }
            | SkillEffect::ManaHealByLevel { power }
            | SkillEffect::ManaHealPercent { power } => {
                let max_mp = world.objects.get_component::<Vitals>(&target_oid).map(|v| v.max_mp as f64).unwrap_or(0.0);
                let amount = match effect {
                    // `ManaHealPercent`: a straight share of the pool. Java
                    // special-cases `power == 100` to the full pool, which is
                    // the same number the multiply gives — kept as one branch.
                    SkillEffect::ManaHealPercent { .. } => (max_mp * *power) / 100.0,
                    // `ManaHeal`: flat power, then the recipient's
                    // `MANA_CHARGE`. Java skips that for a *static* skill; no
                    // skill in this family is static, so it always applies.
                    SkillEffect::ManaHeal { .. } => mana_charge_of(world, target_oid, *power),
                    // `ManaHealByLevel`: `MANA_CHARGE` first, *then* the
                    // level-gap penalty.
                    _ => {
                        let charged = mana_charge_of(world, target_oid, *power);
                        charged * recharge_level_penalty(target_level(world, target_oid), skill.magic_level)
                    }
                };
                restore_mp(world, caster_oid, target_oid, amount);
            }
            // Java's `Mp` handler: `amount`, flat or as a share of max MP.
            SkillEffect::MpRestore { amount, percent } => {
                let max_mp = world.objects.get_component::<Vitals>(&target_oid).map(|v| v.max_mp as f64).unwrap_or(0.0);
                let amount = if *percent { (max_mp * *amount) / 100.0 } else { *amount };
                restore_mp(world, caster_oid, target_oid, amount);
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
            // `RandomizeHate.instant` — move the *caster's* accumulated hate
            // onto a random bystander, so the mob rounds on someone else
            // instead of simply forgetting (Confusion 2, Switch 12).
            SkillEffect::RandomizeHate { chance } => {
                // Java: `if ((effected == effector) || !effected.isAttackable()) return;`
                if target_oid == caster_oid || !crate::game_loop::combat::is_npc_oid(target_oid) {
                    continue;
                }
                if !confuse_chance_passes(world, caster_oid, target_oid, skill, *chance) {
                    continue;
                }
                // The exclusions are wider here than for `Confuse`: never the
                // caster, and never a same-faction attackable ("aggro cannot be
                // transfered to a mob of the same faction").
                let Some(victim) = random_bystander(world, target_oid, caster_oid, true) else { continue };
                if let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&target_oid) {
                    // `getHating` → `stopHating` → `addDamageHate(target, 0, hate)`:
                    // the hate is *moved*, not duplicated.
                    let hate = aggro.0.get(&caster_oid).map(|i| i.hate).unwrap_or(0.0);
                    aggro.0.remove(&caster_oid);
                    aggro.0.entry(victim).or_default().hate += hate;
                }
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
                // `PhysicalAttack.instant()`: crit is rolled here (per-effect in
                // Java), not the once-per-cast magic roll above.
                let (p_atk, level, str_bonus, random_dmg, caster_name) = {
                    let cs = world.objects.get_component::<CombatStats>(&caster_oid);
                    let p_atk = cs.map(|c| c.p_atk).unwrap_or(0.0);
                    let random_dmg = cs.map(|c| c.random_dmg).unwrap_or(0);
                    let str_bonus = world
                        .objects
                        .get_component::<BaseStats>(&caster_oid)
                        .map(|b| world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Str, b.str_))
                        .unwrap_or(1.0);
                    (p_atk, caster_level(world, caster_oid), str_bonus, random_dmg, caster_display_name(world, caster_oid))
                };
                // Java folds `pDefMod` in *before* the shield add, so the
                // shield's own sDef is never scaled by it.
                let base_defence = target_p_def(world, target_oid) * *p_def_mod;
                let defence = defence_after_shield(world, target_oid, base_defence, *ignore_shield_defence);
                let crit = formulas::calc_physical_skill_crit(*critical_chance, str_bonus, world.roll(100));
                let rand_roll = if random_dmg > 0 { world.roll(2 * random_dmg + 1) - random_dmg } else { 0 };
                // A perfect block is a flat 1, whatever the rest would say.
                let damage = match defence {
                    None => 1.0,
                    Some(defence) => {
                        // `weaponMod` is **70 with a `+pAtk+power` bonus term**
                        // for a ranged weapon, 77 for melee — the difference
                        // between an archer's skill and a swordsman's.
                        let ranged = crate::game_loop::ranged::is_ranged(
                            crate::game_loop::ranged::equipped_weapon_type(world, caster_oid)
                                .unwrap_or_default(),
                        );
                        formulas::calc_physical_skill_damage(
                            p_atk,
                            *p_atk_mod,
                            defence,
                            1.0, // already folded into `defence` above
                            *power,
                            formulas::level_mod(level),
                            formulas::random_damage_multiplier(rand_roll),
                            crit,
                            crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, false),
                            ss,
                            ranged,
                        ) * attribute_mod(world, caster_oid, target_oid, skill)
                            * skill_trait_mod(world, caster_oid, target_oid, skill, true)
                            * skill_power_mul(world, caster_oid, false)
                            * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill))
                    }
                };
                // `PhysicalAttackHpLink`'s tail: the same shape as `DeathLink`,
                // keyed on the **caster's** missing HP. At full health the
                // multiplier is 0 — Fatal Counter fired by a healthy archer
                // does nothing at all.
                let damage = if matches!(effect, SkillEffect::PhysicalAttackHpLink { .. }) {
                    let v = world.objects.get_component::<Vitals>(&caster_oid).copied();
                    match v {
                        Some(v) if v.max_hp > 0 => {
                            damage * (-((v.cur_hp * 2.0) / v.max_hp as f64) + 2.0)
                        }
                        _ => damage,
                    }
                } else {
                    damage
                };
                apply_skill_damage(
                    world,
                    caster_oid,
                    target_oid,
                    SkillHit {
                        damage,
                        crit,
                        caster_name: &caster_name,
                        over_hit: skill.over_hit,
                        skill_id: skill.id,
                        ..Default::default()
                    },
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
            // `Betray.onStart` — the servitor turns on its owner. `canStart`
            // requires a player effector and a summon effected, so this is
            // aimed at somebody *else's* pet. The `BETRAYED` flag (which stops
            // it obeying and makes it auto-attackable) rides the landed buff;
            // what happens here is the AI flip.
            SkillEffect::Betray => {
                let Some(owner) = servitor_owner_of(world, target_oid) else {
                    continue; // not a summon — Java's `canStart` refuses
                };
                if !world
                    .objects
                    .has_component::<crate::model::Player>(&caster_oid)
                {
                    continue;
                }
                // `getAI().setIntention(ATTACK, getActingPlayer())` — the
                // servitor's own owner becomes its target. Routed through the
                // ordinary attack order so it stops following, takes the top
                // hate slot and arms the attack timeout exactly like a
                // commanded attack would.
                crate::game_loop::servitor::servitor_attack(world, owner, owner);
            }
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
            // `Bluff.instant` — spin the target to face the caster's heading.
            // Raid bosses and their minions are immune (Java also names NPC
            // 35062, a siege headquarters, explicitly); the pair of rotation
            // packets is what the client animates, and the server-side heading
            // change is what makes a subsequent Backstab land.
            SkillEffect::Bluff { chance } => {
                let is_raid = world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&target_oid)
                    .and_then(|n| world.data.npc_data.get(n.npc_id))
                    .is_some_and(|t| t.is_raid())
                    // Java's `isRaidMinion()` is `Monster.onSpawn`'s
                    // `setIsRaidMinion(_master.isRaid())` — a minion inherits
                    // its master's raid immunity. The port tracks the link as
                    // `MinionOf`, so ask the master's template.
                    || crate::game_loop::minions::is_raid_minion(world, target_oid);
                if is_raid || !confuse_chance_passes(world, caster_oid, target_oid, skill, *chance) {
                    continue;
                }
                let Some(caster_heading) = world
                    .objects
                    .get_component::<crate::model::components::Position>(&caster_oid)
                    .map(|p| p.heading)
                else {
                    continue;
                };
                let target_heading = world
                    .objects
                    .get_component::<crate::model::components::Position>(&target_oid)
                    .map(|p| p.heading)
                    .unwrap_or(0);
                if let Some(region) = region_cell_of(world, target_oid)
                {
                    for pkt in [
                        server_packets::start_rotation(target_oid, target_heading, 1, 65535),
                        server_packets::stop_rotation(target_oid, caster_heading, 65535),
                    ] {
                        crate::game_loop::helpers::broadcast_near_region(world, region, &pkt);
                    }
                }
                if let Some(p) = world
                    .objects
                    .get_component_mut::<crate::model::components::Position>(&target_oid)
                {
                    p.heading = caster_heading;
                }
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
            // `HpByLevel.instant` — heals the **effector**. Life Scavenge (46)
            // and Corpse Life Drain (1151) drain a corpse to top the *caster*
            // up, so the target is only the corpse being consumed.
            SkillEffect::HpByLevel { power } => {
                let Some(v) = world.objects.get_component::<Vitals>(&caster_oid).copied() else {
                    continue;
                };
                // Java clamps to `getMaxHp()` here, **not** to
                // `getMaxRecoverableHp()` — the one heal in this family that
                // ignores the recoverable cap. Ported as written.
                let restored = ((v.cur_hp + *power).min(v.max_hp as f64) - v.cur_hp).trunc();
                if restored <= 0.0 {
                    continue;
                }
                if let Some(v) = world.objects.get_component_mut::<Vitals>(&caster_oid) {
                    v.cur_hp += restored;
                }
                send_sm_to_player(world, caster_oid, sm_ids::S1_HP_HAS_BEEN_RESTORED, &[SmParam::Int(restored as i32)]);
                broadcast_vitals(world, caster_oid);
            }
            SkillEffect::Heal { power } => instant::heal(world, &ctx, skill, *power),
            SkillEffect::HealPercent { power } => instant::heal_percent(world, &ctx, skill, *power),
            SkillEffect::FocusMomentum { amount, max_charges } => {
                // Java's own hardcoded fallback for the never-set-in-this-
                // datapack `MAX_MOMENTUM` stat — see the type's doc comment.
                let max = (*max_charges).min(8);
                let current = world.objects.get_component::<crate::model::Player>(&target_oid).map(|p| p.charges).unwrap_or(0);
                let Some(client_id) = client_for_player(world, target_oid) else { continue };
                if current >= max {
                    send_to_client(world, client_id, server_packets::system_message_with(sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY, &[]));
                    continue;
                }
                let new_charge = (current + *amount).min(max);
                if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&target_oid) {
                    p.charges = new_charge;
                }
                // `setCharges` restarts the decay clock.
                arm_charge_decay(world, target_oid);
                if new_charge == max {
                    send_sm_bare_to_client(world, client_id, sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY);
                } else {
                    send_sm_to_client(
                        world,
                        client_id,
                        sm_ids::YOUR_FORCE_HAS_INCREASED_TO_LEVEL_S1,
                        &[SmParam::Int(new_charge)],
                    );
                }
                crate::game_loop::helpers::send_etc_status_update(world, client_id, target_oid);
            }
            SkillEffect::EnergyAttack { power, critical_chance, p_def_mod, charge_consume, ignore_shield_defence } => instant::energy_attack(world, &ctx, skill, *power, *critical_chance, *p_def_mod, *charge_consume, *ignore_shield_defence),
            SkillEffect::GiveItem { item_id, item_count, item_enchant_level } => {
                give_item(world, target_oid, *item_id, *item_count, *item_enchant_level);
            }
            SkillEffect::GiveItemRandom { groups } => {
                give_item_random(world, target_oid, groups);
            }
            // `GiveSp.instant` — SP Scrolls and the Primeval Isle crystals.
            //
            // Java credits the **effector**, guards on both ends being players
            // and on the effected not being alike-dead, and calls the plain
            // two-arg `addExpAndSp` — which is `useBonuses = false`, so no
            // vitality or rate multiplier applies to a scroll.
            SkillEffect::GiveSp { sp } => {
                let both_players = world
                    .objects
                    .has_component::<crate::model::Player>(&caster_oid)
                    && world
                        .objects
                        .has_component::<crate::model::Player>(&target_oid);
                let dead = is_dead(world, target_oid);
                if both_players && !dead {
                    crate::game_loop::death::add_exp_and_sp(
                        world,
                        caster_oid,
                        0.0,
                        *sp as f64,
                        false,
                    );
                }
            }
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
                control::teleport_to_target(world, caster_oid, target_oid);
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
                control::call_pc_player(world, caster_oid, target_oid, *item_id, *item_count);
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
                // `OpenCommonRecipeBook`/`OpenDwarfRecipeBook.instant`: players
                // only, refused while a private store (incl. manufacture) is up,
                // then `RecipeManager.requestBookOpen`.
                if world.objects.get_component::<crate::model::Player>(&caster_oid).is_some() {
                    let store_type = world
                        .objects
                        .get_component::<crate::model::Player>(&caster_oid)
                        .map(|p| p.store_type)
                        .unwrap_or(0);
                    if store_type != 0 {
                        send_sm(world, caster_oid, sm_ids::ITEM_CREATION_IS_NOT_POSSIBLE_WHILE_ENGAGED_IN_A_TRADE);
                    } else if let Some(cid) = client_for_player(world, caster_oid) {
                        crate::game_loop::crafting::request_book_open(world, cid, *dwarven);
                    }
                }
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
            // `DispelBySlotMyself.instant` — same shape as `DispelBySlot` with
            // two differences that both matter: the list carries **no levels**
            // (every level of a listed abnormal goes), and an
            // **`irreplacableBuff` is spared**, which is what stops Flames of
            // Invincibility from stripping the clan/transform buffs that
            // `isStayAfterDeath()` also protects.
            SkillEffect::DispelBySlotMyself { dispel } => {
                let candidates: Vec<(i32, i32)> = world
                    .objects
                    .get_component::<Buffs>(&target_oid)
                    .map(|buffs| buffs.0.iter().map(|b| (b.skill_id, b.skill_level)).collect())
                    .unwrap_or_default();
                let to_dispel: Vec<i32> = candidates
                    .into_iter()
                    .filter(|&(sid, slvl)| {
                        world.data.skill_data.get(sid, slvl).is_some_and(|bs| {
                            // `!info.getSkill().isIrreplacableBuff()` — the port
                            // folds that tag into `stay_after_death` (G34 S3),
                            // which is the same predicate Java's getter uses.
                            !bs.stay_after_death && dispel.contains(&bs.abnormal_type)
                        })
                    })
                    .map(|(sid, _)| sid)
                    .collect();
                for skill_id in to_dispel {
                    handle_buff_expire(world, target_oid, skill_id);
                }
            }
            // `DispelAll.instant` — `effected.stopAllEffects()`, i.e.
            // `stopEffects(b -> !b.getSkill().isIrreplacableBuff())`: no
            // abnormal-type list and no level ranking, just "everything that is
            // not irreplacable". Skill 4177 Cancellation, the raid-boss sweep.
            //
            // The predicate is `stay_after_death` for the same reason
            // `DispelBySlotMyself` uses it: this port folds `<irreplacableBuff>`
            // into that field (G34 S3) and cannot tell the three source tags
            // apart. It spares marginally more than Java here — a skill with
            // only `<stayAfterDeath>` survives a cancel it would lose upstream
            // — which is the conservative direction for a buff-stripping
            // effect, and consistent with the arm above.
            SkillEffect::DispelAll => {
                let candidates: Vec<(i32, i32, bool)> = world
                    .objects
                    .get_component::<Buffs>(&target_oid)
                    .map(|buffs| {
                        buffs
                            .0
                            .iter()
                            .map(|b| (b.skill_id, b.skill_level, b.passive))
                            .collect()
                    })
                    .unwrap_or_default();
                let to_dispel: Vec<i32> = candidates
                    .into_iter()
                    // Passives never sit in Java's `_actives` list at all.
                    .filter(|&(_, _, passive)| !passive)
                    .filter(|&(sid, slvl, _)| {
                        world
                            .data
                            .skill_data
                            .get(sid, slvl)
                            .is_some_and(|bs| !bs.stay_after_death)
                    })
                    .map(|(sid, _, _)| sid)
                    .collect();
                for skill_id in to_dispel {
                    handle_buff_expire(world, target_oid, skill_id);
                }
            }
            // `Grow` is a continuous-only pair of hooks (onStart/onExit);
            // both live on the buff apply/expire path, so the instant
            // dispatch has nothing to do for it.
            SkillEffect::Grow => {}
            SkillEffect::DispelBySlot { dispel } => instant::dispel_by_slot(world, &ctx, skill, dispel),
            SkillEffect::DispelBySlotProbability { dispel, rate } => instant::dispel_by_slot_probability(world, &ctx, skill, dispel, *rate),
            // `DispelByCategory.instant` — the "Cancel" family (Cancellation,
            // Cleanse, Purification Field, Touch of Death): unlike
            // `DispelBySlot`/`DispelBySlotProbability` (a fixed abnormal-type
            // list) this steals *whatever* is up. `BUFF` walks dances then
            // buffs in reverse cast order (Java's `getDances()`/`getBuffs()`
            // reversed); `DEBUFF` walks debuffs. Both stop once `max` buffs
            // are collected. `ALL` is dead in Java too (no shipped skill uses
            // it) and is a no-op here.
            SkillEffect::DispelByCategory { slot, rate, max } => {
                if is_dead(world, target_oid) {
                    continue;
                }
                let candidates: Vec<(i32, i32, BuffSlot)> = world
                    .objects
                    .get_component::<Buffs>(&target_oid)
                    .map(|buffs| buffs.0.iter().rev().map(|b| (b.skill_id, b.skill_level, b.slot)).collect())
                    .unwrap_or_default();
                let mut to_dispel: Vec<i32> = Vec::new();
                match slot {
                    DispelSlot::Buff => {
                        // `Formulas.calcCancelSuccess`'s only consumer of
                        // `Stat.RESIST_DISPEL_BUFF` — pumped by `ResistDispelByCategory`
                        // since an earlier slice but unread until now.
                        let resist = world
                            .objects
                            .get_component::<StatModifiers>(&target_oid)
                            .map(|m| crate::model::finalize(m, crate::model::stats::Stat::ResistDispelBuff, 1.0))
                            .unwrap_or(1.0);
                        for want in [BuffSlot::Dance, BuffSlot::Buff] {
                            for &(sid, slvl, _) in candidates.iter().filter(|&&(_, _, s)| s == want) {
                                if to_dispel.len() >= *max as usize {
                                    break;
                                }
                                let Some(bs) = world.data.skill_data.get(sid, slvl) else { continue };
                                // `canBeStolen()`: passive/toggle/debuff are
                                // already excluded by the `Dance`/`Buff` slot
                                // filter above. `isIrreplacableBuff()`/hero/GM/
                                // static-skill exclusions aren't modeled.
                                if !bs.can_be_dispelled {
                                    continue;
                                }
                                let hit = *rate >= 100 || {
                                    let chance = *rate as f64
                                        + ((skill.magic_level - bs.magic_level) as f64 * 2.0)
                                        + ((bs.abnormal_time / 120) as f64 * resist);
                                    world.roll(100) < (chance as i32).clamp(25, 75)
                                };
                                if hit {
                                    to_dispel.push(sid);
                                }
                            }
                        }
                    }
                    DispelSlot::Debuff => {
                        for &(sid, slvl, _) in &candidates {
                            if to_dispel.len() >= *max as usize {
                                break;
                            }
                            let Some(bs) = world.data.skill_data.get(sid, slvl) else { continue };
                            if !bs.is_debuff || !bs.can_be_dispelled {
                                continue;
                            }
                            if world.roll(100) <= *rate {
                                to_dispel.push(sid);
                            }
                        }
                    }
                    DispelSlot::All => {}
                }
                for skill_id in to_dispel {
                    handle_buff_expire(world, target_oid, skill_id);
                }
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
            // `FakeDeath.onStart` → `Creature.startFakeDeath()`: drop whatever
            // you were doing and hit the deck. `isAlikeDead()` then covers the
            // rest (no aggro, no being targeted), and the client is told with
            // `ChangeWaitType(WT_START_FAKEDEATH)`.
            //
            // Java's `FAKE_DEATH_UNTARGET` block (clearing the fake-dead player
            // off everyone else's target) is **False** on this dist's
            // `Character.ini`, so it is deliberately not ported.
            SkillEffect::FakeDeath { .. } => {
                // Players only — Java's `startFakeDeath` returns immediately
                // for anything else.
                if client_for_player(world, target_oid).is_none() {
                    continue;
                }
                world.objects.remove_component::<crate::model::components::Intent>(&target_oid);
                if world.objects.has_component::<crate::model::components::Casting>(&target_oid) {
                    crate::game_loop::skills::cast::stop_casting(world, target_oid);
                }
                // `startFakeDeath` calls `abortAttack()` too: you cannot play
                // dead and still land the swing you were mid-way through.
                crate::game_loop::combat::abort_attack(world, target_oid);
                world.objects.remove_component::<crate::model::components::Movement>(&target_oid);
                broadcast_change_wait_type(world, target_oid, server_packets::wait_type::START_FAKEDEATH);
            }
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
            // `SkillTurning.instant` — Spell Turning (1412). Offensive despite
            // the name: it breaks the *target's* cast. Java bails on a
            // self-cast and on raid bosses, and rolls `Rnd.get(100) < chance`
            // unless `staticChance`, which routes through `calcProbability`
            // (level-aware) instead. No dist skill sets `staticChance`.
            SkillEffect::SkillTurning {
                chance,
                static_chance,
            } => {
                let is_raid = world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&target_oid)
                    .and_then(|n| world.data.npc_data.get(n.npc_id))
                    .is_some_and(|t| t.is_raid());
                if caster_oid == target_oid || is_raid {
                    continue;
                }
                let passes = if *static_chance {
                    confuse_chance_passes(world, caster_oid, target_oid, skill, *chance)
                } else {
                    world.roll(100) < *chance
                };
                if passes {
                    crate::game_loop::skills::cast::break_cast(world, target_oid);
                }
            }
            // `TargetMe` / `TargetMeProbability` — the *playable*-side taunt.
            // Java wraps both in `if (effected.isPlayable())`, so taunting a
            // **monster** through these does nothing at all; a mob's aggro
            // comes from the `AddHate`/`GetAgro` effects the same skills carry.
            // The pair differ in exactly two ways: `TargetMe` is a continuous
            // effect that also **locks** the target (cleared on expiry), while
            // `TargetMeProbability` is instant, chance-rolled and lock-free.
            SkillEffect::TargetMe | SkillEffect::TargetMeProbability { .. } => {
                if !world.objects.has_component::<crate::model::Player>(&target_oid) {
                    continue;
                }
                if let SkillEffect::TargetMeProbability { chance } = effect
                    && !confuse_chance_passes(world, caster_oid, target_oid, skill, *chance)
                {
                    continue;
                }
                // `if (effected.getTarget() != effector) effected.setTarget(effector)`
                // — through the client-notifying setter so the selection ring
                // actually moves.
                let already = world
                    .objects
                    .get_component::<crate::model::components::TargetRef>(&target_oid)
                    .and_then(|t| t.0);
                if already != Some(caster_oid)
                    && let Some(client_id) = client_for_player(world, target_oid)
                {
                    crate::game_loop::target::set_target(
                        world,
                        client_id,
                        target_oid,
                        Some(caster_oid),
                    );
                }
                if matches!(effect, SkillEffect::TargetMe) {
                    world.objects.add_components(
                        &target_oid,
                        crate::model::components::LockedTarget(caster_oid),
                    );
                }
            }
            // `GetAgro.instant` — the ported AI derives its attack target
            // fresh from `AggroList::most_hated` every think tick (no cached
            // "current target" field to force directly, unlike Java's AI
            // object), so the faithful equivalent of "force intend-attack the
            // caster" is making the caster's hate dominant: above the current
            // highest entry, not an arbitrary huge constant that would make
            // the taunt unbreakable. `NpcAi::intention` is set the same way
            // `minions::add_hate` does, waking a currently-idle target.
            SkillEffect::GetAgro => {
                let Some(aggro) = world.objects.get_component::<crate::model::npc::AggroList>(&target_oid) else { continue };
                let max_hate = aggro.0.values().map(|i| i.hate).fold(0.0_f64, f64::max);
                if let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&target_oid) {
                    aggro.0.entry(caster_oid).or_default().hate = max_hate + 1.0;
                }
                if let Some(ai) = world.objects.get_component_mut::<crate::model::npc::NpcAi>(&target_oid) {
                    ai.intention = crate::model::npc::NpcIntention::Attack;
                    ai.attack_timeout_tick = world.tick + crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
                }
            }
            // `AddHate.instant` — a flat hate change with no damage
            // (positive: Charm/Lure; negative: unused on this dist but
            // supported). Mirrors the add/reduce shape already used by
            // `minions.rs`/`faction_call`.
            SkillEffect::AddHate { power } => {
                let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&target_oid) else { continue };
                if *power >= 0.0 {
                    aggro.0.entry(caster_oid).or_default().hate += *power;
                } else if let Some(entry) = aggro.0.get_mut(&caster_oid) {
                    entry.hate = (entry.hate + *power).max(0.0);
                }
                if *power > 0.0
                    && let Some(ai) = world.objects.get_component_mut::<crate::model::npc::NpcAi>(&target_oid)
                        && ai.intention != crate::model::npc::NpcIntention::Attack {
                            ai.intention = crate::model::npc::NpcIntention::Attack;
                            ai.attack_timeout_tick = world.tick + crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
                        }
                // No `Attackable.reduceHate` tail here (the −25 calm window +
                // `clearAggroList`), deliberately: Java can't reach it through
                // this handler. `AddHate.instant` passes `(int) -val` for a
                // negative `val`, so a `power=-1240` skill calls
                // `reduceHate(effector, +1240)` → `ai.addHate(+1240)` — the
                // double negation makes Java's "negative AddHate" *raise* hate,
                // which never leaves `getMostHated() == null` and so never
                // arms the calm window. The only genuine `reduceHate` caller is
                // `TransferHate` (skill 489 Shift Target, off-chronicle here).
                // Porting the tail onto this branch's reduce semantics would
                // invent a 25 s stand-down Java never produces.
            }
            // `DeleteHate.instant` — chance-rolled: wipe the *whole* aggro
            // list and disengage (Java `setWalking()` + `setIntention(ACTIVE)`).
            SkillEffect::DeleteHate { chance } => {
                if world.roll(100) >= *chance {
                    continue;
                }
                if let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&target_oid) {
                    aggro.0.clear();
                }
                crate::game_loop::npc_ai::set_active(world, target_oid);
            }
            // `DeleteHateOfMe.instant` — chance-rolled: `stopHating` just the
            // caster's own entry, but Java disengages the AI wholesale
            // regardless of whatever other hate remains — the next think tick
            // re-picks the next-most-hated target on its own if any is left.
            SkillEffect::DeleteHateOfMe { chance } => {
                if world.roll(100) >= *chance {
                    continue;
                }
                if let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&target_oid)
                    && let Some(entry) = aggro.0.get_mut(&caster_oid) {
                        entry.hate = 0.0;
                    }
                crate::game_loop::npc_ai::set_active(world, target_oid);
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
            // `Cp.instant` — an immediate CP change, clamped so it never takes
            // the target past full CP (Java caps the *gain* at the recoverable
            // headroom; a negative amount is applied as-is and floored at 0).
            SkillEffect::Cp { amount, percent } => {
                let Some(pv) = world.objects.get_component::<crate::model::components::PlayerVitals>(&target_oid).copied()
                else {
                    continue; // NPCs have no CP pool
                };
                let basic = if *percent { pv.max_cp as f64 * *amount / 100.0 } else { *amount };
                let headroom = (pv.max_cp as f64 - pv.cur_cp).max(0.0);
                let delta = if basic >= 0.0 { basic.min(headroom) } else { basic };
                if delta != 0.0 {
                    if let Some(v) = world.objects.get_component_mut::<crate::model::components::PlayerVitals>(&target_oid) {
                        v.cur_cp = (v.cur_cp + delta).clamp(0.0, v.max_cp as f64);
                    }
                    broadcast_vitals(world, target_oid);
                }
            }
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
    let Some(race) = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .map(|p| crate::enums::Race::from_ordinal(p.race).unwrap_or(crate::enums::Race::Human))
    else {
        return;
    };
    let Some(pos) = position(world, player_oid) else {
        return;
    };
    let pick = if world.cfg.character.random_respawn_in_town {
        world.roll(64) as usize
    } else {
        0
    };
    if let Some((x, y, z)) = world
        .data
        .map_region
        .town_respawn(pos.x, pos.y, pos.z, race, pick)
    {
        crate::game_loop::death::teleport_player(world, player_oid, x, y, z);
    }
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
