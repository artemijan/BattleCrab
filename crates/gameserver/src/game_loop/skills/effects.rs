//! Effect application: instant damage/heal effects, continuous (buff)
//! effects, and buff expiry.

use crate::game_loop::helpers::client_for_player;
use crate::model::components::{
    BaseStats, Buffs, CombatStats, RegionCell, Speeds, StatModifiers, Vitals,
};
use crate::model::formulas;
use crate::model::skill::{
    ActiveBuff, BuffSlot, DispelSlot, RestorationGroup, Skill, SkillEffect, abnormal_type_client_id,
};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// The `callSkill` → `activateSkill` → effect-handler chain for the effect
/// kinds ported so far. Continuous stat modifiers land as an `ActiveBuff` on
/// the target; `MagicalAttack`/`Heal` are instant.
/// Java's `damage *= getValue(Stat.PHYSICAL_SKILL_POWER, 1)` /
/// `MAGICAL_SKILL_POWER` — the last multiplier a skill's damage passes
/// through. The physical one is applied by every `PhysicalAttack`-family
/// *effect handler* rather than by the shared formula, and the magical one
/// inside `calcMagicDam`; both land at the same point in the arithmetic, so
/// they share this reader (G34 S4).
fn skill_power_mul(world: &World, caster_oid: i32, magic: bool) -> f64 {
    use crate::model::stats::Stat;
    let stat = if magic {
        Stat::MagicalSkillPower
    } else {
        Stat::PhysicalSkillPower
    };
    world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&caster_oid)
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
        .get_component::<crate::model::components::StatModifiers>(&target_oid)
        .and_then(|m| m.skill_evasion.get(&skill.magic_type).copied())
        .unwrap_or(0.0);
    if chance <= 0.0 || (world.roll(100) as f64) >= chance {
        return false;
    }
    let (caster_name, target_name) = (
        creature_name(world, caster_oid),
        creature_name(world, target_oid),
    );
    if let Some(cid) = client_for_player(world, caster_oid)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(crate::network::server_packets::system_message_with(
            crate::network::server_packets::sm_ids::C1_DODGED_THE_ATTACK,
            &[crate::network::server_packets::SmParam::Text(target_name)],
        ));
    }
    if let Some(cid) = client_for_player(world, target_oid)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(crate::network::server_packets::system_message_with(
            crate::network::server_packets::sm_ids::YOU_HAVE_DODGED_C1_S_ATTACK,
            &[crate::network::server_packets::SmParam::Text(caster_name)],
        ));
    }
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
    let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&caster_oid)
    else {
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
        .get_component::<crate::model::components::StatModifiers>(&object_id)
        .map(|m| crate::model::finalize(m, stat, base))
        .unwrap_or(base)
}

/// The inverse of `servitor::servitor_of` — given a servitor, who owns it.
/// The owner link lives on the servitor as `ServitorOf`, which is also what
/// makes Java's `canStart` (`effected.isSummon()`) expressible: no component,
/// not a servitor, no unsummon.
fn servitor_owner_of(world: &World, servitor_oid: i32) -> Option<i32> {
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
            // `SummonNpc.instant`, narrowed to the `EffectPoint` branch — the
            // symbol totems (PLAN_G19_SYMBOLS.md). `Decoy` and the default
            // plain-spawn branch are TODO(G19) (no learnable carriers).
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
                let is_effect_point = world
                    .data
                    .npc_data
                    .get(*npc_id)
                    .is_some_and(|t| t.type_name == "EffectPoint");
                if !is_effect_point {
                    // Java's Decoy and default-spawn branches
                    // (`SummonNpc.java` `switch (npcTemplate.getType())`) are
                    // not ported. Not a deferral: the only `Decoy` carrier is
                    // skill 525, which appears in no skill tree, and every
                    // reachable `SummonNpc` on this dist is an `EffectPoint`
                    // symbol (454-460).
                    continue;
                }
                // GROUND skills spawn at the stored world position; everything
                // else at the effected creature (`SummonNpc.instant`).
                let fallback = world
                    .objects
                    .get_component::<crate::model::components::Position>(&target_oid)
                    .map(|p| (p.x, p.y, p.z))
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
                for _ in 0..(*npc_count).max(1) {
                    crate::game_loop::effect_point::spawn_effect_point(
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
            SkillEffect::MagicalAttack { power } => {
                let power = *power;
                let (m_atk, caster_name) = {
                    let m_atk =
                        world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                    (m_atk, caster_display_name(world, caster_oid))
                };
                let m_def = target_m_def(world, target_oid);
                let failure = roll_magic_failure(world, caster_oid, target_oid, skill, false);
                // `calcMagicDam`'s `attributeMod` term (Volcano's FIRE 20 vs
                // the target's fire resistance).
                let damage = formulas::calc_magic_dam(
                    m_atk,
                    m_def,
                    power,
                    mcrit,
                    crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, true),
                    magic_shots_bonus,
                    failure,
                ) * attribute_mod(world, caster_oid, target_oid, skill)
                    * skill_trait_mod(world, caster_oid, target_oid, skill, false)
                    // `calcMagicDam`'s own tail:
                    // `damage *= getValue(Stat.MAGICAL_SKILL_POWER, 1)`.
                    * skill_power_mul(world, caster_oid, true)
                    * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));
                apply_skill_damage(world, caster_oid, target_oid, damage, mcrit, true, &caster_name, skill.over_hit, false, skill.id);
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
            SkillEffect::MagicalAttackMp { power, critical, critical_limit } => {
                // `calcSuccess`: `isMpBlocked()` refuses outright.
                if crate::game_loop::abnormal::is_mp_blocked(world, target_oid) {
                    continue;
                }
                let m_atk = world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                let m_def = target_m_def(world, target_oid);
                // `calcMagicAffected`: `defence` is the target's mDef only for
                // an *active bad* skill — all four of these are.
                let defence = if skill.is_bad() { m_def } else { 0.0 };
                let gaussian = world.roll_gaussian();
                if !formulas::calc_magic_affected(m_atk, defence, gaussian) {
                    // Java messages both sides and bails.
                    if let Some(cid) = client_for_player(world, caster_oid)
                        && let Some(cs) = world.clients.get(&cid) {
                            cs.send(server_packets::system_message_with(sm_ids::YOUR_ATTACK_HAS_FAILED, &[]));
                        }
                    if let Some(cid) = client_for_player(world, target_oid)
                        && let Some(cs) = world.clients.get(&cid) {
                            cs.send(server_packets::system_message_with(
                                sm_ids::C1_RESISTED_C2_S_DRAIN,
                                &[
                                    SmParam::Text(caster_display_name(world, target_oid)),
                                    SmParam::Text(caster_display_name(world, caster_oid)),
                                ],
                            ));
                        }
                    continue;
                }

                // `calcShldUse` — a perfect block cuts the drain to 1.
                let (shield_def, shield_rate, con_bonus) = crate::game_loop::combat::shield_stats(world, target_oid);
                let (rate_roll, perfect_roll) = (world.roll(100), world.roll(100));
                let shield = formulas::calc_shield_use(shield_rate, con_bonus, false, false, rate_roll, perfect_roll);

                // Java: `mcrit = _critical && Formulas.calcCrit(skill.getMagicCriticalRate(), …)`.
                // All four skills are `<isMagic>1</isMagic>`, and `calcCrit`'s
                // magic branch **discards the rate it was passed** and reads
                // the caster's `MAGIC_CRITICAL_RATE` stat instead — so
                // `<magicCriticalRate>` is dead input here, and the roll is
                // exactly the per-cast `mcrit` already computed above (same
                // stat, same `min(rate, isBad ? 200 : 320) > Rnd.get(1000)`).
                // Only the effect's own `critical` flag gates it.
                let drain_crit = *critical && mcrit;
                let target_max_mp =
                    world.objects.get_component::<Vitals>(&target_oid).map(|v| v.max_mp as f64).unwrap_or(0.0);
                let failure = roll_magic_failure(world, caster_oid, target_oid, skill, false);
                let damage = if shield == formulas::SHIELD_PERFECT {
                    1.0
                } else {
                    formulas::calc_mana_dam(
                        m_atk,
                        m_def + if shield == formulas::SHIELD_SUCCEED { shield_def } else { 0.0 },
                        target_max_mp,
                        *power,
                        magic_shots_bonus,
                        failure,
                        drain_crit,
                        *critical_limit,
                    ) * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill))
                };

                // `mp = Math.min(effected.getCurrentMp(), damage)` — you cannot
                // drain more than is there, and the *reported* figure is the
                // clamped one.
                let drained = {
                    let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) else { continue };
                    let drained = v.cur_mp.min(damage.max(0.0));
                    if damage > 0.0 {
                        v.cur_mp -= drained;
                    }
                    drained
                };
                if drain_crit
                    && let Some(cid) = client_for_player(world, caster_oid)
                        && let Some(cs) = world.clients.get(&cid) {
                            cs.send(server_packets::system_message_with(sm_ids::M_CRITICAL, &[]));
                        }
                if let Some(cid) = client_for_player(world, target_oid)
                    && let Some(cs) = world.clients.get(&cid) {
                        cs.send(server_packets::system_message_with(
                            sm_ids::S2_S_MP_HAS_BEEN_DRAINED_BY_C1,
                            &[
                                SmParam::Text(caster_display_name(world, caster_oid)),
                                SmParam::Int(drained as i32),
                            ],
                        ));
                    }
                if let Some(cid) = client_for_player(world, caster_oid)
                    && let Some(cs) = world.clients.get(&cid) {
                        cs.send(server_packets::system_message_with(
                            sm_ids::YOUR_OPPONENT_S_MP_WAS_REDUCED_BY_S1,
                            &[SmParam::Int(drained as i32)],
                        ));
                    }
                broadcast_vitals(world, target_oid);
            }
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
                apply_skill_damage(world, caster_oid, target_oid, damage, crit, false, &caster_name, skill.over_hit, false, skill.id);
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
                let Some(called) = world.data.skill_data.get(*skill_id, *skill_level).cloned()
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
            // TODO(G34): re-check if any carrier ever uses a wider target type.
            SkillEffect::ImmobilePetBuff => {}
            // `CallParty.instant` — Chant of Gate (1429). Every *other* party
            // member is pulled to the caster, each gated by CallPc's shared
            // `checkSummonTargetStatus`. Note there is no `ConfirmDlg` here:
            // unlike Summon Friend, Java teleports them outright.
            SkillEffect::CallParty => {
                call_party(world, caster_oid);
            }
            SkillEffect::Blow { power, chance_boost, critical_chance, backstab } => {
                use crate::model::components::Position as PosComp;
                // Attacker position relative to the target's facing (for the
                // land roll's positional bonus, the blow's back/side damage
                // bonus, and Backstab's flank requirement).
                let (Some(a), Some(t)) = (
                    world.objects.get_component::<PosComp>(&caster_oid).copied(),
                    world.objects.get_component::<PosComp>(&target_oid).copied(),
                ) else {
                    continue;
                };
                let position = crate::model::movement::get_position(a.x, a.y, t.x, t.y, t.heading);

                // Backstab must land from outside the target's front arc
                // (`!isInFrontOf`). A front Backstab silently fails, like Java's
                // `calcSuccess == false` — no `doAttack`, no message.
                if *backstab && position == crate::model::movement::Position::Front {
                    continue;
                }

                let (p_atk, crit_rate, str_bonus, random_dmg, blow_rate_mod, caster_name) = {
                    let cs = world.objects.get_component::<CombatStats>(&caster_oid);
                    let p_atk = cs.map(|c| c.p_atk).unwrap_or(0.0);
                    let crit_rate = cs.map(|c| c.crit_hit).unwrap_or(0.0);
                    let random_dmg = cs.map(|c| c.random_dmg).unwrap_or(0);
                    let str_bonus = world
                        .objects
                        .get_component::<BaseStats>(&caster_oid)
                        .map(|b| world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Str, b.str_))
                        .unwrap_or(1.0);
                    // `Stat.BLOW_RATE` (`FatalBlowRate` — Focus Death, Critical
                    // Blow, Mortal Strike, Assassination), default 1.0.
                    let blow_rate_mod = world
                        .objects
                        .get_component::<StatModifiers>(&caster_oid)
                        .and_then(|m| m.mul.get(&crate::model::stats::Stat::BlowRate).copied())
                        .unwrap_or(1.0);
                    let name =
                        caster_display_name(world, caster_oid);
                    (p_atk, crit_rate, str_bonus, random_dmg, blow_rate_mod, name)
                };

                // `calcBlowSuccess`: does the blow land? A miss is silent
                // (Java's `calcSuccess == false` skips the whole effect).
                let landed = formulas::calc_blow_success(
                    crit_rate / 10.0,
                    position,
                    crate::game_loop::combat::crit_rate_position_mul(world, caster_oid, position),
                    a.z,
                    t.z,
                    *chance_boost,
                    blow_rate_mod,
                    world.cfg.character.blow_rate_chance_limit,
                    world.roll(100),
                );
                if !landed {
                    continue;
                }

                // `calcBlowDamage` opens on the shield switch: a normal block
                // adds the shield's sDef, a perfect one `return 1` outright.
                // Blows carry no `ignoreShieldDefence` — the parameter does not
                // exist on this formula, so the roll always happens.
                let defence = defence_after_shield(
                    world,
                    target_oid,
                    target_p_def(world, target_oid),
                    false,
                );
                let rand_roll = if random_dmg > 0 { world.roll(2 * random_dmg + 1) - random_dmg } else { 0 };
                let mut damage = match defence {
                    None => 1.0,
                    Some(defence) => {
                        let mut d = formulas::calc_blow_damage(
                            p_atk,
                            *power,
                            defence,
                            position,
                            formulas::random_damage_multiplier(rand_roll),
                            ss,
                        );
                        // `calcBlowDamage`'s `attributeMod` + trait terms.
                        d *= attribute_mod(world, caster_oid, target_oid, skill);
                        d *= skill_trait_mod(world, caster_oid, target_oid, skill, true);
                        d *= pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));
                        d
                    }
                };
                // FatalBlow/Backstab double on a `calcCrit` roll; SoulBlow
                // (`critical_chance == None`) doesn't. Java rolls this *after*
                // the perfect-block shortcut, but on that path the 1 is
                // returned before the crit is ever consulted — so the roll is
                // kept here (it stays in the RNG stream either way) and simply
                // has nothing to double.
                if let Some(cc) = critical_chance
                    && formulas::calc_physical_skill_crit(*cc, str_bonus, world.roll(100))
                    && defence.is_some() {
                        damage *= 2.0;
                    }
                // Java passes `critical = true` to `doAttack` for every blow, so
                // it always shows as a critical hit.
                apply_skill_damage(world, caster_oid, target_oid, damage, true, false, &caster_name, skill.over_hit, false, skill.id);
            }
            SkillEffect::Lethal { full_lethal, half_lethal } => {
                // `skill.getMagicLevel() < effected.getLevel() - 6`: silently
                // refused against a target too far above the skill's level.
                let target_level = creature_level(world, target_oid);
                if skill.magic_level < target_level - 6 {
                    continue;
                }
                // `isLethalable()`: raid bosses are immune — the same check
                // `apply_mute_interrupt` already uses — as is anything a script
                // exempted (`setLethalable(false)`: the siege Headquarters).
                // Grand-boss/door immunity isn't modeled, so it's not checked.
                let is_raid = world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&target_oid)
                    .and_then(|n| n.template(world))
                    .is_some_and(|t| t.is_raid());
                if is_raid
                    || world
                        .objects
                        .has_component::<crate::model::components::NotLethalable>(&target_oid)
                {
                    continue;
                }
                // `isHpBlocked()` (Celestial Shield, …): a landed `DamageBlock`
                // refuses this too, now that it's modeled.
                if crate::game_loop::abnormal::is_hp_blocked(world, target_oid) {
                    continue;
                }
                // `INSTANT_KILL_RESIST` is never set by anything in this
                // datapack (like `MAX_MOMENTUM`), so Java's resist roll would
                // always lose against a 0 stat — not rolled here at all.
                // None of the four outcome SystemMessages below take
                // parameters (`"Lethal Strike!"`, `"Half-Kill!"`, …).
                let caster_client = client_for_player(world, caster_oid);
                let is_player_target = world.objects.get_component::<crate::model::Player>(&target_oid).is_some();
                // `Lethal.instant`'s `chanceMultiplier` — **both** halves:
                // `calcAttributeBonus * calcGeneralTraitBonus(…, false)`. It
                // scales the full- and half-kill chances alike, so a victim
                // resisting the skill's element or trait is correspondingly
                // harder to execute.
                let lethal_amod = attribute_mod(world, caster_oid, target_oid, skill)
                    * calc_general_trait_bonus(
                        world,
                        caster_oid,
                        target_oid,
                        skill.trait_type,
                        false,
                    );
                if world.roll(100) < ((*full_lethal) * lethal_amod) as i32 {
                    if is_player_target {
                        if let Some(v) = world.objects.get_component_mut::<crate::model::components::PlayerVitals>(&target_oid) {
                            v.cur_cp = 1.0;
                        }
                        if let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                            v.cur_hp = 1.0;
                        }
                        if let Some(client_id) = client_for_player(world, target_oid)
                            && let Some(cs) = world.clients.get(&client_id) {
                                cs.send(server_packets::system_message_with(sm_ids::LETHAL_STRIKE, &[]));
                            }
                    } else if crate::game_loop::combat::is_npc_oid(target_oid)
                        && let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                            v.cur_hp = 1.0;
                        }
                    broadcast_vitals(world, target_oid);
                    if let Some(client_id) = caster_client
                        && let Some(cs) = world.clients.get(&client_id) {
                            cs.send(server_packets::system_message_with(sm_ids::HIT_WITH_LETHAL_STRIKE, &[]));
                        }
                } else if world.roll(100) < ((*half_lethal) * lethal_amod) as i32 {
                    if is_player_target {
                        if let Some(v) = world.objects.get_component_mut::<crate::model::components::PlayerVitals>(&target_oid) {
                            v.cur_cp = 1.0;
                        }
                        if let Some(client_id) = client_for_player(world, target_oid)
                            && let Some(cs) = world.clients.get(&client_id) {
                                cs.send(server_packets::system_message_with(sm_ids::HALF_KILL, &[]));
                                cs.send(server_packets::system_message_with(
                                    sm_ids::YOUR_CP_WAS_DRAINED_BECAUSE_YOU_WERE_HIT_WITH_A_HALF_KILL_SKILL,
                                    &[],
                                ));
                            }
                    } else if crate::game_loop::combat::is_npc_oid(target_oid)
                        && let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                            v.cur_hp *= 0.5;
                        }
                    broadcast_vitals(world, target_oid);
                    if let Some(client_id) = caster_client
                        && let Some(cs) = world.clients.get(&client_id) {
                            cs.send(server_packets::system_message_with(sm_ids::HALF_KILL, &[]));
                        }
                }
            }
            SkillEffect::HpDrain { power, percentage } => {
                let power = *power;
                let (m_atk, caster_name) = {
                    let m_atk =
                        world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                    (m_atk, caster_display_name(world, caster_oid))
                };
                let m_def = target_m_def(world, target_oid);
                // `is_drain` swaps the caster-side failure lines for the drain
                // wording (Java checks `skill.hasEffectType(HP_DRAIN)`).
                let failure = roll_magic_failure(world, caster_oid, target_oid, skill, true);
                let damage = formulas::calc_magic_dam(
                    m_atk,
                    m_def,
                    power,
                    mcrit,
                    crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, true),
                    magic_shots_bonus,
                    failure,
                ) * attribute_mod(world, caster_oid, target_oid, skill)
                    * skill_trait_mod(world, caster_oid, target_oid, skill, false)
                    // `MAGICAL_SKILL_POWER` lives *inside* Java's `calcMagicDam`,
                    // so every caller gets it — HpDrain included, even though
                    // its own handler never mentions the stat.
                    * skill_power_mul(world, caster_oid, true)
                    * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));

                // `HpDrain.instant()`: the drained HP is what's actually removed
                // — CP absorbs first (player targets only; NPCs have no CP),
                // then it's clamped to the target's remaining HP. Java reads both
                // as truncated ints, pre-damage.
                let cur_hp = world.objects.get_component::<Vitals>(&target_oid).map(|v| v.cur_hp.floor()).unwrap_or(0.0);
                let cur_cp = world
                    .objects
                    .get_component::<crate::model::components::PlayerVitals>(&target_oid)
                    .map(|v| v.cur_cp.floor())
                    .unwrap_or(0.0);
                let drain = if cur_cp > 0.0 {
                    if damage < cur_cp { 0.0 } else { damage - cur_cp }
                } else if damage > cur_hp {
                    cur_hp
                } else {
                    damage
                };
                // Heal the caster by `percentage`% of the drain, overheal-clamped.
                let heal = (*percentage / 100.0) * drain;
                if heal > 0.0 {
                    if let Some(v) = world.objects.get_component_mut::<Vitals>(&caster_oid) {
                        v.cur_hp = (v.cur_hp + heal).min(v.max_hp as f64);
                    }
                    if let Some(client_id) = client_for_player(world, caster_oid) {
                        let cur = world.objects.get_component::<Vitals>(&caster_oid).map(|v| v.cur_hp as i32).unwrap_or(0);
                        if let Some(cs) = world.clients.get(&client_id) {
                            cs.send(server_packets::status_update(
                                caster_oid,
                                &[(server_packets::status_update_type::CUR_HP, cur)],
                            ));
                        }
                        crate::game_loop::party::notify_party_vitals(world, caster_oid);
                    }
                }
                apply_skill_damage(world, caster_oid, target_oid, damage, mcrit, true, &caster_name, skill.over_hit, false, skill.id);
            }
            // `CpHealPercent.instant` — a share of the target's **max CP**,
            // clamped by `getMaxRecoverableCp()`. Java bails on a dead target,
            // a door and an HP-blocked one (the last is not a typo: the CP heal
            // reads `isHpBlocked`).
            // `OpenDoor.instant` — the lock-picking half of Unlock (27).
            SkillEffect::OpenDoor { chance, is_item } => {
                let Some(door_id) = world
                    .objects
                    .get_component::<crate::model::door::Door>(&target_oid)
                    .map(|d| d.door_id)
                else {
                    continue;
                };
                if crate::game_loop::helpers::instance_of(world, caster_oid)
                    != crate::game_loop::helpers::instance_of(world, target_oid)
                {
                    continue;
                }
                let openable_by_skill = world.data.door_data.get(door_id).is_some_and(|t| {
                    t.open_method == crate::data::door_data::DoorOpenMethod::BySkill
                });
                // Java also refuses when `door.getFort() != null`. This port
                // has no fort system, so that half cannot be evaluated — and
                // for the *skill* path it is vacuous on this dist anyway: none
                // of the 34 `BY_SKILL` doors is a fort door (they are Cruma,
                // Devil's Isle, the Water Garden, Rune ToH and the Four
                // Sepulchers). It is **not** vacuous for an item-cast unlock,
                // which skips the `BY_SKILL` gate entirely.
                // TODO(G34): add the fort gate once forts exist.
                if !openable_by_skill && !*is_item {
                    send_sm(world, caster_oid, sm_ids::THIS_DOOR_CANNOT_BE_UNLOCKED);
                    continue;
                }
                let already_open = world.geo.doors.is_open(door_id);
                if world.roll(100) < *chance && !already_open {
                    crate::game_loop::doors::open_door(world, target_oid);
                } else {
                    send_sm(world, caster_oid, sm_ids::YOU_HAVE_FAILED_TO_UNLOCK_THE_DOOR);
                }
            }
            // `OpenChest.instant` — the treasure-box half of Unlock (27), and a
            // *level* check rather than a chance roll: within 6 levels (5 above
            // 77) the box opens, otherwise it turns on you. Opening it kills the
            // chest with `setSpecialDrop()` + `setMustRewardExpSp(false)`, so it
            // rolls its own drop list and pays no exp.
            SkillEffect::OpenChest => {
                let is_chest = world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&target_oid)
                    .and_then(|n| world.data.npc_data.get(n.npc_id))
                    .is_some_and(|t| t.type_name == "Chest");
                let dead = world
                    .objects
                    .get_component::<Vitals>(&target_oid)
                    .is_some_and(|v| v.dead);
                if !is_chest
                    || dead
                    || crate::game_loop::helpers::instance_of(world, caster_oid)
                        != crate::game_loop::helpers::instance_of(world, target_oid)
                {
                    continue;
                }
                let player_level = creature_level(world, caster_oid);
                let chest_level = creature_level(world, target_oid);
                let band = if player_level <= 77 { 6 } else { 5 };
                if (chest_level - player_level).abs() <= band {
                    broadcast_social_action(world, caster_oid, 3);
                    if let Some(n) = world
                        .objects
                        .get_component_mut::<crate::model::npc::Npc>(&target_oid)
                    {
                        n.special_drop = true;
                        n.must_reward_exp_sp = false;
                    }
                    let max_hp = world
                        .objects
                        .get_component::<Vitals>(&target_oid)
                        .map(|v| v.max_hp as f64)
                        .unwrap_or(0.0);
                    crate::game_loop::combat::npc_receive_damage(
                        world,
                        target_oid,
                        caster_oid,
                        max_hp,
                        false,
                    );
                } else {
                    // Out of band the box is a mimic: Java gives it a single
                    // point of hate and points its AI at the caster.
                    broadcast_social_action(world, caster_oid, 13);
                    crate::game_loop::minions::add_hate(world, target_oid, caster_oid, 1.0);
                }
            }
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
                    // Java also excludes `isRaidMinion()`; this port has no
                    // separate minion type — a raid's minions carry ordinary
                    // `Monster` templates and are tracked by the leader's
                    // `MinionList` instead — so only the boss itself is immune.
                    // TODO(G34): extend to minions if a minion predicate lands.
                    .is_some_and(|t| t.is_raid());
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
                if let Some(region) = world
                    .objects
                    .get_component::<RegionCell>(&target_oid)
                    .map(|r| r.0)
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
            SkillEffect::Unsummon { chance } => {
                // `canStart`: the *effected* must be a summon. The port keys
                // ownership the other way (owner → `SummonRef`), so find the
                // owner by asking the target's own back-reference.
                let Some(owner) = servitor_owner_of(world, target_oid) else {
                    // Not a servitor — Java's `canStart` refuses outright.
                    continue;
                };
                // `calcSuccess`: a negative chance always lands; otherwise the
                // magic-level gate `(effected.getLevel() - 9) <= magicLevel`
                // has to pass first.
                if *chance >= 0 {
                    let target_level = creature_level(world, target_oid);
                    if skill.magic_level > 0 && (target_level - 9) > skill.magic_level {
                        continue;
                    }
                    let rate = *chance as f64
                        * attribute_mod(world, caster_oid, target_oid, skill)
                        * calc_general_trait_bonus(
                            world,
                            caster_oid,
                            target_oid,
                            skill.trait_type,
                            false,
                        );
                    if rate < 100.0 && rate <= world.roll(100) as f64 {
                        continue;
                    }
                }
                crate::game_loop::servitor::unsummon_servitor(world, owner);
            }
            // `DeathLink.instant` — Curse Death Link (1159). The power scales
            // with how close the **caster** is to death:
            // `power × (2 − 2·curHp/maxHp)` — ×2 at 0 HP, ×0 at full, so
            // casting it healthy does literally nothing.
            SkillEffect::DeathLink { power } => {
                let Some(v) = world.objects.get_component::<Vitals>(&caster_oid).copied() else {
                    continue;
                };
                if v.dead {
                    continue;
                }
                let scaled = *power * (-((v.cur_hp * 2.0) / v.max_hp as f64) + 2.0);
                let m_atk = world
                    .objects
                    .get_component::<CombatStats>(&caster_oid)
                    .map(|c| c.m_atk)
                    .unwrap_or(0.0);
                let m_def = target_m_def(world, target_oid);
                let caster_name = caster_display_name(world, caster_oid);
                let failure = roll_magic_failure(world, caster_oid, target_oid, skill, false);
                let damage = formulas::calc_magic_dam(
                    m_atk,
                    m_def,
                    scaled,
                    mcrit,
                    crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, true),
                    magic_shots_bonus,
                    failure,
                ) * attribute_mod(world, caster_oid, target_oid, skill)
                    * skill_trait_mod(world, caster_oid, target_oid, skill, false)
                    * skill_power_mul(world, caster_oid, true)
                    * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));
                apply_skill_damage(
                    world, caster_oid, target_oid, damage, mcrit, true, &caster_name,
                    skill.over_hit, false, skill.id,
                );
            }
            SkillEffect::CpHealPercent { power } => {
                use crate::model::components::PlayerVitals;
                if world
                    .objects
                    .get_component::<Vitals>(&target_oid)
                    .is_none_or(|v| v.dead)
                    || world
                        .objects
                        .has_component::<crate::model::door::Door>(&target_oid)
                    || crate::game_loop::abnormal::is_hp_blocked(world, target_oid)
                {
                    continue;
                }
                let Some(cp) = world
                    .objects
                    .get_component::<PlayerVitals>(&target_oid)
                    .copied()
                else {
                    // NPCs have no CP pool at all.
                    continue;
                };
                let max_cp = cp.max_cp as f64;
                let amount = if *power == 100.0 {
                    max_cp
                } else {
                    max_cp * *power / 100.0
                };
                let ceiling = max_recoverable(
                    world,
                    target_oid,
                    crate::model::stats::Stat::MaxRecoverableCp,
                    max_cp,
                );
                let amount = amount.min((ceiling - cp.cur_cp).max(0.0));
                if amount > 0.0 {
                    if let Some(v) = world
                        .objects
                        .get_component_mut::<PlayerVitals>(&target_oid)
                    {
                        v.cur_cp += amount;
                    }
                    broadcast_vitals(world, target_oid);
                }
            }
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
                if let Some(cid) = client_for_player(world, caster_oid)
                    && let Some(cs) = world.clients.get(&cid)
                {
                    cs.send(server_packets::system_message_with(
                        sm_ids::S1_HP_HAS_BEEN_RESTORED,
                        &[SmParam::Int(restored as i32)],
                    ));
                }
                broadcast_vitals(world, caster_oid);
            }
            SkillEffect::Heal { power } => {
                let power = *power;
                let m_atk = world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                let mut amount = formulas::calc_heal(power, m_atk, mcrit, sps, bss, skill.mp_consume, caster_is_player);
                // Java `Heal`: `amount *= effected.HEAL_EFFECT; amount +=
                // effected.HEAL_EFFECT_ADD` — the *recipient's* stats decide
                // how much of the heal they actually get.
                if let Some(mods) = world.objects.get_component::<crate::model::components::StatModifiers>(&target_oid) {
                    amount *= mods.mul.get(&crate::model::stats::Stat::HealEffect).copied().unwrap_or(1.0);
                    amount += mods.add.get(&crate::model::stats::Stat::HealEffectAdd).copied().unwrap_or(0.0);
                }
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
                // `Heal.java`: `min(amount, max(0, getMaxRecoverableHp() -
                // getCurrentHp()))` — the ceiling is the *recoverable* cap, not
                // the pool, which is what Noblesse Harmony/Symphony lower.
                let ceiling = {
                    let base = world
                        .objects
                        .get_component::<Vitals>(&target_oid)
                        .map(|v| v.max_hp as f64)
                        .unwrap_or(0.0);
                    max_recoverable(
                        world,
                        target_oid,
                        crate::model::stats::Stat::MaxRecoverableHp,
                        base,
                    )
                };
                let healed = {
                    let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else { continue };
                    let amount = amount.min((ceiling - vitals.cur_hp).max(0.0));
                    vitals.cur_hp += amount;
                    amount
                };
                let caster_name = caster_display_name(world, caster_oid);
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
            SkillEffect::HealPercent { power } => {
                let power = *power;
                let Some(max_hp) = world.objects.get_component::<Vitals>(&target_oid).map(|v| v.max_hp as f64) else {
                    continue;
                };
                // Java `full = power == 100.0`, else `maxHp * power / 100`. No
                // `HealEffect`/`HealEffectAdd` recipient scaling (unlike `Heal`).
                let amount = if power == 100.0 { max_hp } else { max_hp * power / 100.0 };
                if amount < 0.0 {
                    // A negative-power instance (none learnable today) is
                    // damage, not healing — Java's `reduceCurrentHp` +
                    // `sendDamageMessage`, reusing the shared damage path.
                    let caster_name = caster_display_name(world, caster_oid);
                    apply_skill_damage(world, caster_oid, target_oid, -amount, false, skill.magic_type == 1, &caster_name, false, false, skill.id);
                    continue;
                }
                // `isHpBlocked()`: a landed `DamageBlock` refuses a positive
                // heal too (the damage branch above already gets this for
                // free through `apply_skill_damage`).
                if crate::game_loop::abnormal::is_hp_blocked(world, target_oid) {
                    continue;
                }
                if crate::game_loop::combat::is_npc_oid(target_oid) {
                    let hp = {
                        let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else { continue };
                        if vitals.dead {
                            continue;
                        }
                        vitals.cur_hp = (vitals.cur_hp + amount).min(vitals.max_hp as f64);
                        (vitals.cur_hp as i32, vitals.max_hp)
                    };
                    if let Some(region) = world.objects.get_component::<RegionCell>(&target_oid).map(|r| r.0) {
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
                    let amount = amount.min((vitals.max_hp as f64 - vitals.cur_hp).max(0.0));
                    vitals.cur_hp += amount;
                    amount
                };
                let caster_name = caster_display_name(world, caster_oid);
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
                        let cur_hp = world.objects.get_component::<Vitals>(&target_oid).map(|v| v.cur_hp as i32).unwrap_or(0);
                        cs.send(server_packets::status_update(target_oid, &[(server_packets::status_update_type::CUR_HP, cur_hp)]));
                    }
                    crate::game_loop::party::notify_party_vitals(world, target_oid);
                }
            }
            SkillEffect::FocusMomentum { amount, max_charges } => {
                // Java's own hardcoded fallback for the never-set-in-this-
                // datapack `MAX_MOMENTUM` stat — see the type's doc comment.
                let max = (*max_charges).min(8);
                let current = world.objects.get_component::<crate::model::Player>(&target_oid).map(|p| p.charges).unwrap_or(0);
                let Some(client_id) = client_for_player(world, target_oid) else { continue };
                if current >= max {
                    if let Some(cs) = world.clients.get(&client_id) {
                        cs.send(server_packets::system_message_with(sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY, &[]));
                    }
                    continue;
                }
                let new_charge = (current + *amount).min(max);
                if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&target_oid) {
                    p.charges = new_charge;
                }
                if let Some(cs) = world.clients.get(&client_id) {
                    if new_charge == max {
                        cs.send(server_packets::system_message_with(sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY, &[]));
                    } else {
                        cs.send(server_packets::system_message_with(
                            sm_ids::YOUR_FORCE_HAS_INCREASED_TO_LEVEL_S1,
                            &[SmParam::Int(new_charge)],
                        ));
                    }
                }
                crate::game_loop::helpers::send_etc_status_update(world, client_id, target_oid);
            }
            SkillEffect::EnergyAttack { power, critical_chance, p_def_mod, charge_consume, ignore_shield_defence } => {
                // `charge = min(chargeConsume, player.charges)` — pre-clamped,
                // so Java's `decreaseCharges` (which only fails when asked to
                // remove more than the player has) never actually refuses here.
                let charge = {
                    let cur = world.objects.get_component::<crate::model::Player>(&caster_oid).map(|p| p.charges).unwrap_or(0);
                    (*charge_consume).min(cur)
                };
                if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&caster_oid) {
                    p.charges -= charge;
                }
                if let Some(client_id) = client_for_player(world, caster_oid) {
                    crate::game_loop::helpers::send_etc_status_update(world, client_id, caster_oid);
                }
                let (p_atk, level, str_bonus, caster_name) = {
                    let p_atk = world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.p_atk).unwrap_or(0.0);
                    let str_bonus = world
                        .objects
                        .get_component::<BaseStats>(&caster_oid)
                        .map(|b| world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Str, b.str_))
                        .unwrap_or(1.0);
                    (p_atk, caster_level(world, caster_oid), str_bonus, caster_display_name(world, caster_oid))
                };
                let base_defence = target_p_def(world, target_oid) * *p_def_mod;
                let defence = defence_after_shield(world, target_oid, base_defence, *ignore_shield_defence);
                let crit = formulas::calc_physical_skill_crit(*critical_chance, str_bonus, world.roll(100));
                // `energyChargesBoost = 1 + (charge * 0.1)` — 10% bonus damage
                // per charge spent, the whole point of building Force first.
                let energy_charges_boost = 1.0 + charge as f64 * 0.1;
                let damage = match defence {
                    None => 1.0,
                    Some(defence) => {
                        formulas::calc_physical_skill_damage(
                            p_atk,
                            1.0, // no separate pAtkMod term in Java's EnergyAttack formula
                            defence,
                            1.0, // already folded into `defence` above
                            *power,
                            formulas::level_mod(level),
                            1.0, // no random-damage term in Java's EnergyAttack formula
                            crit,
                            crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, false),
                            ss,
                            // Java's EnergyAttack has no ranged branch at all —
                            // its `weaponMod` is a flat 77.
                            false,
                        ) * energy_charges_boost
                            // `EnergyAttack.instant`'s `attributeMod` + trait terms.
                            * attribute_mod(world, caster_oid, target_oid, skill)
                            * skill_trait_mod(world, caster_oid, target_oid, skill, true)
                            * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill))
                    }
                };
                apply_skill_damage(world, caster_oid, target_oid, damage, crit, false, &caster_name, skill.over_hit, false, skill.id);
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
                if let Some(race) = world
                    .objects
                    .get_component::<crate::model::Player>(&target_oid)
                    .map(|p| crate::enums::Race::from_ordinal(p.race).unwrap_or(crate::enums::Race::Human))
                {
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
                        if let Some((x, y, z)) = world.data.map_region.town_respawn(pos.x, pos.y, pos.z, race, pick) {
                            crate::game_loop::death::teleport_player(world, target_oid, x, y, z);
                        }
                    }
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
            SkillEffect::Hp { amount, percent } => {
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
                    continue;
                };
                let is_raid = world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&target_oid)
                    .and_then(|n| world.data.npc_data.get(n.npc_id))
                    .is_some_and(|t| t.is_raid());
                if v.dead
                    || is_raid
                    || world
                        .objects
                        .has_component::<crate::model::door::Door>(&target_oid)
                    || crate::game_loop::abnormal::is_hp_blocked(world, target_oid)
                {
                    continue;
                }
                let basic = if *percent {
                    v.max_hp as f64 * *amount / 100.0
                } else {
                    *amount
                };
                let ceiling = max_recoverable(
                    world,
                    target_oid,
                    crate::model::stats::Stat::MaxRecoverableHp,
                    v.max_hp as f64,
                );
                let gain = basic.min((ceiling - v.cur_hp).max(0.0));
                if gain > 0.0 {
                    if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                        vit.cur_hp = (vit.cur_hp + gain).min(vit.max_hp as f64);
                    }
                    broadcast_vitals(world, target_oid);
                }
            }
            SkillEffect::CallPc => {
                call_pc(world, caster_oid, target_oid, skill);
            }
            SkillEffect::GiveRecommendation { amount } => {
                crate::game_loop::reco::apply_give_recommendation(world, caster_oid, target_oid, *amount);
            }
            SkillEffect::CreateHeadquarter => {
                // `HeadquarterCreate.instant`: the effector (an attacker clan
                // leader) plants the HQ flag. All the siege/leader/attacker/
                // flag-cap checks live in the engine (mirrors the effect body +
                // `BuildCampSkillCondition`).
                crate::game_loop::siege::place_siege_flag(world, caster_oid);
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
                    } else if let Some(cid) = crate::game_loop::helpers::client_for_player(world, caster_oid) {
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
            SkillEffect::DispelBySlot { dispel } => {
                // Java `DispelBySlot.instant`: stop each active effect whose
                // originating skill's `<abnormalType>` is in the dispel set and
                // whose `abnormalLevel` is at or below the listed level (a
                // negative level dispels every level). We look each active buff's
                // source skill back up in `skill_data` for its type/level, then
                // route removals through `handle_buff_expire` — which drops the
                // buff, reverts its stats, and rebroadcasts the abnormal icons
                // for both player and NPC targets; the DoT tick chain (e.g.
                // Poison) self-terminates once its buff is gone. Buff snapshot is
                // collected first to avoid overlapping borrows of `world`.
                let candidates: Vec<(i32, i32)> = world
                    .objects
                    .get_component::<Buffs>(&target_oid)
                    .map(|buffs| buffs.0.iter().map(|b| (b.skill_id, b.skill_level)).collect())
                    .unwrap_or_default();
                let to_dispel: Vec<i32> = candidates
                    .into_iter()
                    .filter(|&(sid, slvl)| {
                        world.data.skill_data.get(sid, slvl).is_some_and(|bs| {
                            dispel
                                .iter()
                                .any(|(ty, lvl)| bs.abnormal_type == *ty && (*lvl < 0 || *lvl >= bs.abnormal_level))
                        })
                    })
                    .map(|(sid, _)| sid)
                    .collect();
                for skill_id in to_dispel {
                    handle_buff_expire(world, target_oid, skill_id);
                }
                // Java `DispelBySlot.instant` also dispels a *non-buff*
                // transformation ("Dispel transformations (buff and by GM)"):
                // a TRANSFORM entry matching the current transform id (or the
                // catch-all negative level, e.g. Dismount 839's `TRANSFORM,-1`)
                // calls `stopTransformation(true)`. That's the only revert path
                // for `//transform`/`//ride_bike`, which set the transform
                // directly with no backing buff. Buff-backed transforms are
                // already reverted by the `handle_buff_expire` sweep above, so
                // only act if still transformed. (Java guards the whole method
                // with `hasAbnormalType(...)`, which would make this branch
                // unreachable for GM transforms — an upstream quirk we don't
                // reproduce; the dist skill data's intent is that Dismount
                // always ends the ride.)
                let transform_id = world
                    .objects
                    .get_component::<crate::model::Player>(&target_oid)
                    .map_or(0, |p| p.transform_id);
                if transform_id != 0
                    && dispel
                        .iter()
                        .any(|(ty, lvl)| ty == "TRANSFORM" && (*lvl < 0 || *lvl == transform_id))
                {
                    crate::game_loop::admin::transforms::remove_transform(world, target_oid);
                }
            }
            SkillEffect::DispelBySlotProbability { dispel, rate } => {
                // Java `DispelBySlotProbability.instant`: the same cleanse as
                // `DispelBySlot`, except the `rate`% roll is evaluated **per
                // buff** inside the predicate — so a 40% Mass Warrior Bane
                // strips roughly two of five matching buffs rather than all or
                // nothing. The spec carries no per-type level, so every level
                // of a listed abnormal type is a candidate.
                //
                // Java also skips `isIrreplacableBuff()` effects. Not modelled
                // and not a gap: the tag appears only in the 22800+/23200+/
                // 27800+ skill files, all off-chronicle for Interlude.
                //
                // Note this path deliberately does *not* consult the target's
                // `ResistDispelBuff`: Java reads that stat only in
                // `Formulas.calcCancelSuccess` (the `Cancel` skill family,
                // unported), never in the Bane handler.
                let candidates: Vec<(i32, i32)> = world
                    .objects
                    .get_component::<Buffs>(&target_oid)
                    .map(|buffs| buffs.0.iter().map(|b| (b.skill_id, b.skill_level)).collect())
                    .unwrap_or_default();
                let mut to_dispel: Vec<i32> = Vec::new();
                for (sid, slvl) in candidates {
                    let matches = world
                        .data
                        .skill_data
                        .get(sid, slvl)
                        .is_some_and(|bs| dispel.contains(&bs.abnormal_type));
                    // Roll per candidate, and only for candidates that match —
                    // keeping the roll count (and so the RNG stream) tied to the
                    // buffs actually at risk, as in Java's predicate.
                    if matches && world.roll(100) < *rate {
                        to_dispel.push(sid);
                    }
                }
                for skill_id in to_dispel {
                    handle_buff_expire(world, target_oid, skill_id);
                }
            }
            // `DispelByCategory.instant` — the "Cancel" family (Cancellation,
            // Cleanse, Purification Field, Touch of Death): unlike
            // `DispelBySlot`/`DispelBySlotProbability` (a fixed abnormal-type
            // list) this steals *whatever* is up. `BUFF` walks dances then
            // buffs in reverse cast order (Java's `getDances()`/`getBuffs()`
            // reversed); `DEBUFF` walks debuffs. Both stop once `max` buffs
            // are collected. `ALL` is dead in Java too (no shipped skill uses
            // it) and is a no-op here.
            SkillEffect::DispelByCategory { slot, rate, max } => {
                if world.objects.get_component::<Vitals>(&target_oid).is_some_and(|v| v.dead) {
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
            SkillEffect::StatModifier(_) => {} // collected below
            // Blessing of Protection: no instant action — it lands purely as
            // the timed `PK_PROTECT` abnormal handled by the buff path below
            // (kept off the empty-`buff_effects` bail via `has_protection`).
            // TODO(G-pvp): the actual PK damage immunity.
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
            SkillEffect::TargetCancel { chance } => {
                // `calcSuccess`: an invincible target is never shaken off its
                // mark. Java names the three abnormal types directly rather
                // than going through an effect flag, so this reads the live
                // buffs' `abnormalType` the same way.
                const INVINCIBLE: [&str; 3] = [
                    "ABNORMAL_INVINCIBILITY",
                    "INVINCIBILITY_SPECIAL",
                    "INVINCIBILITY",
                ];
                if world
                    .objects
                    .get_component::<Buffs>(&target_oid)
                    .is_some_and(|b| {
                        b.0.iter()
                            .any(|x| INVINCIBLE.contains(&x.abnormal_type.as_str()))
                    })
                {
                    continue;
                }
                // Java gates this on `Formulas.calcProbability`, not on the raw
                // percentage — so the victim's **level** counts, and Shield
                // Bash slides off a target well above the skill's magic level.
                if !confuse_chance_passes(world, caster_oid, target_oid, skill, *chance) {
                    continue;
                }
                // `setTarget(null)` — the Player override broadcasts
                // `TargetUnselected` with includeSelf, which is what clears the
                // client's selection ring.
                if let Some(client_id) = client_for_player(world, target_oid) {
                    crate::game_loop::target::set_target(world, client_id, target_oid, None);
                } else if let Some(t) = world.objects.get_component_mut::<crate::model::components::TargetRef>(&target_oid) {
                    t.0 = None; // NPC: no client to notify
                }
                // `abortAttack()` / `abortCast()`.
                world.objects.remove_component::<crate::model::components::Intent>(&target_oid);
                if world.objects.has_component::<crate::model::components::Casting>(&target_oid) {
                    crate::game_loop::skills::cast::stop_casting(world, target_oid);
                }
            }
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
                    .get_component::<crate::model::components::StatModifiers>(&target_oid)
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
            // damage path; `AttackTrait`'s accumulator has no consumer yet
            // (TODO(G20) on its doc comment).
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
/// an inline `// TODO: M.Crit can occur even if this skill is resisted` at that
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
            let caster_name = world
                .objects
                .get_component::<crate::model::Player>(&caster_oid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            apply_skill_damage(
                world,
                caster_oid,
                target_oid,
                damage,
                true,
                true,
                &caster_name,
                skill.over_hit,
                false,
                skill.id,
            );
        }
    }
}

/// The continuous half of Java `Skill.applyEffects` — everything that turns a
/// cast into one timed `ActiveBuff` on the target — split out from the instant
/// (damage/heal) half above so it can be driven on its own.
///
/// `abnormal_time_override` is Java's `abnormalTime` parameter: `None` uses the
/// skill's own `abnormalTime`, `Some(secs)` overrides it. Buff restore at login
/// is the caller that passes it, mirroring Java `restoreEffects`'
/// `skill.applyEffects(this, this, false, remainingTime)` — the `instant =
/// false` there is exactly why this half has to be separable, since a restored
/// buff must not re-fire the skill's damage or heal.
pub(crate) fn apply_continuous_effects(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    abnormal_time_override: Option<i32>,
) -> bool {
    // Continuous effects → one ActiveBuff on the target (`applyEffects`).
    let buff_effects = skill.stat_modifier_effects();
    // A `DamOverTime` (poison/bleed) debuff has no stat modifier but still
    // lands as a timed buff (for the icon + expiry) whose ticks are armed
    // below — so it must not bail here on an empty `buff_effects`.
    // Any effect whose whole job happens on the periodic tick chain: it carries
    // no stat modifier, but the buff must still land (for the icon, the expiry
    // and — crucially — to keep the tick chain alive, which stops the moment
    // the buff is gone).
    let has_periodic = skill.effects.iter().any(|e| {
        matches!(
            e,
            SkillEffect::DamOverTime { .. }
                | SkillEffect::HealOverTime { .. }
                | SkillEffect::ManaDamOverTime { .. }
                | SkillEffect::ManaHealOverTime { .. }
                | SkillEffect::MpConsumePerLevel { .. }
                | SkillEffect::Relax { .. }
                | SkillEffect::ChameleonRest { .. }
                | SkillEffect::Fear { .. }
                | SkillEffect::FakeDeath { .. }
        )
    });
    // Blessing of Protection, DefenceTrait (Mental Shield / Resist Shock) and
    // VampiricAttack (Vampiric Rage) likewise carry no stat modifier but must
    // still land as an icon-only timed buff (their abnormal + duration): their
    // real mechanics aren't modeled yet, but the buff must show and expire.
    // Stun/sleep/paralyze/root carry no stat modifier either — their whole
    // mechanic is the abnormal flag — so they must survive this guard too.
    // State-only effects carry no stat modifier: the CC flags, and
    // `BlockAbnormalSlot`'s blocked-type set. Both must survive the
    // empty-effects guard or the buff is dropped whole and never lands.
    let has_state_flag = skill.effect_flags() != 0 || !skill.blocked_abnormals().is_empty();
    // `Transformation` also carries no stat modifier of its own (the transform
    // template's stat/speed overrides apply separately) but must still land as
    // a timed `TRANSFORM` buff — that buff's expiry is what drives the revert.
    let has_iconless_buff = skill.effects.iter().any(|e| {
        matches!(
            e,
            SkillEffect::ProtectionBlessing
                | SkillEffect::DefenceTrait { .. }
                | SkillEffect::VampiricAttack { .. }
                | SkillEffect::MagicMpCost { .. }
                | SkillEffect::Reuse { .. }
                | SkillEffect::DamageShield { .. }
                | SkillEffect::Transform { .. }
                | SkillEffect::AttackTrait { .. }
                // `TargetMe` carries no stat modifier and stamps no
                // `effect_flag` — its whole mechanic is the `LockedTarget`
                // component, which `handle_buff_expire` clears. Without this
                // the buff is dropped by the guard, the expiry hook never
                // runs, and the taunt lock becomes **permanent**. Fifth slice
                // caught by this guard; any new modifier-less effect must join
                // one of its three categories.
                | SkillEffect::TargetMe
                // `SkillEvasion` likewise: its contribution lives in a
                // per-magicType map that only `handle_buff_expire` unmerges,
                // so a dropped buff makes the dodge chance permanent.
                | SkillEffect::SkillEvasion { .. }
            // `Lucky` is an empty effect in Java too — `Player.isLucky()` asks
            // whether the buff is *present*, so landing is the whole job.
            | SkillEffect::Lucky
            // Its grant is written *after* the buff lands (by `night_stats`),
            // so at guard time it looks modifier-less. Tenth slice caught here.
            | SkillEffect::NightStatModify { .. }
            // The two listener-shaped triggers: Java attaches their listener to
            // the **buff**, and this port finds them by scanning the bearer's
            // buff list, so a dropped buff means the trigger never fires at
            // all. Seventh and eighth slices caught by this guard.
            | SkillEffect::TriggerSkillByDamage { .. }
            | SkillEffect::TriggerSkillByMagicType { .. }
        )
    });
    if buff_effects.is_empty() && !has_periodic && !has_iconless_buff && !has_state_flag {
        return false;
    }

    // Debuff landing roll — Java `Formulas.calcEffectSuccess`. A bad skill with
    // an `activateRate` (≠ -1) can be resisted: compute the chance, roll it, and
    // report the outcome to the caster with the computed chance baked in — a
    // "landed with X% chance on <target>" line on success, or a
    // "<target> has resisted <skill>: X%" line on a failed roll (which also skips
    // the buff and its DoT ticks). Self-targeted casts never resist (Java's
    // `target != attacker`). Buffs and always-land debuffs (`-1`) fall straight
    // through. `activateRate == -1` is filtered here so those consume no roll
    // (keeps the ordering of the remaining rolls stable). Both lines are
    // single-target only so an AoE debuff doesn't spam one line per target.
    // `calcEffectSuccess`'s first resist clause, ahead of the roll: a target
    // that is **casting** a skill whose `<abnormalResists>` names this skill's
    // `abnormalType` shrugs it off outright. That is what makes the long-ritual
    // skills uninterruptible — 176 skills declare a list, 146 of them the full
    // crowd-control set.
    if skill.is_debuff
        && caster_oid != target_oid
        && casting_resists_abnormal(world, target_oid, &skill.abnormal_type)
    {
        return false;
    }
    if skill.is_bad() && caster_oid != target_oid && skill.activate_rate != -1 {
        let target_level = creature_level(world, target_oid);
        // Java: `skill.isDebuff() ? target.getStat().getValue(RESIST_ABNORMAL_DEBUFF, 1) : 1`.
        let debuff_resist_mod = if skill.is_debuff {
            world
                .objects
                .get_component::<crate::model::components::StatModifiers>(&target_oid)
                .and_then(|m| {
                    m.mul
                        .get(&crate::model::stats::Stat::ResistAbnormalDebuff)
                        .copied()
                })
                .unwrap_or(1.0)
        } else {
            1.0
        };
        let rate = formulas::calc_effect_land_rate(
            skill.magic_level,
            skill.activate_rate,
            skill.lvl_bonus_rate,
            target_level,
            debuff_resist_mod,
            // `calcEffectSuccess`'s `elementMod` — an elemental debuff lands
            // more easily on a target weak to its element.
            attribute_mod(world, caster_oid, target_oid, skill),
            calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, false),
            // The two `<basicProperty>` terms — a stat subtracted inside
            // `baseMod`, and the mesmerizing-debuff chain multiplied after the
            // clamp (G34 S2, `game_loop::basic_property`).
            crate::game_loop::basic_property::abnormal_resist(
                world,
                target_oid,
                skill.basic_property,
            ),
            crate::game_loop::basic_property::resist_bonus(world, target_oid, skill.basic_property),
        );
        // Java: resisted when `finalRate <= Rnd.get(100)` (0-99). Roll before the
        // message so the outcome line reflects it and the roll order stays stable.
        let resisted = rate <= world.roll(100) as f64;
        if skill.affect_scope == crate::model::skill::AffectScope::Single {
            // Two of this server's own messages (ids 9000/9001), so the client
            // renders and colours them like any other rather than receiving a
            // sentence we formatted. They only display once the client table
            // has been rebuilt — `l2r-tools client-dat sync-messages`.
            use commons::system_messages::SmValue;
            use commons::system_messages::generated::{
                C1_HAS_RESISTED_S2_CHANCE_WAS_S3, S1_LANDED_ON_C2_CHANCE_WAS_S3,
            };
            let target_name = creature_name(world, target_oid);
            let spell = SmValue::Skill {
                id: skill.id,
                level: skill.level,
            };
            let chance = rate as i32;
            let message = if resisted {
                C1_HAS_RESISTED_S2_CHANCE_WAS_S3::new(target_name, spell, chance)
            } else {
                S1_LANDED_ON_C2_CHANCE_WAS_S3::new(spell, target_name, chance)
            };
            if let Some(client_id) = client_for_player(world, caster_oid)
                && let Some(cs) = world.clients.get(&client_id)
            {
                cs.send(server_packets::system_message(&message));
            }
        }
        if resisted {
            return false;
        }
    }
    // Java `EffectList` only schedules a stop task when the effect's time is
    // positive; a toggle or a 0-`abnormalTime` buff (e.g. Super Haste 7029,
    // `operateType=T`) persists until it's toggled/removed. Model that as a
    // sentinel expiry with no `BuffExpire` schedule, else it would vanish the
    // same tick it lands.
    // `Formulas.calcMagicAffected`: a target under `DEBUFF_BLOCK` (Mystic
    // Immunity, Celestial Shield) refuses every incoming debuff outright — no
    // roll, no partial landing. Self-cast is exempt for the same reason the
    // resist roll is: Java compares `target != attacker`.
    if skill.is_debuff
        && caster_oid != target_oid
        && crate::game_loop::abnormal::is_debuff_blocked(world, target_oid)
    {
        return false;
    }
    // The mirror image, from `EffectList.add`:
    // `if (info.getEffected().isBuffBlocked() && !skill.isBad()) return;`.
    // Note it keys on `isBad()` (effectPoint < 0) rather than `isDebuff()`, and
    // has **no self-cast exemption** — Dance of Medusa stops the victim
    // buffing themselves too, which is the point of it (G34 S3).
    if !skill.is_bad() && crate::game_loop::abnormal::is_buff_blocked(world, target_oid) {
        return false;
    }

    // `EffectList.addActive`'s blocked-slot gate: a buff whose abnormal type is
    // in the target's blocked set (from a live `BlockAbnormalSlot`) can't land
    // at all. This is what keeps two Prophecies off the same character.
    // "NONE" is the no-abnormal sentinel and is never blockable.
    if skill.abnormal_type != "NONE" {
        let blocked = world
            .objects
            .get_component::<Buffs>(&target_oid)
            .is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.blocked_abnormals.contains(&skill.abnormal_type))
            });
        if blocked {
            return false;
        }
    }

    // Java `BuffInfo.setAbnormalTime` is applied only for a *positive* override
    // ("if equal or lesser than zero will be ignored"), so a bad stored value
    // falls back to the skill's own duration rather than making the buff permanent.
    let abnormal_time = abnormal_time_override
        .filter(|&t| t > 0)
        .unwrap_or(skill.abnormal_time);
    let permanent = abnormal_time <= 0;
    let expires_at_tick = if permanent {
        u64::MAX
    } else {
        world.tick + abnormal_time as u64 * 10
    };
    let buff = ActiveBuff {
        skill_id: skill.id,
        skill_level: skill.level,
        abnormal_type_client_id: abnormal_type_client_id(&skill.abnormal_type),
        abnormal_type: skill.abnormal_type.clone(),
        abnormal_level: skill.abnormal_level,
        slot: skill.buff_slot(),
        expires_at_tick,
        passive: false,
        effect_flags: skill.effect_flags(),
        blocked_abnormals: skill.blocked_abnormals(),
        abnormal_visuals: skill.abnormal_visuals.clone(),
        effects: buff_effects,
    };

    // Java `Skill.applyEffects`, inside the `if (addContinuousEffects)` branch
    // and immediately after `EffectList.add(info)`: "Check for mesmerizing
    // debuffs and increase resist level." Position matters — it is on the
    // *landed* path, past the resist roll, so a debuff that keeps failing never
    // builds the resistance that would lock it out (G34 S2).
    //
    // `addContinuousEffects` is `isToggle() || (isContinuous() && …)`, so an
    // instant-only debuff does not accrue; `increase_resist_level` filters the
    // `NONE` property and the can't-accrue targets (every player on this dist).
    if skill.is_debuff
        && (skill.is_continuous || skill.operate_type == crate::model::skill::OperateType::Toggle)
    {
        crate::game_loop::basic_property::increase_resist_level(
            world,
            target_oid,
            skill.basic_property,
        );
    }

    // Arm the poison/bleed damage-over-time ticks (Java `BuffInfo.
    // scheduleEffects` → `scheduleAtFixedRate`). The recurring `DamOverTimeTick`
    // self-terminates once this buff's `BuffExpire` removes it or the target
    // dies; done here so it covers both NPC and player targets.
    schedule_dam_over_time(world, caster_oid, target_oid, skill);

    // `DefenceTrait.onStart` — merge the buff's per-trait resistances. Done
    // here, above the NPC/player split, because a resisted mob is as real as a
    // resisted player.
    for effect in &skill.effects {
        match effect {
            SkillEffect::DefenceTrait { traits } => merge_defence_traits(world, target_oid, traits),
            // `AttackTrait.onStart` — the attacker-side twin. Note it merges
            // onto the **effected**, which for these self-buffs is the caster.
            SkillEffect::AttackTrait { traits } => merge_attack_traits(world, target_oid, traits),
            _ => {}
        }
    }
    // `MagicMpCost.onStart` / `Reuse.onStart` — same place, same reasoning.
    merge_skill_rates(world, target_oid, skill);

    // NPC target: buffs modify the mob's server-side stats (no buff icons —
    // those are self-only — and no NpcInfo re-broadcast, so a speed change
    // isn't reflected client-side until respawn; the combat math uses it now).
    if crate::game_loop::combat::is_npc_oid(target_oid) {
        apply_buff_to_npc(world, target_oid, buff, skill.id);
        if skill.effect_flags() & crate::model::skill::effect_flag::BLOCK_ACTIONS != 0 {
            apply_block_actions_interrupt(world, target_oid);
        }
        apply_mute_interrupt(world, target_oid, skill);
        if !permanent {
            world.scheduler.schedule(
                expires_at_tick,
                ScheduledTask::BuffExpire {
                    player_object_id: target_oid,
                    skill_id: skill.id,
                },
            );
        }
        // The NPC branch is the *success* tail, not a guard: the buff was
        // applied, so an `onStart` side effect keyed on landing (the DoT
        // magic-crit burst) is due.
        return true;
    }
    {
        let landed =
            if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) =
                world.objects.get_many_mut::<(
                    &mut crate::model::Player,
                    &BaseStats,
                    &mut StatModifiers,
                    &crate::model::inventory::Inventory,
                    &mut Buffs,
                    &mut Speeds,
                    &mut CombatStats,
                )>(&target_oid)
            {
                target.apply_buff(
                    &world.data,
                    base,
                    &mut mods,
                    inventory,
                    &mut buffs,
                    &mut speeds,
                    &mut combat,
                    buff,
                )
            } else {
                false
            };
        // A refused buff (a same-type buff of equal/higher level is already up)
        // changes nothing — don't schedule its expiry (a stale `BuffExpire` on a
        // shared skill id would drop the surviving buff early) or rebroadcast.
        if !landed {
            return false;
        }
        if !permanent {
            world.scheduler.schedule(
                expires_at_tick,
                ScheduledTask::BuffExpire {
                    player_object_id: target_oid,
                    skill_id: skill.id,
                },
            );
        }
        let now = world.tick;
        if let Some(client_id) = client_for_player(world, target_oid)
            && let Some(buffs) = world.objects.get_component::<Buffs>(&target_oid)
            && let Some(cs) = world.clients.get(&client_id)
        {
            cs.send(crate::network::enter_world::abnormal_status_update(
                buffs, now,
            ));
        }
        // Max HP/MP/CP live on a separate path from `recalculate_stats`; fold
        // the buff's MaxHp/MaxMp/MaxCp modifiers into them too (e.g. a +MP buff).
        recompute_max_vitals(world, target_oid);
        if skill.effect_flags() & crate::model::skill::effect_flag::BLOCK_ACTIONS != 0 {
            apply_block_actions_interrupt(world, target_oid);
        }
        apply_mute_interrupt(world, target_oid, skill);
        // A stat buff changed pAtk/pDef/speed/…; Java's `recalculateStats(true)`
        // follows with `broadcastUserInfo()`. Without this the client shows the
        // buff icon but never the changed stats or movement speed (and other
        // players never see the speed change).
        crate::game_loop::party::broadcast_user_info(world, target_oid);
        // Java pushes the visual set only from `startAbnormalVisualEffect` /
        // `stopAbnormalVisualEffect`, i.e. only when the set actually changed —
        // not on every buff. A skill with no `<abnormalVisualEffect>` can't have
        // changed anything, so it sends nothing.
        if !skill.abnormal_visuals.is_empty() {
            refresh_abnormal_visuals(world, target_oid);
        }
        // `Transformation` landed: the `UserInfo`/`CharInfo` broadcast above
        // already carries the new display id, but the client also needs the
        // self-only `ExUserInfoAbnormalVisualEffect` (transform display id) and
        // a refreshed `SkillList` for the transform's granted skills to show up
        // — the two extras `admin::transforms::apply_transform`'s broadcast
        // sends on top of `broadcast_user_info`.
        if skill
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::Transform { .. }))
        {
            crate::game_loop::admin::transforms::refresh_transform_visuals(world, target_oid);
        }
    }
    true
}

/// Java `Player.restoreEffects` (buff half): re-apply the buffs a character was
/// carrying at logout, each with the remaining time that was stored — the
/// countdown resumes where it stopped rather than accounting for the time spent
/// offline, which is what makes an hour-long buff still an hour long after an
/// overnight logout.
///
/// Runs after the character is spawned, since applying a buff touches the live
/// stat/scheduler/packet paths. Each row goes through
/// [`apply_continuous_effects`] with the stored duration as Java's custom
/// `abnormalTime`, self-cast (`effector == effected`, matching Java's
/// `applyEffects(this, this, …)`), which also means the debuff resist roll is
/// skipped — a debuff that was up at logout comes back rather than getting a
/// second chance to be resisted.
///
/// A row whose skill no longer exists (datapack change, skill removed) is
/// dropped silently, like Java's `skill == null` continue.
pub(crate) fn restore_persisted_buffs(
    world: &mut World,
    object_id: i32,
    rows: &[crate::db::SkillBuffRow],
) {
    for row in rows {
        let Some(skill) = world
            .data
            .skill_data
            .get(row.skill_id, row.skill_level)
            .cloned()
        else {
            continue;
        };
        apply_continuous_effects(
            world,
            object_id,
            object_id,
            &skill,
            Some(row.remaining_time_secs),
        );
    }
}

/// Push the creature's current abnormal-visual set to their **own** client
/// (`ExUserInfoAbnormalVisualEffect`). The set other people see rides on the
/// `CharInfo` that `broadcast_user_info` already sends; this is the self-facing
/// half, without which a stunned player sees no swirl on themselves.
fn refresh_abnormal_visuals(world: &World, object_id: i32) {
    let Some(client_id) = client_for_player(world, object_id) else {
        return;
    };
    let visuals = crate::game_loop::abnormal::visual_effects(world, object_id);
    let invisible = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.hidden);
    let transform = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map_or(0, |p| p.transform_display_id);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(
            crate::network::user_info::ex_user_info_abnormal_visual_effect(
                object_id, invisible, transform, &visuals,
            ),
        );
    }
}

/// `broadcastPacket(new ChangeWaitType(creature, moveType))` — the fake-death
/// pose, sent to observers **and** the player themselves (Java's `Player`
/// override makes `broadcastPacket` include self).
fn broadcast_change_wait_type(world: &mut World, object_id: i32, move_type: i32) {
    let Some(pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
        .copied()
    else {
        return;
    };
    let pkt = server_packets::change_wait_type(object_id, move_type, pos.x, pos.y, pos.z);
    crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
}

/// `Creature.stopFakeDeath` — get back up: tell every client to end the pose
/// and re-`Revive` the body (Java sends both, with a comment about a client
/// quirk that needs the second one).
///
/// Java also calls `setRecentFakeDeath(true)` here, starting the
/// `isRecentFakeDeath()` grace period during which mobs still ignore you.
/// **`PlayerFakeDeathUpProtection = 0` on this dist**, so that window is zero
/// seconds wide and the flag can never read true — not ported, matching the
/// `MP_BLOCK`/`MAX_MOMENTUM` precedent for config-disabled behaviour.
pub(crate) fn stop_fake_death(world: &mut World, object_id: i32) {
    broadcast_change_wait_type(world, object_id, server_packets::wait_type::STOP_FAKEDEATH);
    let pkt = server_packets::revive(object_id);
    crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
}

/// `TriggerSkillByDamage`'s `onDamageReceivedEvent` — the mirror of
/// [`fire_attack_triggers`], evaluated for every hit the **bearer takes**.
///
/// Same subscription-versus-scan trade as the attack side: Java attaches a
/// listener when the carrying buff starts, this port scans the victim's skill
/// book at damage time.
///
/// Java's gates in order: not a DoT tick, no self-hits, the attacker level
/// window, the damage floor, the chance roll, the `hpPercent` **upper** bound
/// on the bearer's HP share, and the `attackerType` narrowing (Mirage takes
/// `Playable`, so a mob hitting you never sets it off).
pub(crate) fn fire_damage_received_triggers(
    world: &mut World,
    victim_oid: i32,
    attacker_oid: i32,
    damage: i32,
    is_dot: bool,
) {
    // `event.isDamageOverTime()` and `attacker == target` both bail.
    if is_dot || victim_oid == attacker_oid {
        return;
    }
    // Java attaches the listener to the **buff**, so the carriers here are the
    // bearer's live effects — not their skill book. That is the opposite of
    // `fire_attack_triggers`, whose carriers are passives folded into
    // `StatModifiers` and therefore absent from the buff list; knowing Mirage
    // and being under it are different things.
    let Some(buffs) = world.objects.get_component::<Buffs>(&victim_oid) else {
        return;
    };
    let known: Vec<(i32, i32)> = buffs
        .0
        .iter()
        .filter(|a| !a.passive)
        .map(|a| (a.skill_id, a.skill_level))
        .collect();

    let attacker_is_playable = world
        .objects
        .has_component::<crate::model::Player>(&attacker_oid)
        || world
            .objects
            .has_component::<crate::model::components::PetOf>(&attacker_oid)
        || world
            .objects
            .has_component::<crate::model::components::ServitorOf>(&attacker_oid);

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
            continue;
        };
        for effect in &carrier.effects {
            let SkillEffect::TriggerSkillByDamage {
                min_damage,
                chance,
                skill_id: trigger_id,
                skill_level: trigger_level,
                hp_percent,
                attacker_playable_only,
                on_attacker,
            } = effect
            else {
                continue;
            };
            if *chance == 0 || *trigger_id == 0 || *trigger_level == 0 {
                continue;
            }
            if damage < *min_damage {
                continue;
            }
            if *attacker_playable_only && !attacker_is_playable {
                continue;
            }
            // `hpPercent` is an *upper* bound: Java bails when the bearer is
            // healthier than it. 100 (the default) can never bail.
            if *hp_percent < 100 {
                let share = world
                    .objects
                    .get_component::<Vitals>(&victim_oid)
                    .filter(|v| v.max_hp > 0)
                    .map(|v| v.cur_hp * 100.0 / v.max_hp as f64)
                    .unwrap_or(100.0);
                if share > *hp_percent as f64 {
                    continue;
                }
            }
            // `Rnd.get(100) > _chance` bails — `chance` itself passes.
            if *chance < 100 && world.roll(100) > *chance {
                continue;
            }
            fired.push((*trigger_id, *trigger_level, *on_attacker));
        }
    }

    for (trigger_id, trigger_level, on_attacker) in fired {
        let Some(trigger) = world
            .data
            .skill_data
            .get(trigger_id, trigger_level)
            .cloned()
        else {
            continue;
        };
        // `targetType`: ENEMY casts back at whoever hit you, SELF on yourself.
        let target = if on_attacker {
            attacker_oid
        } else {
            victim_oid
        };
        let already = world
            .objects
            .get_component::<Buffs>(&target)
            .is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.skill_id == trigger_id && x.skill_level >= trigger_level)
            });
        if already {
            continue;
        }
        // Java's `triggerCast(event.getAttacker(), target, skill)` — note the
        // *attacker* is the caster of the counter-trigger, not the bearer.
        apply_skill_effects(world, attacker_oid, target, &trigger);
    }
}

/// `TriggerSkillByMagicType`'s `onSkillUseEvent` — fires when the bearer
/// finishes casting a skill whose `magicType` is in the list.
///
/// Dance of Shadows (366) is the learnable carrier: any ordinary cast fires
/// Cancel Shadow Move (7097) on the party, which is how the dance's stealth
/// ends the moment you do something.
pub(crate) fn fire_magic_type_triggers(
    world: &mut World,
    caster_oid: i32,
    cast_target_oid: i32,
    cast_magic_type: i32,
) {
    // Carriers are live buffs, not book entries — see the note on
    // `fire_damage_received_triggers`.
    let Some(buffs) = world.objects.get_component::<Buffs>(&caster_oid) else {
        return;
    };
    let known: Vec<(i32, i32)> = buffs
        .0
        .iter()
        .filter(|a| !a.passive)
        .map(|a| (a.skill_id, a.skill_level))
        .collect();

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
            continue;
        };
        for effect in &carrier.effects {
            let SkillEffect::TriggerSkillByMagicType {
                magic_types,
                chance,
                skill_id: trigger_id,
                skill_level: trigger_level,
                on_party,
            } = effect
            else {
                continue;
            };
            if *chance == 0 || *trigger_id == 0 || *trigger_level == 0 || magic_types.is_empty() {
                continue;
            }
            if !magic_types.contains(&cast_magic_type) {
                continue;
            }
            if *chance < 100 && world.roll(100) > *chance {
                continue;
            }
            fired.push((*trigger_id, *trigger_level, *on_party));
        }
    }

    for (trigger_id, trigger_level, on_party) in fired {
        let Some(trigger) = world
            .data
            .skill_data
            .get(trigger_id, trigger_level)
            .cloned()
        else {
            continue;
        };
        // Java resolves the trigger's own `targetType` against the *triggering
        // cast's* target, not the bearer — so the default `TARGET` lands on
        // whoever was just hit, and `MY_PARTY` on the caster's party.
        let targets = if on_party {
            world
                .objects
                .get_component::<crate::model::components::PartyRef>(&caster_oid)
                .and_then(|r| world.parties.get(&r.0))
                .map(|p| p.members.clone())
                .unwrap_or_else(|| vec![caster_oid])
        } else {
            vec![cast_target_oid]
        };
        for t in targets {
            let already = world.objects.get_component::<Buffs>(&t).is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.skill_id == trigger_id && x.skill_level >= trigger_level)
            });
            if already {
                continue;
            }
            apply_skill_effects(world, caster_oid, t, &trigger);
        }
    }
}

/// `TriggerSkillByAttack`'s `onAttackEvent`, evaluated for every hit the
/// attacker lands (`combat::handle_attack_hit`).
///
/// Java subscribes each effect to `OnCreatureDamageDealt` when the carrying
/// skill starts. These carriers are *passives* (weapon masteries), whose
/// effects this port folds into `StatModifiers` rather than keeping as a live
/// effect list — so instead of a subscription the attacker's skill book is
/// scanned at hit time. That is a handful of `HashMap` lookups per swing; if it
/// ever shows up in a profile it should become a cached index like
/// `NpcAiSkillIndex`, not a behavioural change.
///
/// Ported gates, in Java's order: damage floor, **criticality equality**
/// (`isCritical != event.isCritical()` bails — so an `isCritical=false` trigger
/// fires only on non-crits), no self-hits, the chance roll, and the
/// `allowWeapons` mask. `allowSkillAttack` defaults to false and this is the
/// normal-attack path, so the skill-attack clause is satisfied by construction.
pub(crate) fn fire_attack_triggers(
    world: &mut World,
    attacker_oid: i32,
    target_oid: i32,
    damage: i32,
    crit: bool,
) {
    // `event.getAttacker() == event.getTarget()` bails.
    if attacker_oid == target_oid {
        return;
    }
    // Only players carry these skills on this dist (the three learnable
    // carriers are all class passives/dances).
    let Some(book) = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&attacker_oid)
    else {
        return;
    };
    let known: Vec<(i32, i32)> = book.0.iter().map(|(&id, &lvl)| (id, lvl)).collect();

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
            continue;
        };
        for effect in &carrier.effects {
            let SkillEffect::TriggerSkillByAttack {
                min_damage,
                chance,
                skill_id: trigger_id,
                skill_level: trigger_level,
                on_party,
                is_critical,
                allow_weapons,
            } = effect
            else {
                continue;
            };
            if *chance == 0 || damage < *min_damage || *is_critical != crit {
                continue;
            }
            // `Rnd.get(100) > _chance` bails — note `>`, so `chance` itself
            // still fires (a 100 chance is certain).
            if world.roll(100) > *chance {
                continue;
            }
            if *allow_weapons != 0 && !attacker_weapon_allowed(world, attacker_oid, *allow_weapons)
            {
                continue;
            }
            fired.push((*trigger_id, *trigger_level, *on_party));
        }
    }

    for (trigger_id, trigger_level, on_party) in fired {
        let Some(trigger) = world
            .data
            .skill_data
            .get(trigger_id, trigger_level)
            .cloned()
        else {
            continue;
        };
        // `targetType`: SELF or MY_PARTY. The party case reduces to the caster
        // when unpartied, which is how Java's PARTY target handler behaves too.
        let mut targets = vec![attacker_oid];
        if on_party {
            // Java's PARTY target handler treats an unpartied caster as a
            // party of one, which is also what `skills::affect` does.
            targets = world
                .objects
                .get_component::<crate::model::components::PartyRef>(&attacker_oid)
                .and_then(|r| world.parties.get(&r.0))
                .map(|p| p.members.clone())
                .unwrap_or_else(|| vec![attacker_oid]);
        }
        for t in targets {
            // Java's refresh guard: `if (buffInfo == null || buffInfo.getSkill()
            // .getLevel() < triggerSkill.getLevel())` — don't re-cast while the
            // same buff is already up at that level or higher.
            let already = world.objects.get_component::<Buffs>(&t).is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.skill_id == trigger_id && x.skill_level >= trigger_level)
            });
            if already {
                continue;
            }
            // `SkillCaster.triggerCast` — no cast time, no MP, no reuse.
            apply_skill_effects(world, attacker_oid, t, &trigger);
        }
    }
}

/// `event.getAttacker().getActiveWeaponItem().getItemType().mask() & _allowWeapons`.
fn attacker_weapon_allowed(world: &World, attacker_oid: i32, mask: u32) -> bool {
    let Some(inv) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&attacker_oid)
    else {
        return false;
    };
    crate::model::weapon_condition_passes(mask, inv, &world.data.item_data)
}

/// `Formulas.calcProbability` against the *effected* creature's level — the
/// shared chance gate on `Confuse` and `RandomizeHate`.
fn confuse_chance_passes(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
) -> bool {
    let level = target_level(world, target_oid);
    let attribute = attribute_mod(world, caster_oid, target_oid, skill);
    let trait_mod =
        calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, false);
    let roll = world.roll(100);
    formulas::calc_probability(skill.magic_level, chance, level, attribute, trait_mod, roll)
}

/// Java's `forEachVisibleObject(effected, Creature.class, …)` plus each
/// handler's own exclusions, then `targetList.get(Rnd.get(size))`.
///
/// `Confuse` excludes only the victim themselves (which the query already
/// does). `RandomizeHate` additionally excludes the caster and any attackable
/// **of the victim's own faction** — "aggro cannot be transfered to a mob of
/// the same faction" — which `exclude_caster_and_clan` selects.
fn random_bystander(
    world: &mut World,
    victim_oid: i32,
    caster_oid: i32,
    exclude_caster_and_clan: bool,
) -> Option<i32> {
    let mut candidates = crate::game_loop::helpers::visible_creatures(world, victim_oid);
    if exclude_caster_and_clan {
        candidates.retain(|&oid| oid != caster_oid && !same_npc_faction(world, victim_oid, oid));
    }
    if candidates.is_empty() {
        return None;
    }
    let idx = world.roll(candidates.len() as i32) as usize;
    candidates.get(idx).copied()
}

/// Java `((Attackable) cha).isInMyClan(effectedMob)` — two NPCs sharing a clan
/// tag. A player is never in an NPC's faction.
fn same_npc_faction(world: &World, a_oid: i32, b_oid: i32) -> bool {
    let clan_of = |oid: i32| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| n.template(world))
            .map(|t| t.clans.clone())
    };
    match (clan_of(a_oid), clan_of(b_oid)) {
        (Some(a), Some(b)) => a.iter().any(|c| b.contains(c)),
        _ => false,
    }
}

/// `effected.setTarget(target)` + `setIntention(AI_INTENTION_ATTACK, target)`,
/// in the two shapes this port has: hate for an NPC, a plain target swap for a
/// player.
fn retarget_onto(world: &mut World, victim_oid: i32, new_target_oid: i32) {
    if crate::game_loop::combat::is_npc_oid(victim_oid) {
        let max_hate = world
            .objects
            .get_component::<crate::model::npc::AggroList>(&victim_oid)
            .map(|a| a.0.values().map(|i| i.hate).fold(0.0_f64, f64::max))
            .unwrap_or(0.0);
        if let Some(aggro) = world
            .objects
            .get_component_mut::<crate::model::npc::AggroList>(&victim_oid)
        {
            aggro.0.entry(new_target_oid).or_default().hate = max_hate + 1.0;
        }
        if let Some(ai) = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(&victim_oid)
        {
            ai.intention = crate::model::npc::NpcIntention::Attack;
            ai.attack_timeout_tick = world.tick + crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
        }
    } else if let Some(client_id) = client_for_player(world, victim_oid) {
        crate::game_loop::target::set_target(world, client_id, victim_oid, Some(new_target_oid));
    }
}

/// `effected.getStat().getValue(Stat.MANA_CHARGE, amount)` — the recipient's
/// recharge bonus. Java's two-arg `getValue` is `mul * baseValue + add`, so
/// Higher Mana Gain 285 (`mode=DIFF`, +22..81 by level) is a flat addition.
fn mana_charge_of(world: &World, target_oid: i32, amount: f64) -> f64 {
    use crate::model::stats::Stat;
    let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&target_oid)
    else {
        return amount;
    };
    let mul = mods.mul.get(&Stat::ManaCharge).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::ManaCharge).copied().unwrap_or(0.0);
    (mul * amount) + add
}

/// `ManaHealByLevel`'s recharge penalty: a target more than 5 levels above the
/// skill's `magicLevel` gets progressively less, and 15+ levels above gets
/// **nothing at all**.
///
/// Java writes it as an `if/else if` ladder from `levelDiff == 6` (×0.9) down
/// to `== 14` (×0.1) with `>= 15` → 0; that is exactly `1 - (diff - 5)/10`
/// over the ladder's range, so it collapses to arithmetic here rather than
/// nine branches. A gap of 5 or less is unpenalised.
pub(crate) fn recharge_level_penalty(target_level: i32, skill_magic_level: i32) -> f64 {
    let diff = target_level - skill_magic_level;
    if diff <= 5 {
        return 1.0;
    }
    if diff >= 15 {
        return 0.0;
    }
    1.0 - ((diff - 5) as f64 / 10.0)
}

fn target_level(world: &World, oid: i32) -> i32 {
    if let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) {
        return p.level;
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .and_then(|n| n.template(world))
        .map(|t| t.level)
        .unwrap_or(1)
}

/// The tail every MP-restore handler shares: the dead / `isMpBlocked` gate, the
/// overheal clamp, the write, `broadcastStatusUpdate`, and the self-vs-other
/// system message.
///
/// Java clamps against `getMaxRecoverableMp()` (`MAX_RECOVERABLE_MP` over
/// `maxMp`). Two skills declare `LimitMp` — Seal of Limit (1509) and Mass
/// Restriction (11603) — but **neither is reachable**: 1509 appears on no
/// skill tree, NPC or item, and 11603 is post-Interlude. So the stat is
/// identity and the ceiling is plain `maxMp` here.
fn restore_mp(world: &mut World, caster_oid: i32, target_oid: i32, amount: f64) {
    use server_packets::{SmParam, sm_ids};
    // `effected.isDead() || effected.isDoor() || effected.isMpBlocked()`.
    if world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    if crate::game_loop::abnormal::is_mp_blocked(world, target_oid) {
        return;
    }
    // "Prevents overheal and negative amount".
    let restored = {
        let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
            return;
        };
        let headroom = (v.max_mp as f64 - v.cur_mp).max(0.0);
        let restored = amount.min(headroom).max(0.0);
        if restored != 0.0 {
            v.cur_mp += restored;
        }
        restored
    };
    if restored != 0.0 {
        broadcast_vitals(world, target_oid);
    }
    // Java sends the message even when the amount rounded to nothing.
    if let Some(cid) = client_for_player(world, target_oid) {
        let pkt = if caster_oid != target_oid {
            server_packets::system_message_with(
                sm_ids::S2_MP_HAS_BEEN_RESTORED_BY_C1,
                &[
                    SmParam::Text(caster_display_name(world, caster_oid)),
                    SmParam::Int(restored as i32),
                ],
            )
        } else {
            server_packets::system_message_with(
                sm_ids::S1_MP_HAS_BEEN_RESTORED,
                &[SmParam::Int(restored as i32)],
            )
        };
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(pkt);
        }
    }
}

/// `Creature.reduceCurrentHp`'s fake-death branch: any real damage taken while
/// playing dead ends the act (`stopFakeDeath(true)` — note the `true`, which
/// *removes the effect*, not just the pose). Finds whichever active buff
/// carries the `FAKE_DEATH` flag and expires it, which routes through
/// `handle_buff_expire` → [`stop_fake_death`] for the client-side stand-up.
pub(crate) fn break_fake_death_on_damage(world: &mut World, object_id: i32) {
    use crate::model::skill::effect_flag;
    if super::super::abnormal::flags_of(world, object_id) & effect_flag::FAKE_DEATH == 0 {
        return;
    }
    let skill_ids: Vec<i32> = world
        .objects
        .get_component::<Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .filter(|x| x.effect_flags & effect_flag::FAKE_DEATH != 0)
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default();
    for skill_id in skill_ids {
        handle_buff_expire(world, object_id, skill_id);
    }
}

/// Java `EffectList.stopEffectsOnDamage()` — drop every live buff whose skill
/// declares `<removedOnDamage>`, called from `CreatureStatus.reduceHp` /
/// `PlayerStatus.reduceHp` the moment the holder takes a hit.
///
/// This is what wakes a slept character: `Sleep` (1069, 1072, 1394, the mob
/// casts 4046/4185/4201/4660-4662, …) applies `BlockActions`, and the tag is
/// the *only* thing that takes it back off before the timer. Same tag breaks
/// `Hide` (922) and `Force Meditation` (441).
///
/// Java reads the flag off the `BuffInfo`'s skill (`info.getSkill()
/// .isRemovedOnDamage()`) rather than off a cached copy, so the buff's
/// `(skill_id, skill_level)` is resolved back through the skill table here for
/// the same reason — nothing to keep in sync, and buffs restored from the DB on
/// relog behave identically to freshly-cast ones.
pub(crate) fn stop_effects_on_damage(world: &mut World, object_id: i32) {
    let skill_ids: Vec<i32> = world
        .objects
        .get_component::<Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .filter(|x| {
                    world
                        .data
                        .skill_data
                        .get(x.skill_id, x.skill_level)
                        .is_some_and(|s| s.removed_on_damage)
                })
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default();
    for skill_id in skill_ids {
        handle_buff_expire(world, object_id, skill_id);
    }
}

/// How far one fear shove throws the victim — Java `Fear.FEAR_RANGE`.
const FEAR_RANGE: f64 = 500.0;

/// `Fear.canStart` — who can be feared at all. Raid bosses are immune (the
/// same `isRaid()` bail `Mute` has), and on the NPC side only the `Attackable`
/// subtree qualifies, minus the siege-defence family: a fear must not scatter
/// stationed defenders off a castle wall or push a siege golem around.
/// A player is always fearable. Java's `isSummon()` leg folds into the same
/// case, and servitors landed with G29 — so nothing is missing here; the two
/// legs are simply one branch in this port.
fn fear_can_start(world: &World, target_oid: i32) -> bool {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
    else {
        return true;
    };
    let Some(t) = npc.template(world) else {
        return false;
    };
    if t.is_raid() {
        return false;
    }
    // Java's `isSummon()` leg: a pet/servitor is fearable like a player. (In
    // this port a summon is an NPC entity, so it reaches the NPC branch below —
    // which its non-Attackable "Servitor" type would otherwise reject.)
    if world
        .objects
        .has_component::<crate::model::components::ServitorOf>(&target_oid)
        || world
            .objects
            .has_component::<crate::model::components::PetOf>(&target_oid)
    {
        return true;
    }
    t.is_attackable_class()
        && !matches!(
            t.type_name.as_str(),
            "Defender" | "FortCommander" | "SiegeFlag"
        )
        && t.race != Some(crate::enums::Race::SiegeWeapon as i32)
}

/// `Fear.fearAction` — one shove: pick a flight direction, project
/// [`FEAR_RANGE`] units along it, clamp the destination to walkable geodata and
/// walk there.
///
/// The direction is `Util.calculateAngleFrom(effector, effected)` on the first
/// shove — the angle *from the caster to the victim*, so the victim runs
/// directly away — and the victim's own heading (`convertHeadingToDegree`) on
/// every later tick, which keeps them fleeing the way they were first thrown
/// rather than re-deriving a bearing from a caster who may be dead or gone.
/// Java's `toRadians(atan2-in-degrees)` round-trip collapses to the raw
/// `atan2`, so the first case is computed directly in radians here.
fn fear_action(world: &mut World, effector: Option<i32>, effected: i32) {
    use crate::model::components::Position;
    // `Creature.moveToLocation`'s own bail — a rooted or stunned victim can't
    // be driven anywhere, though the fear's timer keeps running.
    if crate::game_loop::abnormal::is_movement_disabled(world, effected)
        || crate::game_loop::abnormal::is_blocked_from_actions(world, effected)
    {
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&effected).copied() else {
        return;
    };
    let radians = match effector.and_then(|e| world.objects.get_component::<Position>(&e).copied())
    {
        Some(src) => ((pos.y - src.y) as f64).atan2((pos.x - src.x) as f64),
        // `Util.convertHeadingToDegree`: heading / 182.044444444, in degrees.
        None => (pos.heading as f64 / 182.044_444_444).to_radians(),
    };
    let dest_x = (pos.x as f64 + FEAR_RANGE * radians.cos()) as i32;
    let dest_y = (pos.y as f64 + FEAR_RANGE * radians.sin()) as i32;
    // Java projects at the victim's *own* z and lets geodata correct it.
    let (vx, vy, vz) = world
        .geo
        .get_valid_location(pos.x, pos.y, pos.z, dest_x, dest_y, pos.z);

    // `getAI().setIntention(AI_INTENTION_MOVE_TO, destination)` — the player and
    // NPC halves of Java's shared `Creature.moveToLocation` (each already does
    // its own geodata/pathfinding pass on top of the clamp above).
    if let Some(client_id) = client_for_player(world, effected) {
        crate::game_loop::position::intention_move_to(
            world,
            client_id,
            effected,
            pos,
            (vx, vy, vz),
        );
    } else {
        // Set before the move: `move_npc_to` can bail (no speed, no path), and
        // Java changes the intention regardless of whether the walk starts.
        if let Some(ai) = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(&effected)
        {
            ai.intention = crate::model::npc::NpcIntention::MoveTo;
        }
        crate::game_loop::npc_ai::move_npc_to(world, effected, vx, vy, vz);
    }
}

/// `Mute.onStart` — silencing someone also drops the cast they were already
/// mid-way through, otherwise a mute landing during a cast would let that cast
/// finish. **Raid bosses are immune** (Java's `effected.isRaid()` bail), which
/// is what stops a single silence from neutering a raid.
///
/// Unlike a stun this does not touch movement — a silenced character walks
/// normally.
fn apply_mute_interrupt(world: &mut World, target_oid: i32, skill: &Skill) {
    let mutes = skill.effect_flags()
        & (crate::model::skill::effect_flag::MUTED
            | crate::model::skill::effect_flag::PHYSICAL_MUTED)
        != 0;
    if !mutes {
        return;
    }
    let is_raid = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_raid());
    if is_raid {
        return;
    }
    // Java's is `abortCast()` → `stopCasting(true)`, so the same
    // `MagicSkillCanceled` applies here: a silenced caster's animation has to
    // stop with the cast.
    crate::game_loop::skills::cast::abort_all_skill_casters(world, target_oid);
}

/// Java `AttackableStatus.reduceHp` + `Attackable.setOverhitValues`: bank the
/// *excess* damage of a killing `<overHit>` blow, so the kill reward can pay a
/// bonus for it.
///
/// `excess = damage - currentHp`. A blow that fails to kill (negative excess)
/// **disarms** the state — as does any damage from a non-overhit skill — so the
/// record only ever survives on a corpse, and only from the blow that made it
/// one.
fn record_overhit(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    damage: f64,
    over_hit: bool,
) {
    use crate::model::components::Overhit;
    if damage <= 0.0 {
        return;
    }
    let cur_hp = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0);
    let excess = damage - cur_hp;
    if !over_hit || excess < 0.0 {
        world.objects.remove_component::<Overhit>(&target_oid);
        return;
    }
    world.objects.add_components(
        &target_oid,
        Overhit {
            damage: excess,
            attacker: caster_oid,
        },
    );
}

/// `BlockActions.onStart` — `startParalyze()` (`abortCast` + `stopMove`) plus
/// `abortAllSkillCasters()` on the freshly-stunned victim: a skill that lands
/// `BLOCK_ACTIONS` interrupts whatever the target was doing, rather than only
/// preventing the *next* action. Without this a stun landing mid-cast would let
/// the cast finish.
///
/// The abort goes through [`cast::abort_all_skill_casters`], i.e. Java's
/// `stopCasting(true)` — an *aborted* stop, which broadcasts
/// `MagicSkillCanceled`. Dropping the cast quietly is not enough: that packet is
/// what stops the cast animation client-side, so a silent stop leaves a slept
/// mob (or player) visibly finishing its channel — and its skill FX playing —
/// for the rest of the client-side cast time after the sleep already landed.
///
/// A root deliberately does not do this — it stops movement (the movement
/// primitives refuse it from the next tick) but leaves a cast running.
///
/// TODO(G34): Java's `startParalyze` also calls `abortAttack()`, which drops the
/// swing already in flight (`CreatureAttackTaskManager.abortAttack`). This port
/// has no cancel handle on a scheduled `AttackHit`, so a stun landing between a
/// swing's start and its hit tick still lets that hit land.
fn apply_block_actions_interrupt(world: &mut World, target_oid: i32) {
    // Order matters: abort the cast *first*. `stop_casting` resumes the move
    // the cast interrupted (`start_casting` stashes it), so clearing movement
    // before the cast would see it immediately restored — the victim would keep
    // walking while stunned.
    crate::game_loop::skills::cast::abort_all_skill_casters(world, target_oid);
    // Then freeze them where they stand and tell everyone who can see them.
    if world
        .objects
        .has_component::<crate::model::components::Movement>(&target_oid)
    {
        world
            .objects
            .remove_component::<crate::model::components::Movement>(&target_oid);
        if let Some(pos) = world
            .objects
            .get_component::<crate::model::components::Position>(&target_oid)
            .copied()
            && let Some(region) = world
                .objects
                .get_component::<crate::model::components::RegionCell>(&target_oid)
                .map(|r| r.0)
        {
            crate::game_loop::helpers::broadcast_near_region(
                world,
                region,
                &server_packets::stop_move(target_oid, pos.x, pos.y, pos.z, pos.heading),
            );
        }
    }
    // Monsters additionally lose their chase leg; `think` will no-op while the
    // flag is up, and the AI resumes on its own once it expires.
}

/// A target creature's level (Java `Creature.getLevel()`) for the debuff
/// landing-rate math — an NPC reads its template, a player its record. Defaults
/// to 1, matching the Spoil landing-level fallback.
fn creature_level(world: &World, oid: i32) -> i32 {
    // Java `Cubic.getLevel()` → `_owner.getLevel()`. Checked before the NPC/
    // player split because a cubic's caster entity is neither.
    if let Some(c) = world
        .objects
        .get_component::<crate::model::components::CubicOf>(&oid)
    {
        return world
            .objects
            .get_component::<crate::model::Player>(&c.owner_object_id)
            .map(|p| p.level)
            .unwrap_or(1);
    }
    if crate::game_loop::combat::is_npc_oid(oid) {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| n.template(world))
            .map(|t| t.level)
            .unwrap_or(1)
    } else {
        world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .map(|p| p.level)
            .unwrap_or(1)
    }
}

/// `RebalanceHP.instant` — Balance Life (1043).
///
/// Two passes over the same set: sum `maxHp` and `curHp` across every living
/// party member in `affect_range` (plus their pet and servitors), then set each
/// of them to `maxHp * (sumCur / sumMax)`. Java bails outright when the caster
/// is not a player, and does nothing at all when there is no party — an
/// unpartied cast is wasted, which is *not* the "party of one" reading every
/// other party-scoped effect uses.
///
/// The heal direction matters: only a member whose HP goes **up** is clamped by
/// `MAX_RECOVERABLE_HP` (and a member already above that ceiling keeps what
/// they have rather than being pulled down to it). A member who loses HP is
/// written unconditionally — the ceiling guards heals, not the redistribution.
fn rebalance_party_hp(world: &mut World, caster_oid: i32, skill: &Skill) {
    if !world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    let Some(members) = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&caster_oid)
        .and_then(|r| world.parties.get(&r.0))
        .map(|p| p.members.clone())
    else {
        // No party: Java's `if (party != null)` guard skips the whole effect.
        return;
    };
    let Some(origin) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
        .copied()
    else {
        return;
    };
    let range = skill.affect_range;
    let in_range = |world: &World, oid: i32| -> bool {
        // `Util.checkIfInRange(range, effector, target, true)` — 3D, and a
        // range of 0 means "no distance filter" the same way the affect
        // helpers read it.
        if range <= 0 {
            return true;
        }
        world
            .objects
            .get_component::<crate::model::components::Position>(&oid)
            .is_some_and(|p| {
                let (dx, dy, dz) = (
                    (origin.x - p.x) as f64,
                    (origin.y - p.y) as f64,
                    (origin.z - p.z) as f64,
                );
                dx * dx + dy * dy + dz * dz <= (range as f64) * (range as f64)
            })
    };

    // Every creature the effect touches: each member, then their pet and
    // servitor. Java walks all three lists twice; collecting once keeps the
    // two passes over exactly the same set.
    let mut touched: Vec<i32> = Vec::new();
    for member in &members {
        for oid in std::iter::once(*member)
            .chain(crate::game_loop::servitor::pet_of(world, *member))
            .chain(crate::game_loop::servitor::servitor_of(world, *member))
        {
            let alive = world
                .objects
                .get_component::<Vitals>(&oid)
                .is_some_and(|v| !v.dead);
            if alive && in_range(world, oid) {
                touched.push(oid);
            }
        }
    }

    let (mut full_hp, mut current_hp) = (0.0f64, 0.0f64);
    for &oid in &touched {
        if let Some(v) = world.objects.get_component::<Vitals>(&oid) {
            full_hp += v.max_hp as f64;
            current_hp += v.cur_hp;
        }
    }
    if full_hp <= 0.0 {
        return;
    }
    let percent = current_hp / full_hp;

    for &oid in &touched {
        let Some(v) = world.objects.get_component::<Vitals>(&oid).copied() else {
            continue;
        };
        let mut new_hp = v.max_hp as f64 * percent;
        if new_hp > v.cur_hp {
            let ceiling = max_recoverable(
                world,
                oid,
                crate::model::stats::Stat::MaxRecoverableHp,
                v.max_hp as f64,
            );
            if v.cur_hp > ceiling {
                new_hp = v.cur_hp;
            } else if new_hp > ceiling {
                new_hp = ceiling;
            }
        }
        if let Some(vit) = world.objects.get_component_mut::<Vitals>(&oid) {
            vit.cur_hp = new_hp.clamp(0.0, vit.max_hp as f64);
        }
        broadcast_vitals(world, oid);
    }
}

/// `target.isCastingNow(s -> s.getSkill().getAbnormalResists().contains(
/// skill.getAbnormalType()))` — is the target part-way through a cast that
/// declares immunity to this abnormal type?
///
/// Empty `abnormal_type` never matches: Java compares against an
/// `AbnormalType` enum whose `NONE` is not in any resist list.
fn casting_resists_abnormal(world: &World, target_oid: i32, abnormal_type: &str) -> bool {
    if abnormal_type.is_empty() {
        return false;
    }
    let Some(casting) = world
        .objects
        .get_component::<crate::model::components::Casting>(&target_oid)
    else {
        return false;
    };
    world
        .data
        .skill_data
        .get(casting.0.skill_id, casting.0.skill_level)
        .is_some_and(|s| {
            s.abnormal_resists
                .iter()
                .any(|t| t.eq_ignore_ascii_case(abnormal_type))
        })
}

/// Test hook for [`creature_level`], which is private to this module.
#[cfg(test)]
pub(crate) fn creature_level_for_test(world: &World, oid: i32) -> i32 {
    creature_level(world, oid)
}

/// A target creature's display name (Java `Creature.getName()`) for the debuff
/// landed/resisted caster line — an NPC's template name or the player's name.
fn creature_name(world: &World, oid: i32) -> String {
    if crate::game_loop::combat::is_npc_oid(oid) {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| n.template(world))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    } else {
        world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }
}

/// `CallParty.instant` — Chant of Gate (1429).
///
/// Every *other* party member is pulled to the caster. There is deliberately no
/// `ConfirmDlg` here: unlike Summon Friend, Java calls `teleToLocation`
/// outright, so a party member gets no say in it.
///
/// Each member is gated by `CallPc.checkSummonTargetStatus`, whose refusals are
/// **messaged to the caster**, not the member — the ported subset is dead, in a
/// private store, and in combat (Java also checks rooted, olympiad, observer,
/// flying mount, combat flag, the `NO_SUMMON_FRIEND`/`JAIL` zones and instance
/// permissions; none of those states are modelled for this path yet).
/// TODO(G34): extend the gate list as those states land.
fn call_party(world: &mut World, caster_oid: i32) {
    use server_packets::{SmParam, sm_ids};

    let Some(members) = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&caster_oid)
        .and_then(|r| world.parties.get(&r.0))
        .map(|p| p.members.clone())
    else {
        // `if (party == null) return` — solo, the cast is simply wasted.
        return;
    };
    let Some(dest) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
        .copied()
    else {
        return;
    };

    for member in members {
        // `effector != partyMember` — the caster is not recalled to itself.
        if member == caster_oid {
            continue;
        }
        let name = world
            .objects
            .get_component::<crate::model::Player>(&member)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let refusal = if world
            .objects
            .get_component::<Vitals>(&member)
            .is_none_or(|v| v.dead)
        {
            Some(sm_ids::C1_IS_DEAD_AT_THE_MOMENT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED)
        } else if world
            .objects
            .get_component::<crate::model::Player>(&member)
            .is_some_and(|p| p.store_type != 0)
        {
            Some(
                sm_ids::C1_IS_CURRENTLY_TRADING_OR_OPERATING_A_PRIVATE_STORE_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED,
            )
        } else if crate::game_loop::combat::has_attack_stance(world, member) {
            // `isInCombat()` — the attack stance is exactly Java's flag.
            Some(sm_ids::C1_IS_ENGAGED_IN_COMBAT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED)
        } else {
            None
        };
        if let Some(sm) = refusal {
            send_sm_with(world, caster_oid, sm, &[SmParam::PlayerName(name)]);
            continue;
        }
        crate::game_loop::death::teleport_player(world, member, dest.x, dest.y, dest.z);
    }
}

/// `handlers/effecthandlers/CallPc.java`, the `player == null` branch — a
/// **monster** dragging its victim to itself. This is Porta's (20213) "Summon"
/// (4161), and Java's body is five lines:
///
/// ```text
/// effected.abortCast();
/// effected.abortAttack();
/// effected.stopMove(null);
/// effected.sendPacket(new FlyToLocation(effected, effector, FlyType.DUMMY, …));
/// effected.setLocation(effector.getLocation());
/// ```
///
/// Note `setLocation`, **not** `teleToLocation`: no fade, no decay/respawn, no
/// `Appearing` round trip. The victim slides across on the client and the
/// server just moves the point. The whole hop is bounded by the skill's
/// `castRange` (600 for 4161), so it never crosses more than one world region
/// and the ordinary visibility sweep picks up the new neighbourhood.
///
/// The `TargetType::Enemy` gate is Java's: `CallPc` on any other target type
/// from a non-player effector falls to the `teleToLocation` branch, which is
/// the *player* being recalled — not something a monster does.
fn call_pc(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    // "if (effector == effected) return" — a mob can't summon itself.
    if caster_oid == target_oid {
        return;
    }
    // The ported half is the NPC one; a player effector wants the Summon
    // Friend `ConfirmDlg` round trip, which isn't built (see
    // `SkillEffect::CallPc`).
    if world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    if skill.target_type != crate::model::skill::TargetType::Enemy {
        return;
    }
    // `effected.getActingPlayer()` — the branch is player-only; a servitor
    // caught in the cast is left where it stands, as in Java.
    if !world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
    {
        return;
    }
    let Some(dest) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
        .copied()
    else {
        return;
    };
    let Some(from) = world
        .objects
        .get_component::<crate::model::components::Position>(&target_oid)
        .copied()
    else {
        return;
    };

    // `abortCast()` / `abortAttack()` / `stopMove(null)`.
    //
    // `abortCast()` is `SkillCaster.canAbortCast`-gated — a *target* check, not
    // the phase check its Java comment claims — so it takes the same helper the
    // teleport prologue uses, not [`super::cast::abort_cast`], whose `!launched`
    // guard would swallow the `MagicSkillCanceled` that stops the victim's own
    // cast animation client-side.
    super::cast::abort_cast_when_untargeted(world, target_oid);
    world
        .objects
        .remove_component::<crate::model::components::AttackState>(&target_oid);
    world
        .objects
        .remove_component::<crate::model::components::Movement>(&target_oid);
    world
        .objects
        .remove_component::<crate::model::components::Intent>(&target_oid);
    // Java's `stopMove(null)` ends with `broadcastPacket(new StopMove(this))`.
    // Dropping the `Movement` component only stops the *server* walking the
    // victim; without the packet every client keeps animating the run toward
    // the old destination, so the drag leaves the character sliding. Java
    // broadcasts it before `setLocation`, i.e. at the old point.
    crate::game_loop::helpers::broadcast_including_self(
        world,
        target_oid,
        &server_packets::stop_move(target_oid, from.x, from.y, from.z, from.heading),
    );

    // Java's `FlyToLocation` constructor arms `blinkActive` for a player
    // target, which makes the next `ValidatePosition` skip its out-of-sync
    // snap — otherwise the victim's own stale position report drags it back
    // out of the mob's lap.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    {
        p.blink_active = true;
    }
    // Java sends `FlyToLocation` to the effected player only; everyone else
    // learns the new position from the movement/validate-position stream. The
    // port broadcasts it so bystanders see the yank rather than a silent
    // teleport — the packet is a pure animation and the client ignores it for
    // objects it can't see.
    crate::game_loop::helpers::broadcast_including_self(
        world,
        target_oid,
        &server_packets::fly_to_location(
            target_oid,
            (from.x, from.y, from.z),
            (dest.x, dest.y, dest.z),
            server_packets::FlyType::Dummy,
        ),
    );

    if let Some(pos) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&target_oid)
    {
        pos.x = dest.x;
        pos.y = dest.y;
        pos.z = dest.z;
    }
    if let Some(region) = world
        .objects
        .get_component_mut::<crate::model::components::RegionCell>(&target_oid)
    {
        region.0 = crate::world::region_of(dest.x, dest.y);
    }
    // Java sends nothing else here — in particular no `MagicSkillCanceled` for
    // the caster. A cancel would end the summoning FX the client keeps drawing
    // for the skill's own (skillgrp) duration, past the 2 s cast; Java has that
    // leftover too, so the port keeps it rather than inventing a packet.
}

/// `handlers/effecthandlers/Restoration.java` — instant single-item grant.
/// Backs item-use skills wrapping a fixed pack/box reward (spiritshot packs,
/// jewelry boxes, …): the item's `<skills>` entry casts a skill with this
/// effect, and *that* is where the actual reward comes from — before this
/// was ported, such skills loaded with an empty effect list, so the item was
/// still consumed (`items::use_item_skills` destroys it once any skill
/// "lands") but granted nothing.
fn give_item(
    world: &mut World,
    target_oid: i32,
    item_id: i32,
    item_count: i64,
    item_enchant_level: i32,
) {
    use server_packets::sm_ids;

    if item_id <= 0 || item_count <= 0 {
        if let Some(client_id) = client_for_player(world, target_oid)
            && let Some(cs) = world.clients.get(&client_id)
        {
            cs.send(server_packets::system_message_with(
                sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE,
                &[],
            ));
        }
        return;
    }
    // Java `Restoration`: `if (_itemEnchantmentLevel > 0) setEnchantLevel(...)`.
    grant_and_notify(
        world,
        target_oid,
        &[(item_id, item_count, item_enchant_level.max(0))],
    );
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
        if let Some(client_id) = client_for_player(world, target_oid)
            && let Some(cs) = world.clients.get(&client_id)
        {
            cs.send(server_packets::system_message_with(
                sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE,
                &[],
            ));
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
    use server_packets::{SmParam, sm_ids};

    for &(item_id, amount, enchant) in grants {
        let Some(changed_oids) =
            crate::game_loop::items::add_inventory_item(world, target_oid, item_id, amount)
        else {
            continue;
        };
        // Stamp the rolled/fixed enchant onto the freshly created item(s). Only
        // non-stackable items carry an enchant; a stackable grant returns an
        // existing stack's oid, which must not be touched.
        if enchant > 0
            && !world
                .data
                .item_data
                .get(item_id)
                .map(|t| t.is_stackable)
                .unwrap_or(false)
            && let Some(inv) = world.objects.get_component_mut::<Inventory>(&target_oid)
        {
            for &oid in &changed_oids {
                inv.set_item_enchant(oid, enchant);
            }
        }
        let Some(inventory) = world.objects.get_component::<Inventory>(&target_oid) else {
            continue;
        };
        if let Some(client_id) = client_for_player(world, target_oid) {
            let iu = crate::network::enter_world::inventory_update(
                inventory,
                &world.data,
                &changed_oids,
            );
            if let Some(cs) = world.clients.get(&client_id) {
                // Java `RestorationRandom.sendMessage`: count>1 → "obtained S2 S1";
                // single enchanted → "obtained a +S1 S2"; else "obtained S1".
                let sm = if amount > 1 {
                    server_packets::system_message_with(
                        sm_ids::YOU_HAVE_OBTAINED_S2_S1,
                        &[SmParam::ItemName(item_id), SmParam::Long(amount)],
                    )
                } else if enchant > 0 {
                    server_packets::system_message_with(
                        sm_ids::YOU_HAVE_OBTAINED_A_S1_S2,
                        &[SmParam::Int(enchant), SmParam::ItemName(item_id)],
                    )
                } else {
                    server_packets::system_message_with(
                        sm_ids::YOU_HAVE_OBTAINED_S1,
                        &[SmParam::ItemName(item_id)],
                    )
                };
                cs.send(sm);
            }
            crate::game_loop::helpers::send_inventory_update(world, client_id, target_oid, iu);
        }
    }
}

/// Send a bare (no-argument) system message to `player_oid`, if online.
fn send_sm(world: &World, player_oid: i32, sm_id: i16) {
    crate::game_loop::helpers::send_sm_to_player(world, player_oid, sm_id, &[]);
}

/// `Creature.broadcastSocialAction` — a playable's emote goes to everyone in
/// range *including* itself (`broadcastPacket`), unlike the quest engine's
/// self-only `sendPacket` variant.
fn broadcast_social_action(world: &mut World, oid: i32, action_id: i32) {
    let Some(region) = world.objects.get_component::<RegionCell>(&oid).map(|r| r.0) else {
        return;
    };
    let pkt = server_packets::social_action(oid, action_id);
    crate::game_loop::helpers::broadcast_near_region(world, region, &pkt);
}

/// Send a system message with parameters to `player_oid`, if online.
fn send_sm_with(world: &World, player_oid: i32, sm_id: i16, params: &[server_packets::SmParam]) {
    crate::game_loop::helpers::send_sm_to_player(world, player_oid, sm_id, params);
}

/// Resolve `Formulas.calcMagicSuccess`' inputs for a cast. `penalty` is the
/// caller-owned backing store for the config penalty table (the struct borrows
/// it), since `world` is re-borrowed mutably for the roll.
fn magic_success_input<'a>(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    penalty: &'a [f64],
) -> formulas::MagicSuccess<'a> {
    use crate::model::npc::Npc;

    // Java `attacker.isAttackable() || target.isAttackable()`. `isAttackable()`
    // is the `Attackable` class test (monsters, guards, defenders), not
    // `isAutoAttackable` — a peaceful Folk on either side takes the PvP branch.
    let is_attackable = |oid: i32| {
        crate::game_loop::combat::is_npc_oid(oid)
            && world
                .objects
                .get_component::<Npc>(&oid)
                .and_then(|n| n.template(world))
                .is_some_and(|t| t.is_attackable_class())
    };

    let caster_player_level = world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .map(|p| p.level);

    // `target.isRaid() || target.isRaidMinion()` — a minion counts as a raid
    // only when its leader is one (Java sets `_isRaidMinion` from the spawning
    // raid boss, not from the minion's own template).
    let target_is_raid = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_raid())
        || world
            .objects
            .get_component::<crate::game_loop::minions::MinionOf>(&target_oid)
            .and_then(|leader| world.objects.get_component::<Npc>(&leader.0))
            .and_then(|n| n.template(world))
            .is_some_and(|t| t.is_raid());

    formulas::MagicSuccess {
        pve: is_attackable(caster_oid) || is_attackable(target_oid),
        target_level: creature_level(world, target_oid),
        effective_level: if world
            .cfg
            .character
            .calculate_magic_success_by_skill_magic_level
            && skill.magic_level > 0
        {
            skill.magic_level
        } else {
            caster_level(world, caster_oid)
        },
        caster_player_level,
        target_is_raid,
        min_npc_level_for_magic_penalty: world.cfg.npc.min_npc_level_for_magic_penalty,
        skill_chance_penalty: penalty,
        // `target.getStat().getMul(MAGIC_SUCCESS_RES, 1)` — read off the
        // *target*, and 1.0 for anyone without Anti Magic / M. Def.
        res_modifier: world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&target_oid)
            .and_then(|m| {
                m.mul
                    .get(&crate::model::stats::Stat::MagicSuccessRes)
                    .copied()
            })
            .unwrap_or(1.0),
        magic_accuracy: world
            .objects
            .get_component::<CombatStats>(&caster_oid)
            .map(|c| c.magic_accuracy)
            .unwrap_or(0),
        magic_evasion: world
            .objects
            .get_component::<CombatStats>(&target_oid)
            .map(|c| c.magic_evasion)
            .unwrap_or(0),
    }
}

/// `Formulas.calcMagicDam`'s `ALT_GAME_MAGICFAILURES` block: roll
/// `calcMagicSuccess`, and on a miss roll it a *second* time to pick between
/// half damage and a flat 1, messaging both sides the way Java does.
///
/// Two Java quirks are load-bearing here and deliberately preserved:
/// 1. The second roll — and therefore the damage reduction — only happens when
///    the attacker is a player. An NPC caster that fails the first roll deals
///    **full** damage; only the player target's "You resisted" line is sent.
/// 2. Both the attacker-side and target-side messages fire on the same failure,
///    so a resisted PvP nuke messages caster and victim.
fn roll_magic_failure(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    is_drain: bool,
) -> formulas::MagicFailure {
    use server_packets::{SmParam, sm_ids};

    if !world.cfg.character.magic_failures {
        return formulas::MagicFailure::None;
    }

    let penalty = world
        .cfg
        .npc
        .skill_chance_penalty_for_lvl_differences
        .clone();
    let input = magic_success_input(world, caster_oid, target_oid, skill, &penalty);
    if formulas::calc_magic_success(&input, world.roll(100)) {
        return formulas::MagicFailure::None;
    }

    let caster_is_player = world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .is_some();
    let target_is_player = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .is_some();

    let outcome = if caster_is_player {
        // Java re-runs `calcMagicSuccess` here — an independent second roll,
        // not a reuse of the first one's result.
        let input = magic_success_input(world, caster_oid, target_oid, skill, &penalty);
        if formulas::calc_magic_success(&input, world.roll(100)) {
            if is_drain {
                // A drain keeps its own retail line, which says the same thing
                // in the terms of the skill that caused it.
                send_sm(world, caster_oid, sm_ids::DRAIN_WAS_ONLY_50_SUCCESSFUL);
            } else {
                // Java `Formulas.calcMagicDam`: the caster is told whose
                // resistance halved it, target first then attacker, both as
                // plain names.
                use commons::system_messages::generated::DAMAGE_IS_DECREASED_BECAUSE_C1_RESISTED_C2_S_MAGIC;
                let message = DAMAGE_IS_DECREASED_BECAUSE_C1_RESISTED_C2_S_MAGIC::new(
                    creature_name(world, target_oid),
                    creature_name(world, caster_oid),
                );
                if let Some(client_id) = client_for_player(world, caster_oid)
                    && let Some(cs) = world.clients.get(&client_id)
                {
                    cs.send(server_packets::system_message(&message));
                }
            }
            formulas::MagicFailure::Half
        } else {
            let target_name = creature_name(world, target_oid);
            send_sm_with(
                world,
                caster_oid,
                sm_ids::C1_HAS_RESISTED_YOUR_S2,
                &[
                    SmParam::Text(target_name),
                    SmParam::SkillName {
                        id: skill.id,
                        level: skill.level,
                    },
                ],
            );
            formulas::MagicFailure::Resisted
        }
    } else {
        // NPC caster: Java leaves `damage` untouched.
        formulas::MagicFailure::None
    };

    if target_is_player {
        let caster_name = caster_display_name(world, caster_oid);
        send_sm_with(
            world,
            target_oid,
            if is_drain {
                sm_ids::YOU_RESISTED_C1_S_DRAIN
            } else {
                sm_ids::YOU_RESISTED_C1_S_MAGIC
            },
            &[crate::network::server_packets::SmParam::Text(caster_name)],
        );
    }

    outcome
}

/// `handlers/effecthandlers/Spoil.java` + its `calcSuccess`
/// (`Formulas.calcMagicSuccess`): mark a live monster spoiled so its `<spoil>`
/// list rolls into sweep loot on death, wake its AI (`EVT_ATTACKED`), and
/// message the caster. Non-monster/dead targets are rejected; an already-
/// spoiled mob reports it; a resisted cast lands silently (no effect).
fn apply_spoil(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    use crate::model::npc::Npc;
    use server_packets::sm_ids;

    // `!effected.isMonster() || effected.isDead()` → INVALID_TARGET.
    let is_monster = crate::game_loop::combat::is_npc_oid(target_oid)
        && world
            .objects
            .get_component::<Npc>(&target_oid)
            .and_then(|n| n.template(world))
            .is_some_and(|t| t.is_auto_attackable());
    let dead = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(true);
    if !is_monster || dead {
        send_sm(world, caster_oid, sm_ids::INVALID_TARGET);
        return;
    }
    // `target.isSpoiled()` → already spoiled.
    if world
        .objects
        .get_component::<Npc>(&target_oid)
        .map(|n| n.spoiler_object_id != 0)
        .unwrap_or(false)
    {
        send_sm(world, caster_oid, sm_ids::IT_HAS_ALREADY_BEEN_SPOILED);
        return;
    }
    // `calcSuccess` = `Formulas.calcMagicSuccess`, unconditional here — Spoil's
    // own handler calls it directly, so `MagicFailures` doesn't gate it.
    let penalty = world
        .cfg
        .npc
        .skill_chance_penalty_for_lvl_differences
        .clone();
    let input = magic_success_input(world, caster_oid, target_oid, skill, &penalty);
    if !formulas::calc_magic_success(&input, world.roll(100)) {
        // Magic resisted: `applyEffectScope` skips `instant()` — no effect,
        // and Java sends no message on a failed `calcSuccess`.
        return;
    }
    if let Some(npc) = world.objects.get_component_mut::<Npc>(&target_oid) {
        npc.spoiler_object_id = caster_oid;
    }
    send_sm(
        world,
        caster_oid,
        sm_ids::THE_SPOIL_CONDITION_HAS_BEEN_ACTIVATED,
    );
    // `target.getAI().notifyEvent(EVT_ATTACKED, effector)`.
    crate::game_loop::combat::npc_wake_on_attacked(world, target_oid, caster_oid);
}

/// `handlers/effecthandlers/Sweeper.java`: hand out the spoil loot rolled at
/// death (`Attackable.takeSweep`). The dead/spoiled/owner gate is enforced up
/// front by `resolve_cast_target` (the `OpSweeper` condition), so here we only
/// re-check ownership defensively and distribute the claimed items.
fn apply_sweeper(world: &mut World, caster_oid: i32, target_oid: i32) {
    use crate::model::components::Position;
    use crate::model::npc::Npc;

    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    // `checkSpoilOwner(player, false)` — silent (the message-carrying check ran
    // at cast start).
    let spoiler = world
        .objects
        .get_component::<Npc>(&target_oid)
        .map(|n| n.spoiler_object_id)
        .unwrap_or(0);
    if spoiler == 0
        || (spoiler != caster_oid
            && !crate::game_loop::party::same_party(world, caster_oid, spoiler))
    {
        return;
    }
    // `takeSweep()` — atomically claim the loot (a second sweep gets nothing).
    // TODO(G15): `checkInventorySlotsAndWeight` (inventory-full refusal) is
    // skipped. Weight *is* modelled (`game_loop::weight`, and G34 S4.1 added
    // the `WEIGHT_LIMIT`/`WEIGHT_PENALTY` stats); this path simply does not
    // consult it, so the gap is the check rather than the machinery.
    let Some(items) = world
        .objects
        .get_component_mut::<Npc>(&target_oid)
        .and_then(|n| n.sweep_items.take())
    else {
        return;
    };
    let corpse = world
        .objects
        .get_component::<Position>(&target_oid)
        .map(|p| (p.x, p.y))
        .unwrap_or((0, 0));
    for (item_id, count) in items {
        // Solo → the sweeper; partied `*_INCLUDING_SPOIL` → a party member.
        // Sweep loot always enters the looter's inventory (Java `addItem`),
        // bypassing the auto-loot ground-drop toggle.
        let looter = crate::game_loop::party::spoil_looter(world, caster_oid, corpse);
        grant_and_notify(world, looter, &[(item_id, count, 0)]);
    }
}

/// `handlers/effecthandlers/Sow.java` — the manor sow (skill 2097). The Seed
/// item handler has already flagged the mob (`seed_id`/`seeder_object_id`); on a
/// live `canBeSown` monster the caster sowed and hasn't yet seeded, roll
/// `calcSuccess` and — on success — mark it seeded and stash the crop it yields.
///
/// Java consumes the seed item inside this effect; this port consumes it via the
/// item-skill path that cast the sow skill (the Seed handler), so no consume
/// here — the same one-seed cost.
pub(crate) fn apply_sow(world: &mut World, caster_oid: i32, target_oid: i32) {
    use crate::model::Player;
    use crate::model::npc::{Npc, NpcAi, NpcIntention};

    let Some(player_level) = world
        .objects
        .get_component::<Player>(&caster_oid)
        .map(|p| p.level)
    else {
        return;
    };
    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    let Some((seed_id, seeder, seeded, can_be_sown, target_level, skill_ids)) = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|npc| {
            let state = (npc.seed_id, npc.seeder_object_id, npc.seeded);
            npc.template(world).map(|t| {
                (
                    state.0,
                    state.1,
                    state.2,
                    t.can_be_sown,
                    t.level,
                    t.skill_list.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                )
            })
        })
    else {
        return;
    };
    let dead = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(true);
    // Java: dead / !canBeSown / already seeded / not this player's seed → bail.
    if dead || !can_be_sown || seeded || seed_id == 0 || seeder != caster_oid {
        return;
    }
    let Some((crop_id, seed_level, alternative)) = world
        .data
        .manor
        .seed_by_id(seed_id)
        .map(|s| (s.crop_id, s.level, s.alternative))
    else {
        return;
    };

    if calc_sow_success(
        seed_level,
        alternative,
        player_level,
        target_level,
        world.roll(99),
    ) {
        // The crop count: a "strong type" mob (skills 4303..=4310) multiplies it
        // ×2..×9, plus a hi-level-mob bonus, all scaled by `RateDropManor`.
        let mut count: i64 = 1;
        for id in &skill_ids {
            if (4303..=4310).contains(id) {
                count *= (*id - 4301) as i64; // 4303→×2 … 4310→×9
            }
        }
        let diff = target_level - seed_level - 5;
        if diff > 0 {
            count += diff as i64;
        }
        let harvest_count = count * world.cfg.rates.rate_drop_manor as i64;
        if let Some(npc) = world.objects.get_component_mut::<Npc>(&target_oid) {
            npc.seeded = true;
            npc.harvest_item = Some((crop_id, harvest_count));
        }
        // TODO(manor): THE_SEED_WAS_SUCCESSFULLY_SOWN — SystemMessageId not in
        // this repo's data (the sow itself is applied).
    }
    // TODO(manor): the failure branch sends THE_SEED_WAS_NOT_SOWN (same reason).

    // Java sets the mob's AI to IDLE after a sow attempt.
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&target_oid) {
        ai.intention = NpcIntention::Active;
    }
}

/// `Sow.calcSuccess`: a level-scaled chance (base 90 %, or 20 % for the
/// alternative seed). **Java quirk kept**: its `Math.max(basicSuccess, 1)` is a
/// discarded statement, so `basic` is never floored — a large level mismatch
/// yields a ≤0 % (always-fail) chance.
fn calc_sow_success(
    seed_level: i32,
    alternative: bool,
    player_level: i32,
    target_level: i32,
    roll: i32,
) -> bool {
    let min = seed_level - 5;
    let max = seed_level + 5;
    let mut basic = if alternative { 20 } else { 90 };
    if target_level < min {
        basic -= 5 * (min - target_level);
    }
    if target_level > max {
        basic -= 5 * (target_level - max);
    }
    let diff = (player_level - target_level).abs();
    if diff > 5 {
        basic -= 5 * (diff - 5);
    }
    roll < basic
}

/// `handlers/effecthandlers/Harvesting.java` — the manor harvest (skill 2098):
/// on a dead, seeded corpse the caster sowed, roll `calcSuccess` and hand over
/// the stashed crop (`Attackable.takeHarvest`).
pub(crate) fn apply_harvesting(world: &mut World, caster_oid: i32, target_oid: i32) {
    use crate::model::Player;
    use crate::model::npc::Npc;

    let Some(player_level) = world
        .objects
        .get_component::<Player>(&caster_oid)
        .map(|p| p.level)
    else {
        return;
    };
    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    let dead = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(false);
    if !dead {
        return;
    }
    let Some((seeder, seeded, target_level)) = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|npc| {
            let state = (npc.seeder_object_id, npc.seeded);
            npc.template(world).map(|t| (state.0, state.1, t.level))
        })
    else {
        return;
    };
    if caster_oid != seeder {
        // TODO(manor): YOU_ARE_NOT_AUTHORIZED_TO_HARVEST — sm id not in repo data.
        return;
    }
    if !seeded {
        return;
    }
    if calc_harvest_success(player_level, target_level, world.roll(99)) {
        // `takeHarvest()` — read and clear the stashed crop.
        let harvest = world
            .objects
            .get_component_mut::<Npc>(&target_oid)
            .and_then(|npc| npc.harvest_item.take());
        if let Some((crop_id, count)) = harvest {
            grant_and_notify(world, caster_oid, &[(crop_id, count, 0)]);
        }
    }
}

/// `Harvesting.calcSuccess`: base 100 %, a 5 % penalty per level of gap beyond
/// 5, floored at 1 % (this one *is* clamped, unlike [`calc_sow_success`]).
fn calc_harvest_success(player_level: i32, target_level: i32, roll: i32) -> bool {
    let diff = (player_level - target_level).abs();
    let mut basic = 100;
    if diff > 5 {
        basic -= (diff - 5) * 5;
    }
    if basic < 1 {
        basic = 1;
    }
    roll < basic
}

/// `handlers/effecthandlers/ConsumeBody.java`: decay the swept corpse at once
/// (`Npc.endDecayTask` → `onDecay`). Paired after `Sweeper` on skill 42 so the
/// body vanishes immediately. Only a dead NPC (the resolved corpse target).
fn apply_consume_body(world: &mut World, _caster_oid: i32, target_oid: i32) {
    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    if world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| !v.dead)
        .unwrap_or(true)
    {
        return;
    }
    // `endDecayTask()` runs `onDecay` now; the corpse's originally-scheduled
    // `NpcDecay` task then becomes a no-op (the entity is already despawned).
    crate::game_loop::death::handle_npc_decay(world, target_oid);
}

/// `calcShldUse` applied to a **skill's** defence term (Java's
/// `PhysicalAttack`/`EnergyAttack`/`calcBlowDamage` all share this shape).
///
/// Returns `None` on a **perfect block**, which the callers turn into a flat
/// **1** damage — Java expresses it as `defence = -1` and then skips the whole
/// damage branch, or `return 1` in `calcBlowDamage`. Otherwise the (possibly
/// shield-augmented) defence.
///
/// The two rolls are consumed even when the target has no shield, matching
/// `calc_shield_use`'s own early return — it is the *rate* that is zero, not
/// the roll that is skipped.
pub(crate) fn defence_after_shield(
    world: &mut World,
    target_oid: i32,
    base_defence: f64,
    ignore_shield_defence: bool,
) -> Option<f64> {
    if ignore_shield_defence {
        return Some(base_defence);
    }
    let (shield_def, shield_rate, con_bonus) =
        crate::game_loop::combat::shield_stats(world, target_oid);
    let (rate_roll, perfect_roll) = (world.roll(100), world.roll(100));
    match formulas::calc_shield_use(
        shield_rate,
        con_bonus,
        false,
        false,
        rate_roll,
        perfect_roll,
    ) {
        formulas::SHIELD_PERFECT => None,
        formulas::SHIELD_SUCCEED => Some(base_defence + shield_def),
        _ => Some(base_defence),
    }
}

/// The target-side `mDef` for the magic damage formula — players through
/// their stat pipeline, NPCs through the `MDefenseFinalizer` shape
/// (base × MEN bonus × level mod).
fn target_p_def(world: &World, target_oid: i32) -> f64 {
    if let Some(cs) = world.objects.get_component::<CombatStats>(&target_oid) {
        return cs.p_def;
    }
    if let Some(p_def) = world
        .objects
        .get_component::<crate::model::door::Door>(&target_oid)
        .and_then(|d| world.data.door_data.get(d.door_id))
        .map(|t| (t.p_def as f64).max(1.0))
    {
        return p_def;
    }
    1.0
}

/// The trait term every damage formula multiplies in, as one call.
///
/// Java spells it out per handler as three separate factors —
/// `weaponTraitMod · (generalTraitMod == 0 ? 1 : generalTraitMod) · weaknessMod`
/// — and the **`== 0 ? 1`** guard is not decoration: an invulnerable trait
/// zeroes `calcGeneralTraitBonus`, and the damage formulas deliberately treat
/// that as "no modifier" rather than "no damage" (the landing roll is where
/// invulnerability actually bites). `physical` picks whether the weapon term
/// applies: the magic formulas (`calcMagicDam`) have no weapon trait at all.
pub(crate) fn skill_trait_mod(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    physical: bool,
) -> f64 {
    let weapon = if physical {
        calc_weapon_trait_bonus(world, caster_oid, target_oid)
    } else {
        1.0
    };
    let general = calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, true);
    let general = if general == 0.0 { 1.0 } else { general };
    weapon * general * calc_weakness_bonus(world, caster_oid, target_oid, skill.trait_type)
}

/// `Formulas.calcAttributeBonus(attacker, target, skill)` — the elemental
/// damage/land-rate multiplier (PLAN_G19_ATTRIBUTES.md). With a skill element
/// (Volcano FIRE 20): attacker's matching POWER stat + the skill's value vs
/// the target's matching RES. Without one, the attacker's **strongest POWER
/// stat elects the element** (Java `CreatureStat.getAttackElement`'s "temp
/// fix" scan) — which is how Holy Weapon colors an attribute-less skill holy.
/// Nothing elected (both sides bare) → 1.0.
pub(crate) fn attribute_mod(world: &World, caster_oid: i32, target_oid: i32, skill: &Skill) -> f64 {
    use crate::model::stats::Element;
    let (attack, element) = match skill.attribute_type {
        Some(el) => (
            element_stat(world, caster_oid, el, false) + skill.attribute_value as f64,
            el,
        ),
        None => {
            let mut best: Option<(Element, f64)> = None;
            for el in Element::ALL {
                let v = element_stat(world, caster_oid, el, false);
                if v > best.map_or(0.0, |(_, b)| b) {
                    best = Some((el, v));
                }
            }
            match best {
                Some((el, v)) => (v, el),
                None => return 1.0,
            }
        }
    };
    let defence = element_stat(world, target_oid, element, true);
    crate::model::formulas::calc_attribute_bonus(attack, defence)
}

/// One element stat (`*_POWER` / `*_RES`) read the `AttributeFinalizer` way:
/// template base (NPCs — players have none), then the merged modifiers.
/// Players read their rebuilt `StatModifiers`; NPCs keep none, so their
/// active buffs are folded on read (the abnormal-flags pattern) — which is
/// what lets Day of Doom's −50s bite a mob.
fn element_stat(
    world: &World,
    oid: i32,
    element: crate::model::stats::Element,
    defence: bool,
) -> f64 {
    let stat = if defence {
        element.res_stat()
    } else {
        element.power_stat()
    };
    let base = world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .and_then(|n| n.template(world))
        .map(|t| {
            if defence {
                t.base_element_res[element.index()] as f64
            } else {
                match t.base_attack_element {
                    Some((el, v)) if el == element => v as f64,
                    _ => 0.0,
                }
            }
        })
        .unwrap_or(0.0);
    if let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&oid)
    {
        return base * mods.mul.get(&stat).copied().unwrap_or(1.0)
            + mods.add.get(&stat).copied().unwrap_or(0.0);
    }
    // NPC: fold the active buffs' stat modifiers for this stat.
    let (mut add, mut mul) = (0.0, 1.0);
    if let Some(buffs) = world.objects.get_component::<Buffs>(&oid) {
        for b in &buffs.0 {
            for m in &b.effects {
                if m.stat == stat {
                    match m.mode {
                        crate::model::stats::StatModifierType::Diff => add += m.amount,
                        crate::model::stats::StatModifierType::Per => mul *= 1.0 + m.amount / 100.0,
                    }
                }
            }
        }
    }
    base * mul + add
}

fn target_m_def(world: &World, target_oid: i32) -> f64 {
    if let Some(cs) = world.objects.get_component::<CombatStats>(&target_oid) {
        // Players + NPCs: memoized at spawn through the MDefenseFinalizer shape.
        return cs.m_def;
    }
    // Siege doors carry no `CombatStats` — their mDef is a flat template value.
    if let Some(m_def) = world
        .objects
        .get_component::<crate::model::door::Door>(&target_oid)
        .and_then(|d| world.data.door_data.get(d.door_id))
        .map(|t| (t.m_def as f64).max(1.0))
    {
        return m_def;
    }
    1.0
}

/// `Player.sendDamageMessage`'s crit line: magic skills show `M_CRITICAL`,
/// physical skills `C1_LANDED_A_CRITICAL_HIT` (named after the attacker).
fn crit_message(is_magic: bool, caster_name: &str) -> Vec<u8> {
    use server_packets::{SmParam, sm_ids};
    if is_magic {
        server_packets::system_message_with(sm_ids::M_CRITICAL, &[])
    } else {
        server_packets::system_message_with(
            sm_ids::C1_LANDED_A_CRITICAL_HIT,
            &[SmParam::PlayerName(caster_name.to_string())],
        )
    }
}

/// Port of `Creature.doAttack` → `reduceCurrentHp` for instant skill damage
/// (magic and physical): the caster-side messages here, the victim-side
/// application (CP soak, death, NPC hate/AI wake) shared with the auto-attack
/// path in `combat::apply_physical_damage`'s per-kind receivers. `is_magic`
/// picks the crit line (`Player.sendDamageMessage`: `M_CRITICAL` for magic,
/// `C1_LANDED_A_CRITICAL_HIT` for physical skills).
/// `Formulas.calcCounterAttack` — Shield of Revenge (439) and Counterattack
/// (447), whose `CounterPhysicalSkill` effect grants a **chance** (20 % / 90 %),
/// not a multiplier.
///
/// Two guards decide whether it can fire at all, and both are easy to drop:
/// **only melee skills are counterable** (`skill.isMagic() ||
/// skill.getCastRange() > 40` bails), and the counter is skipped for a dead
/// target and for DoT ticks. The counter damage itself is
/// `target.pAtk * 873 / attacker.pDef`, scaled by the weapon/general trait and
/// attribute bonuses.
fn calc_counter_attack(
    world: &mut World,
    attacker_oid: i32,
    target_oid: i32,
    skill_id: i32,
    is_dot: bool,
) {
    /// Java `Formulas.MELEE_ATTACK_RANGE`.
    const MELEE_ATTACK_RANGE: i32 = 40;
    if is_dot {
        return;
    }
    let Some(skill) = world.data.skill_data.get(skill_id, 1).cloned() else {
        return;
    };
    if skill.magic_type == 1 || skill.cast_range > MELEE_ATTACK_RANGE {
        return;
    }
    if world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    let chance = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&target_oid)
        .and_then(|m| {
            m.add
                .get(&crate::model::stats::Stat::VengeanceSkillPhysicalDamage)
                .copied()
        })
        .unwrap_or(0.0);
    if chance <= 0.0 || (world.roll(100) as f64) >= chance {
        return;
    }
    let (target_p_atk, attacker_p_def) = (
        world
            .objects
            .get_component::<CombatStats>(&target_oid)
            .map(|c| c.p_atk)
            .unwrap_or(0.0),
        world
            .objects
            .get_component::<CombatStats>(&attacker_oid)
            .map(|c| c.p_def)
            .unwrap_or(0.0)
            .max(1.0),
    );
    let counter = (target_p_atk * 873.0 / attacker_p_def)
        * skill_trait_mod(world, target_oid, attacker_oid, &skill, true)
        * attribute_mod(world, target_oid, attacker_oid, &skill);
    if counter <= 0.0 {
        return;
    }
    let (attacker_name, target_name) = (
        creature_name(world, attacker_oid),
        creature_name(world, target_oid),
    );
    if let Some(cid) = client_for_player(world, target_oid)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(server_packets::system_message_with(
            server_packets::sm_ids::YOU_COUNTERED_C1_S_ATTACK,
            &[server_packets::SmParam::Text(attacker_name)],
        ));
    }
    if let Some(cid) = client_for_player(world, attacker_oid)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(server_packets::system_message_with(
            server_packets::sm_ids::C1_IS_PERFORMING_A_COUNTERATTACK,
            &[server_packets::SmParam::Text(target_name)],
        ));
    }
    crate::game_loop::combat::apply_physical_damage(
        world,
        target_oid,
        attacker_oid,
        counter,
        false,
        true,
    );
}

pub(crate) fn apply_skill_damage(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    damage: f64,
    crit: bool,
    is_magic: bool,
    caster_name: &str,
    // Java `AttackableStatus.reduceHp` consults the skill's `<overHit>` here.
    // Passed explicitly rather than re-read, because the damage value this
    // needs only exists at the call site.
    over_hit: bool,
    // `CreatureStatus.reduceHp`'s `isDOT` — a DoT tick (and only a DoT tick)
    // still applies through `HP_BLOCK` (`isHpBlocked() && !(isDOT || …)`).
    // Every instant-effect call site passes `false`; only
    // `handle_dam_over_time_tick` passes `true`.
    is_dot: bool,
    // The skill driving this hit, surfaced to quest `onAttack` handlers so they
    // can distinguish a skill from a melee swing (Java's `onAttack(..., Skill)`).
    skill_id: i32,
) {
    record_overhit(world, caster_oid, target_oid, damage, over_hit);
    use server_packets::{SmParam, sm_ids};

    // `Formulas.calcCounterAttack`, which Java runs from `reduceCurrentHp`
    // *before* the damage lands ("Counterattacks happen before damage
    // received") whenever a skill is involved (G34 S4).
    calc_counter_attack(world, caster_oid, target_oid, skill_id, is_dot);

    // A siege door: route the hit straight to the gate's HP (no CP/hate/AI
    // receivers) and refresh its HP bar, then report the damage to the caster.
    if world
        .objects
        .has_component::<crate::model::door::Door>(&target_oid)
    {
        let door_name = world
            .objects
            .get_component::<crate::model::door::Door>(&target_oid)
            .and_then(|d| world.data.door_data.get(d.door_id))
            .map(|t| t.name.clone())
            .unwrap_or_default();
        if let Some(client_id) = client_for_player(world, caster_oid)
            && let Some(cs) = world.clients.get(&client_id)
        {
            if crit {
                cs.send(crit_message(is_magic, caster_name));
            }
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
                &[
                    SmParam::PlayerName(caster_name.to_string()),
                    SmParam::Text(door_name),
                    SmParam::Int(damage as i32),
                ],
            ));
        }
        crate::game_loop::combat::apply_door_damage(world, target_oid, damage as i32);
        return;
    }

    let target_param = if let Some(p) = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
    {
        SmParam::PlayerName(p.name.clone())
    } else if let Some(t) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| n.template(world))
    {
        SmParam::NpcName(t.id)
    } else {
        return;
    };
    let dmg_int = damage as i32;

    if let Some(client_id) = client_for_player(world, caster_oid)
        && let Some(cs) = world.clients.get(&client_id)
    {
        if crit {
            cs.send(crit_message(is_magic, caster_name));
        }
        cs.send(server_packets::system_message_with(
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
            &[
                SmParam::PlayerName(caster_name.to_string()),
                target_param,
                SmParam::Int(dmg_int),
                // `sendDamageMessage`'s `addPopup(target, attacker, -damage)`
                // — the on-screen floating damage number over the target.
                SmParam::Popup {
                    target: target_oid,
                    attacker: caster_oid,
                    damage: -dmg_int,
                },
            ],
        ));
    }

    // Victim-side application: CP soak/HP/death/cast-break for players
    // (including the C1_HAS_RECEIVED message), hate + AI wake + death for
    // NPCs — the same receivers the auto-attack hits go through. The skill id
    // rides on the world for the duration of the hit so quest `onAttack` can
    // read it (Java threads `Skill` straight into the notification).
    world.quest_attack_skill = Some(skill_id);
    crate::game_loop::combat::apply_attack_damage(
        world,
        caster_oid,
        target_oid,
        damage,
        is_dot,
        Some(is_magic),
    );
    world.quest_attack_skill = None;
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
    refresh_summon_info(world, target_oid);
}

/// A **summon** whose stats just changed has to tell the client, or a buff the
/// player cast deliberately appears to do nothing.
///
/// A generic mob doesn't get this: the port never re-broadcasts `NpcInfo` on a
/// buff, so a buffed mob's speed change only shows after respawn. That is
/// tolerable for a mob nobody is watching closely and wrong for a servitor —
/// Servitor Haste (attack speed) and Servitor Wind Walk (movement speed) both
/// land in fields `PetInfo`/`SummonInfo` carry, and both are cast by the owner
/// *expecting* to see the difference.
fn refresh_summon_info(world: &mut World, target_oid: i32) {
    let Some(owner) = world
        .objects
        .get_component::<crate::model::components::ServitorOf>(&target_oid)
        .map(|s| s.owner_object_id)
    else {
        return;
    };
    crate::game_loop::servitor::send_pet_info(
        world,
        owner,
        target_oid,
        crate::game_loop::servitor::PetInfoKind::Default,
    );
    crate::game_loop::servitor::broadcast_summon_info(world, target_oid, false);
}

/// Push a creature's current buffs to every player who has it targeted (Java
/// `EffectList.updateEffectIcons` → `ExAbnormalStatusUpdateFromTarget` to the
/// status listeners) — this is what draws the buff icons under a target's HP
/// bar. Used for NPC targets; players get their own self bar separately.
pub(crate) fn broadcast_target_buffs(world: &mut World, target_oid: i32) {
    let now = world.tick;
    let pkt = match world.objects.get_component::<Buffs>(&target_oid) {
        Some(buffs) => crate::network::enter_world::ex_abnormal_status_update_from_target(
            target_oid, buffs, now,
        ),
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
        if let Some(cid) = client_for_player(world, oid)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(pkt.clone());
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
    let Some(t) = world.data.npc_data.get(npc_id) else {
        return;
    };
    // Read the champion flag out before the multi-borrow below: a champion's
    // recomputed stats must keep their multipliers, or the first buff cast on
    // one would quietly strip them back to the ordinary template values.
    let champion_mods = crate::model::ChampionStatMods::of(
        &world.cfg.champion,
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&target_oid)
            .is_some_and(|n| n.champion),
    );
    if let Some((buffs, mut combat, mut speeds, mut vitals)) = world.objects.get_many_mut::<(
        &Buffs,
        &mut CombatStats,
        &mut Speeds,
        &mut crate::model::components::Vitals,
    )>(&target_oid)
    {
        crate::model::recompute_npc_stats_from_buffs(
            &world.data,
            t,
            buffs,
            champion_mods,
            &mut combat,
            &mut speeds,
            &mut vitals,
        );
    }
}

/// Recompute a player's max HP/MP/CP from base + CON/MEN + gear + the current
/// buff modifier maps — Java's `Max{Hp,Mp,Cp}Finalizer`, which run inside the
/// same `recalculateStats`. The player's `recalculate_stats` only covers
/// combat/speed stats, so this must be called alongside any buff apply/remove
/// (clan skills, Clan Advent, GM buffs, …) or the HP/MP/CP stat modifiers those
/// carry never move the bar. Current values are only clamped *down* (Java
/// doesn't heal on a max increase). Callers already broadcast UserInfo.
pub(crate) fn recompute_max_vitals(world: &mut World, oid: i32) {
    use crate::model::components::{PlayerVitals, StatModifiers, Vitals};
    use crate::model::inventory::Inventory;
    let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) else {
        return;
    };
    let (level, class_id, base_class_id) = (p.level, p.class_id, p.base_class_id);
    let t = world
        .data
        .player_templates
        .get(class_id)
        .or_else(|| world.data.player_templates.get(base_class_id))
        .cloned()
        .unwrap_or_default();
    let (max_hp, max_mp, max_cp) = {
        let Some(mods) = world.objects.get_component::<StatModifiers>(&oid) else {
            return;
        };
        let inv = world.objects.get_component::<Inventory>(&oid);
        (
            crate::model::calc_max_hp(&world.data, &t, level, inv, mods),
            crate::model::calc_max_mp(&world.data, &t, level, inv, mods),
            crate::model::calc_max_cp(&world.data, &t, level, mods),
        )
    };
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
        v.max_hp = max_hp as i32;
        v.max_mp = max_mp as i32;
        if v.cur_hp > max_hp {
            v.cur_hp = max_hp;
        }
        if v.cur_mp > max_mp {
            v.cur_mp = max_mp;
        }
    }
    if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&oid) {
        pv.max_cp = max_cp as i32;
        if pv.cur_cp > max_cp {
            pv.cur_cp = max_cp;
        }
    }
}

/// Java `Config.EFFECT_TICK_RATIO` (character.ini `EffectTickRatio`, default
/// 666 ms) — the base period of an over-time effect's tick. Not yet a Rust
/// config knob; the datapack assumes the retail default.
const EFFECT_TICK_RATIO_MS: u64 = 666;

/// `effect.getTicks() * EFFECT_TICK_RATIO` expressed in whole game ticks
/// (`game_loop::TICK` = 100 ms): both the delay to the first DoT tick and the
/// interval between ticks (Java `scheduleAtFixedRate(task, period, period)`).
/// `0` when `ticks <= 0`, which suppresses scheduling.
fn dot_interval_ticks(ticks: i32) -> u64 {
    if ticks <= 0 {
        return 0;
    }
    (ticks as u64 * EFFECT_TICK_RATIO_MS) / crate::game_loop::TICK.as_millis() as u64
}

/// Damage per DoT tick: `power * getTicksMultiplier()`, where
/// `getTicksMultiplier() = ticks * EFFECT_TICK_RATIO / 1000`
/// (`AbstractEffect`). Curse Poison lvl 1 (power 11, ticks 5) → `11 * 5 * 666 /
/// 1000 ≈ 36.6` every `5 * 666 = 3330 ms`.
fn dot_tick_damage(power: f64, ticks: i32) -> f64 {
    power * (ticks as f64 * EFFECT_TICK_RATIO_MS as f64) / 1000.0
}

/// Arm the first `DamOverTimeTick` for a skill carrying a `DamOverTime` effect
/// (Java `BuffInfo.scheduleEffects`). One recurring task per skill drives all
/// its DoT effects; the cadence comes from the first such effect (Interlude
/// poison/bleed skills carry exactly one). A no-op for skills without a DoT.
fn schedule_dam_over_time(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    let interval = skill
        .effects
        .iter()
        .find_map(|e| match e {
            SkillEffect::DamOverTime { ticks, .. }
            | SkillEffect::HealOverTime { ticks, .. }
            | SkillEffect::ManaDamOverTime { ticks, .. }
            | SkillEffect::MpConsumePerLevel { ticks, .. }
            | SkillEffect::Relax { ticks, .. }
            | SkillEffect::ChameleonRest { ticks, .. }
            | SkillEffect::ManaHealOverTime { ticks, .. }
            | SkillEffect::Fear { ticks }
            | SkillEffect::FakeDeath { ticks, .. }
                if *ticks > 0 =>
            {
                Some(dot_interval_ticks(*ticks))
            }
            _ => None,
        })
        .unwrap_or(0);
    if interval == 0 {
        return;
    }
    world.scheduler.schedule(
        world.tick + interval,
        ScheduledTask::DamOverTimeTick {
            caster: caster_oid,
            target: target_oid,
            skill_id: skill.id,
            skill_level: skill.level,
        },
    );
}

/// Push a periodic tick's HP/MP change to the owner and their party — the
/// `broadcastStatusUpdate(effector)` every `onActionTime` ends with.
pub(crate) fn broadcast_vitals_for(world: &World, target_oid: i32) {
    broadcast_vitals(world, target_oid);
}

fn broadcast_vitals(world: &World, target_oid: i32) {
    if let Some(client_id) = client_for_player(world, target_oid)
        && let Some((v, cs)) = world
            .objects
            .get_component::<Vitals>(&target_oid)
            .copied()
            .zip(world.clients.get(&client_id))
    {
        cs.send(server_packets::status_update(
            target_oid,
            &[
                (server_packets::status_update_type::CUR_HP, v.cur_hp as i32),
                (server_packets::status_update_type::CUR_MP, v.cur_mp as i32),
            ],
        ));
    }
    crate::game_loop::party::notify_party_vitals(world, target_oid);
}

/// `DamOverTime.onActionTime` — one poison/bleed tick. Deals
/// `power * getTicksMultiplier()` from `caster` to `target` for each of the
/// skill's DoT effects, then reschedules itself. The chain stops (Java's
/// fixed-rate task cancelled by `BuffFinishTask`) when the buff is no longer
/// present — its `BuffExpire` removes it at `abnormalTime` — or the target is
/// dead. `can_kill == false` clamps each tick to leave the target at 1 HP
/// (Java: "Fix for players dying by DOTs"). A non-toggle DoT never
/// self-cancels on the tick's own return value (`BuffInfo.onTick` only cancels
/// toggles), so the reschedule is unconditional while the buff lives.
pub(crate) fn handle_dam_over_time_tick(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill_id: i32,
    skill_level: i32,
) {
    // Buff gone (expired / removed / dispelled) → end the tick chain.
    let buff_present = world
        .objects
        .get_component::<Buffs>(&target_oid)
        .is_some_and(|b| b.0.iter().any(|entry| entry.skill_id == skill_id));
    if !buff_present {
        return;
    }
    // Dead target → stop (Java `onActionTime`: `isDead()` bails).
    if world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    let Some(skill) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
        return;
    };
    // Effector name for the damage message (`Player.sendDamageMessage`); empty
    // for an NPC effector (no client to message — the base no-op).
    let caster_name = world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .map(|p| p.name.clone())
        .unwrap_or_default();

    let mut interval = 0;
    // Set when a tick returns Java's `false` for a *toggle*, which cancels it
    // (`BuffInfo.onTick` only honours the return value for toggles).
    let mut deactivate_toggle = false;
    let is_toggle = skill.operate_type == crate::model::skill::OperateType::Toggle;

    for effect in &skill.effects {
        match effect {
            // `HealOverTime.onActionTime`. `power` is negative for the upkeep
            // toggles, so this both heals and drains.
            SkillEffect::HealOverTime { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else { continue };
                let max_hp = v.max_hp as f64;
                // Java's early bails: at full HP a healing tick is skipped, and
                // a draining one is skipped when it would take the target to 0.
                // (With a negative power the second test is `hp + |power| <= 0`,
                // which never fires — ported as written rather than "fixed".)
                if *power > 0.0 {
                    if v.cur_hp >= max_hp {
                        deactivate_toggle |= is_toggle;
                        continue;
                    }
                } else if v.cur_hp - *power <= 0.0 {
                    deactivate_toggle |= is_toggle;
                    continue;
                }
                let mut hp = v.cur_hp + dot_tick_damage(*power, *ticks);
                // Cap at max when healing, floor at 1 when draining — a HoT
                // upkeep never kills its owner.
                hp = if *power > 0.0 { hp.min(max_hp) } else { hp.max(1.0) };
                if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                    vit.cur_hp = hp;
                }
                broadcast_vitals(world, target_oid);
            }
            // `ManaDamOverTime.onActionTime` — MP upkeep. Shares this arm with
            // `MpConsumePerLevel` (the fighter-toggle upkeep effect): Java's
            // formula for the latter is `power * getTicksMultiplier()` whenever
            // the skill has no `abnormalTime`, which is every instance in this
            // datapack (all 19 are toggles/`AU` skills), so the two are
            // computed identically here. Split them out if a skill ever pairs
            // `MpConsumePerLevel` with an `abnormalTime` (the level-scaled
            // `((level-1)/7.5) * base * abnormalTime` branch) — no skill in
            // this datapack does, so that branch is unreachable rather than
            // pending.
            // `Fear.onActionTime` — keep running. Java passes `null` for the
            // effector here (not the caster it had at `onStart`), so every
            // repeat steers by the victim's current heading: they keep going
            // the way the first shove threw them instead of being re-aimed
            // away from a caster who may be dead, gone or long out of range.
            SkillEffect::Fear { ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                fear_action(world, None, target_oid);
            }
            // `ChameleonRest.onActionTime` — Relax's stand-up and out-of-MP
            // stops, **without** its HP-full stop: you are resting to hide,
            // not to heal, so a full HP bar does not retire it.
            SkillEffect::ChameleonRest { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                if world.objects.has_component::<crate::model::Player>(&target_oid)
                    && !crate::game_loop::sit_stand::is_sitting(world, target_oid)
                {
                    deactivate_toggle |= is_toggle;
                    continue;
                }
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
                    continue;
                };
                let drain = dot_tick_damage(*power, *ticks);
                // Java compares before spending and bails on `>`, so a tick that
                // costs exactly the remaining MP still runs.
                if drain > v.cur_mp {
                    if let Some(client_id) = client_for_player(world, target_oid)
                        && let Some(cs) = world.clients.get(&client_id)
                    {
                        cs.send(server_packets::system_message_with(
                            server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP,
                            &[],
                        ));
                    }
                    deactivate_toggle = true;
                    continue;
                }
                if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                    vit.cur_mp = (vit.cur_mp - drain).max(0.0);
                }
                broadcast_vitals(world, target_oid);
            }
            // `ManaHealOverTime.onActionTime` — the mirror of the drain arm
            // below. Java's two early-outs are asymmetric: a **positive** power
            // stops once MP is already full, a negative one stops when the tick
            // would take MP to zero or below, and the write floors at 1 rather
            // than 0 — a drain wearing this handler can never empty the pool.
            SkillEffect::ManaHealOverTime { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
                    continue;
                };
                if v.dead {
                    continue;
                }
                // `getMaxRecoverableMp()` — `MAX_RECOVERABLE_MP` over `maxMp`.
                // `LimitMp`'s two carriers are unreachable here (see
                // `restore_mp`), so the ceiling is plain `maxMp`.
                let ceiling = v.max_mp as f64;
                if *power > 0.0 {
                    if v.cur_mp >= ceiling {
                        continue;
                    }
                } else if v.cur_mp - *power <= 0.0 {
                    continue;
                }
                let delta = dot_tick_damage(*power, *ticks);
                let restored = if *power > 0.0 {
                    (v.cur_mp + delta).min(ceiling)
                } else {
                    (v.cur_mp + delta).max(1.0)
                };
                if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                    vit.cur_mp = restored;
                }
                broadcast_vitals(world, target_oid);
            }
            // `Relax.onActionTime` — the MP upkeep above, plus the two extra
            // stop conditions the plain upkeep effects do not have.
            SkillEffect::Relax { power, ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                // "the holder stood up" — Java returns `false` outright, which
                // cancels the toggle. Standing is how a player turns Relax off.
                if world.objects.has_component::<crate::model::Player>(&target_oid)
                    && !crate::game_loop::sit_stand::is_sitting(world, target_oid)
                {
                    deactivate_toggle |= is_toggle;
                    continue;
                }
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
                    continue;
                };
                // Java's `(curHp + 1) > maxRecoverableHp`: the point of Relax is
                // to regenerate, so it retires itself once there is nothing left
                // to heal — with its own message, distinct from running dry.
                if v.cur_hp + 1.0 > v.max_hp as f64 && is_toggle {
                    if let Some(client_id) = client_for_player(world, target_oid)
                        && let Some(cs) = world.clients.get(&client_id)
                    {
                        cs.send(server_packets::system_message_with(
                            server_packets::sm_ids::THAT_SKILL_HAS_BEEN_DE_ACTIVATED_AS_HP_WAS_FULLY_RECOVERED,
                            &[],
                        ));
                    }
                    deactivate_toggle = true;
                    continue;
                }
                let drain = dot_tick_damage(*power, *ticks);
                if drain > v.cur_mp && is_toggle {
                    if let Some(client_id) = client_for_player(world, target_oid)
                        && let Some(cs) = world.clients.get(&client_id)
                    {
                        cs.send(server_packets::system_message_with(
                            server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP,
                            &[],
                        ));
                    }
                    deactivate_toggle = true;
                    continue;
                }
                if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                    vit.cur_mp = (vit.cur_mp - drain).max(0.0);
                }
                broadcast_vitals(world, target_oid);
            }
            SkillEffect::ManaDamOverTime { power, ticks }
            | SkillEffect::MpConsumePerLevel { power, ticks }
            // `FakeDeath.onActionTime` is the same `power * getTicksMultiplier()`
            // MP drain, with the same toggle self-deactivate on empty MP.
            | SkillEffect::FakeDeath { power, ticks }
                if *ticks > 0 =>
            {
                interval = dot_interval_ticks(*ticks);
                let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else { continue };
                let drain = dot_tick_damage(*power, *ticks);
                if drain > v.cur_mp && is_toggle {
                    // Out of MP: the toggle switches itself off.
                    if let Some(client_id) = client_for_player(world, target_oid)
                        && let Some(cs) = world.clients.get(&client_id) {
                            cs.send(server_packets::system_message_with(
                                server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP,
                                &[],
                            ));
                        }
                    deactivate_toggle = true;
                    continue;
                }
                if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                    vit.cur_mp = (vit.cur_mp - drain).max(0.0);
                }
                broadcast_vitals(world, target_oid);
            }
            _ => {}
        }

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
        interval = dot_interval_ticks(*ticks);
        let mut damage = dot_tick_damage(*power, *ticks);
        // `!canKill`: a tick may never drop the target below 1 HP.
        if !*can_kill {
            let cur_hp = world
                .objects
                .get_component::<Vitals>(&target_oid)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0);
            if cur_hp <= 1.0 {
                continue;
            }
            if damage >= cur_hp - 1.0 {
                damage = cur_hp - 1.0;
            }
        }
        if damage > 0.0 {
            // Java `effector.doAttack(damage, effected, skill, isDOT=true, …,
            // critical=false, …)`: no crit line; reuses the shared victim-side
            // path (CP soak / NPC hate / AI wake / death).
            apply_skill_damage(
                world,
                caster_oid,
                target_oid,
                damage,
                false,
                skill.magic_type == 1,
                &caster_name,
                false,
                true,
                skill.id,
            );
            // A `canKill` tick can kill outright — stop then.
            if world
                .objects
                .get_component::<Vitals>(&target_oid)
                .is_none_or(|v| v.dead)
            {
                return;
            }
        }
    }
    if deactivate_toggle {
        // Java's `false` return cancels a toggle's effect outright; the tick
        // chain then ends with the buff.
        handle_buff_expire(world, target_oid, skill_id);
        return;
    }
    if interval > 0 {
        world.scheduler.schedule(
            world.tick + interval,
            ScheduledTask::DamOverTimeTick {
                caster: caster_oid,
                target: target_oid,
                skill_id,
                skill_level,
            },
        );
    }
}

/// `BuffFinishTask`, fired when a buff's `abnormalTime` elapses
/// (`ScheduledTask::BuffExpire`). A buff already gone (re-cast/replaced) is a
/// no-op, matching the scheduler's dead-id contract.
/// Java `EffectList.remove` — take the buff off and run everything that hangs
/// off its removal, ending with `applyEffectScope(EffectScope.END, …)`.
///
/// The END scope is applied *here* rather than inside the removal body because
/// that body has several early exits (the NPC path returns before the player
/// broadcasts); hanging the end-effects off the wrapper means every removal
/// route fires them exactly once, which is what Java's single call site does.
pub(crate) fn handle_buff_expire(world: &mut World, player_object_id: i32, skill_id: i32) {
    // Read before the removal — the buff has to still be there to know whether
    // this call is the one that actually took it off.
    let was_active = world
        .objects
        .get_component::<Buffs>(&player_object_id)
        .is_some_and(|b| b.0.iter().any(|b| b.skill_id == skill_id));
    let end_effects = world
        .data
        .skill_data
        .get(skill_id, 1)
        .map(|s| s.end_effects.clone())
        .unwrap_or_default();

    handle_buff_expire_inner(world, player_object_id, skill_id);

    // Anchor (1170) is the learnable carrier: its first stage holds the body
    // rigid and this fires skill 6091 for the paralysis its own description
    // promises. Applied after the removal, so a called skill that re-buffs the
    // same target cannot race it.
    if was_active && !end_effects.is_empty() {
        let called = Skill {
            effects: end_effects,
            ..world
                .data
                .skill_data
                .get(skill_id, 1)
                .cloned()
                .unwrap_or_default()
        };
        apply_skill_effects(world, player_object_id, player_object_id, &called);
    }
}

fn handle_buff_expire_inner(world: &mut World, player_object_id: i32, skill_id: i32) {
    // Forced/unconditional removal — also used by dispel/cure, which strip a
    // buff before its timer. The natural-timeout path gates on `expires_at_tick`
    // at the scheduler dispatch so a stale `BuffExpire` from a re-cast can't drop
    // the refreshed buff early.
    let still_active = world
        .objects
        .get_component::<Buffs>(&player_object_id)
        .is_some_and(|b| b.0.iter().any(|b| b.skill_id == skill_id));
    if !still_active {
        return;
    }
    // `ResurrectionSpecial.onExit` — the auto-resurrect. The buff does nothing
    // while it is up; what fires it is being *stripped*, which is what death
    // does. Java refuses in olympiad and outside the effect's `instanceId`
    // list; neither is modelled for this path (no carrier on this dist
    // declares `instanceId`). TODO(G34): add the olympiad gate.
    if let Some(res) = world.data.skill_data.get(skill_id, 1).and_then(|s| {
        s.effects.iter().find_map(|e| match e {
            SkillEffect::ResurrectionSpecial {
                power,
                hp_percent,
                mp_percent,
                cp_percent,
            } => Some((*power, *hp_percent, *mp_percent, *cp_percent)),
            _ => None,
        })
    }) {
        let (power, hp, mp, cp) = res;
        // Java's effector is the caster; these are self-buffs, so the bearer
        // proposes their own revive.
        crate::game_loop::death::revive_request(
            world,
            player_object_id,
            player_object_id,
            power,
            hp,
            mp,
            cp,
            skill_id,
            0, // no affectRange bypass — this is a self-revive, not a mass one
        );
    }
    // `SkillEvasion.onExit` — `removeSkillEvasionTypeValue(magicType, amount)`.
    // Merged onto a per-bucket map rather than a `Stat`, so it needs its own
    // unmerge; without it Ultimate Evasion's 40 % dodge would be permanent.
    if let Some(evasions) = world.data.skill_data.get(skill_id, 1).map(|s| {
        s.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::SkillEvasion { magic_type, amount } => Some((*magic_type, *amount)),
                _ => None,
            })
            .collect::<Vec<_>>()
    }) && !evasions.is_empty()
        && let Some(mods) = world
            .objects
            .get_component_mut::<crate::model::components::StatModifiers>(&player_object_id)
    {
        for (magic_type, amount) in evasions {
            let entry = mods.skill_evasion.entry(magic_type).or_insert(0.0);
            *entry = (*entry - amount).max(0.0);
        }
    }
    // `TargetMe.onExit` — `setLockedTarget(null)`. The lock is what stops the
    // victim clicking a different NPC ("Failed to change enmity"), so it must
    // go the moment the taunt does (G34 S4).
    if world
        .data
        .skill_data
        .get(skill_id, 1)
        .is_some_and(|s| s.effects.iter().any(|e| matches!(e, SkillEffect::TargetMe)))
    {
        world
            .objects
            .remove_component::<crate::model::components::LockedTarget>(&player_object_id);
    }
    // `DefenceTrait.onExit` — unmerge before the buff row goes, while the skill
    // is still resolvable. Covers the NPC branch below as well as the player
    // one, and every removal route (timeout, dispel, death) funnels here.
    if let Some(effects) = world
        .data
        .skill_data
        .get(skill_id, buff_level(world, player_object_id, skill_id))
        .map(|s| s.effects.clone())
    {
        for effect in &effects {
            match effect {
                SkillEffect::DefenceTrait { traits } => {
                    remove_defence_traits(world, player_object_id, traits)
                }
                SkillEffect::AttackTrait { traits } => {
                    remove_attack_traits(world, player_object_id, traits)
                }
                _ => {}
            }
        }
    }
    // `MagicMpCost.onExit` / `Reuse.onExit`.
    if let Some(skill) = world
        .data
        .skill_data
        .get(skill_id, buff_level(world, player_object_id, skill_id))
        .cloned()
    {
        remove_skill_rates(world, player_object_id, &skill);
    }
    // Did the buff about to go carry a visual? If not, the set can't change and
    // no `ExUserInfoAbnormalVisualEffect` is due (Java's same rule).
    let had_visuals = world
        .objects
        .get_component::<Buffs>(&player_object_id)
        .is_some_and(|b| {
            b.0.iter()
                .any(|x| x.skill_id == skill_id && !x.abnormal_visuals.is_empty())
        });
    // NPC: drop the buff and recompute from the template (no icons/broadcast).
    if crate::game_loop::combat::is_npc_oid(player_object_id) {
        // `Fear.onExit`: `if (!effected.isPlayer()) notifyEvent(EVT_THINK)` —
        // a mob left mid-flight is still on `MOVE_TO`, whose think arm does
        // nothing, so without this it would keep walking out its last leg
        // before ever re-engaging. Reading the flag *before* the buff is
        // dropped is what makes this specific to fear rather than to any
        // expiring NPC buff.
        let was_afraid = world
            .objects
            .get_component::<Buffs>(&player_object_id)
            .is_some_and(|b| {
                b.0.iter().any(|x| {
                    x.skill_id == skill_id
                        && x.effect_flags & crate::model::skill::effect_flag::FEAR != 0
                })
            });
        if let Some(b) = world.objects.get_component_mut::<Buffs>(&player_object_id) {
            b.0.retain(|x| x.skill_id != skill_id);
        }
        if was_afraid
            && let Some(ai) = world
                .objects
                .get_component_mut::<crate::model::npc::NpcAi>(&player_object_id)
            && ai.intention == crate::model::npc::NpcIntention::MoveTo
        {
            ai.intention = crate::model::npc::NpcIntention::Active;
        }
        recompute_npc_buffed_stats(world, player_object_id);
        broadcast_target_buffs(world, player_object_id);
        // The expiry has to reach the client too, or the summon keeps showing
        // the buffed speed after the buff is gone.
        refresh_summon_info(world, player_object_id);
        return;
    }
    // `Transformation` buffs carry no stat modifier — `remove_buff` below is a
    // no-op for them — so the revert lives here: drop the display id/collision/
    // granted skills before the generic removal, and defer the extra self
    // packets (AVE + SkillList) to piggyback on the `broadcast_user_info` call
    // a few lines down rather than sending a second `UserInfo`.
    let skill_level = world
        .objects
        .get_component::<Buffs>(&player_object_id)
        .and_then(|b| {
            b.0.iter()
                .find(|x| x.skill_id == skill_id)
                .map(|x| x.skill_level)
        });
    let is_transform = skill_level.is_some_and(|lvl| {
        world.data.skill_data.get(skill_id, lvl).is_some_and(|s| {
            s.effects
                .iter()
                .any(|e| matches!(e, SkillEffect::Transform { .. }))
        })
    });
    if is_transform {
        crate::game_loop::admin::transforms::remove_transform_state(world, player_object_id);
    }
    // `FakeDeath.onExit` — stand back up. Read the flag off the *expiring buff*
    // (not the skill template) so this fires only for fake death, and keeps
    // working for a buff whose skill row is no longer loadable — the same
    // source `Fear`'s own `onExit` and `break_fake_death_on_damage` use.
    let was_fake_dead = world
        .objects
        .get_component::<Buffs>(&player_object_id)
        .is_some_and(|b| {
            b.0.iter().any(|x| {
                x.skill_id == skill_id
                    && x.effect_flags & crate::model::skill::effect_flag::FAKE_DEATH != 0
            })
        });
    if was_fake_dead {
        stop_fake_death(world, player_object_id);
    }
    if let Some((player, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &mut crate::model::Player,
            &BaseStats,
            &mut StatModifiers,
            &crate::model::inventory::Inventory,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&player_object_id)
    {
        player.remove_buff(
            &world.data,
            base,
            &mut mods,
            inventory,
            &mut buffs,
            &mut speeds,
            &mut combat,
            skill_id,
        );
    }
    // Reverting a MaxHp/MaxMp/MaxCp buff shrinks the bar (and clamps current).
    recompute_max_vitals(world, player_object_id);
    let now = world.tick;
    // Removing the buff reverted its stat contribution — rebroadcast so the
    // client (and nearby players, for speed) see the stats return to normal.
    crate::game_loop::party::broadcast_user_info(world, player_object_id);
    if is_transform {
        crate::game_loop::admin::transforms::refresh_transform_visuals(world, player_object_id);
    }
    if had_visuals {
        refresh_abnormal_visuals(world, player_object_id);
    }
    let Some(client_id) = client_for_player(world, player_object_id) else {
        return;
    };
    if let Some(buffs) = world.objects.get_component::<Buffs>(&player_object_id)
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(crate::network::enter_world::abnormal_status_update(
            buffs, now,
        ));
    }
}

/// The caster's name for the damage system messages. NPCs cast skills as of
/// G21, so this can't `expect` a `Player` — a monster resolves to its template
/// name. These strings only ever reach the *caster's own* client, which an NPC
/// doesn't have, so the value is cosmetic for the NPC path; the helper exists
/// so the shared effect code stops panicking on a non-player caster.
fn caster_display_name(world: &World, oid: i32) -> String {
    if let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) {
        return p.name.clone();
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .and_then(|n| n.template(world))
        .map(|t| t.name.clone())
        .unwrap_or_default()
}

/// The caster's level for `levelMod` in the physical-skill damage formula
/// (Java reads `Creature.getLevel()`, which both players and NPCs implement).
fn caster_level(world: &World, oid: i32) -> i32 {
    if let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) {
        return p.level;
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .and_then(|n| n.template(world))
        .map(|t| t.level)
        .unwrap_or(1)
}

/// Java `Formulas.calcGeneralTraitBonus(attacker, target, traitType, false)` —
/// how much a debuff's landing chance is scaled by the target's resistance to
/// its trait. **The clause order is Java's and is load-bearing:** `NONE` first,
/// then *invulnerability* — which applies to every group, not just the
/// resistable one — and only then the group gate.
///
/// - **group 3** (the resistable debuff traits: SHOCK, HOLD, SLEEP, POISON,
///   DERANGEMENT, PARALYZE, BLEED, …) is what the dist's `<trait>` tags almost
///   entirely declare, and what the learnable resistances defend.
/// - **group 2** (`*_WEAKNESS`, declared by 5 skills here) additionally needs
///   the *attacker* to carry a matching `AttackTrait`. Nothing is ported that
///   grants one, so `hasAttackTrait` is always false and Java's own guard
///   returns 1.0 — the branch is a no-op rather than a gap.
/// - **group 1** (weapon types, plus `ETC`) and `NONE` are never scaled here.
///
/// The attacker side is otherwise omitted because `getAttackTrait` is **1.0**
/// for anyone without an `AttackTrait` buff, which makes Java's
/// `max(attackTrait − defenceTrait, 0.05)` exactly `max(1 − defence, 0.05)`.
///
/// `ignore_resistance` is Java's fourth argument: the **damage** formulas pass
/// `true` (group 3 short-circuits to 1.0 — a stun resistance does not soften
/// the stun's damage), the landing roll passes `false`.
pub(crate) fn calc_general_trait_bonus(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    trait_type: crate::model::skill::TraitType,
    ignore_resistance: bool,
) -> f64 {
    use crate::model::components::DefenceTraits;
    use crate::model::skill::TraitType;
    if trait_type == TraitType::None {
        return 1.0;
    }
    let Some(traits) = world.objects.get_component::<DefenceTraits>(&target_oid) else {
        return 1.0;
    };
    // Java tests invulnerability *before* the group switch, so a weapon- or
    // weakness-trait immunity zeroes the chance too.
    if traits.invulnerable.contains(&trait_type) {
        return 0.0;
    }
    match trait_type.group() {
        // The `*_WEAKNESS` family needs **both** sides: the attacker's
        // `AttackTrait` and the target's `DefenceTrait`.
        2 => {
            if !has_attack_trait(world, attacker_oid, trait_type)
                || !traits.resist.contains_key(&trait_type)
            {
                return 1.0;
            }
        }
        3 => {
            if ignore_resistance {
                return 1.0;
            }
        }
        _ => return 1.0,
    }
    let defence = traits.resist.get(&trait_type).copied().unwrap_or(0.0);
    // A *negative* defence trait is a vulnerability (4416's -15), so this can
    // legitimately exceed 1.0 — Java only floors it.
    (attack_trait(world, attacker_oid, trait_type) - defence).max(0.05)
}

/// Java `getAttackTrait` — **1.0** for anyone without a matching `AttackTrait`
/// buff (the table's identity), which is what makes the group-3 case read as
/// the plain `1 − defence`.
fn attack_trait(world: &World, oid: i32, trait_type: crate::model::skill::TraitType) -> f64 {
    world
        .objects
        .get_component::<crate::model::components::AttackTraits>(&oid)
        .and_then(|at| at.values.get(&trait_type).copied())
        .unwrap_or(1.0)
}

/// Java `hasAttackTrait` — membership, which is a *different* question from the
/// value: an unbuffed attacker's value is 1.0 but `hasAttackTrait` is false, and
/// the group-2 branch gates on the latter.
fn has_attack_trait(world: &World, oid: i32, trait_type: crate::model::skill::TraitType) -> bool {
    world
        .objects
        .get_component::<crate::model::components::AttackTraits>(&oid)
        .is_some_and(|at| at.values.contains_key(&trait_type))
}

/// `Formulas.calcWeaponTraitBonus` — `max(0.22, 1 − defenceTrait(weaponType))`.
///
/// The attacker's *weapon type* is itself a `TraitType` (SWORD, DAGGER, BOW …),
/// and the dist's armour buffs really do grant those defence traits (19 skills
/// name SWORD, 24 DAGGER, 45 BOW…). The 0.22 floor is Java's, and note there is
/// no `hasDefenceTrait` gate here — the raw table value is read, so an absent
/// entry is a clean 1.0.
pub(crate) fn calc_weapon_trait_bonus(world: &World, attacker_oid: i32, target_oid: i32) -> f64 {
    let weapon_trait = crate::model::skill::TraitType::of_weapon(
        crate::game_loop::ranged::equipped_weapon_type(world, attacker_oid).unwrap_or_default(),
    );
    let defence = world
        .objects
        .get_component::<crate::model::components::DefenceTraits>(&target_oid)
        .and_then(|d| d.resist.get(&weapon_trait).copied())
        .unwrap_or(0.0);
    (1.0 - defence).max(0.22)
}

/// `Formulas.calcWeaknessBonus` — the product over every `*_WEAKNESS` trait the
/// attacker carries *and* the target is weak to, **excluding the skill's own**
/// trait (that one is already counted by `calcGeneralTraitBonus`).
///
/// Java's invulnerability test in here reads `isInvulnerableTrait(traitType)` —
/// the **skill's** trait, not the loop variable. That looks like a slip, but it
/// is what the reference server does, so it is reproduced.
pub(crate) fn calc_weakness_bonus(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    skill_trait: crate::model::skill::TraitType,
) -> f64 {
    use crate::model::components::DefenceTraits;
    let Some(defence) = world.objects.get_component::<DefenceTraits>(&target_oid) else {
        return 1.0;
    };
    if defence.invulnerable.contains(&skill_trait) {
        return 1.0;
    }
    let mut result = 1.0;
    for weakness in crate::model::skill::TraitType::ALL_WEAKNESS {
        if weakness == skill_trait {
            continue;
        }
        let Some(def) = defence.resist.get(&weakness).copied() else {
            continue;
        };
        if !has_attack_trait(world, attacker_oid, weakness) {
            continue;
        }
        result *= (attack_trait(world, attacker_oid, weakness) - def).max(0.05);
    }
    result
}

/// `Formulas.calcAttackTraitBonus` — the auto-attack's whole trait term: the
/// weapon bonus times every group-2 weakness, floored at 0.05.
/// Test hook for [`pvp_pve_bonus`], which is private to this module.
#[cfg(test)]
pub(crate) fn pvp_pve_bonus_for_test(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    skill: Option<&Skill>,
) -> f64 {
    pvp_pve_bonus(world, attacker_oid, target_oid, skill)
}

/// `Formulas.calculatePvpPveBonus`, resolved against world state.
///
/// `skill = None` is Java's auto-attack branch (its `skill == null`), which
/// reads the `*_PHYSICAL_ATTACK_*` pair rather than either skill pair.
///
/// Returns 1.0 for any pairing that is neither playable-vs-playable nor
/// involves an `Attackable` — two non-attackable NPCs, or a door.
pub(crate) fn pvp_pve_bonus(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    skill: Option<&Skill>,
) -> f64 {
    use crate::model::stats::Stat;

    let mul = |oid: i32, stat: Stat| -> f64 {
        world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&oid)
            .map(|m| crate::model::finalize(m, stat, 1.0))
            .unwrap_or(1.0)
    };

    // `isPlayable()` — a player or their summon (Java's `Playable` subtree).
    let is_playable = |oid: i32| {
        world.objects.has_component::<crate::model::Player>(&oid)
            || world
                .objects
                .has_component::<crate::model::components::PetOf>(&oid)
            || world
                .objects
                .has_component::<crate::model::components::ServitorOf>(&oid)
    };
    let template = |oid: i32| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| world.data.npc_data.get(n.npc_id))
    };
    let is_attackable = |oid: i32| template(oid).is_some_and(|t| t.is_attackable_class());

    // PvP: both sides playable.
    if is_playable(attacker_oid) && is_playable(target_oid) {
        let (atk_stat, def_stat) = match skill {
            None => (
                Stat::PvpPhysicalAttackDamage,
                Stat::PvpPhysicalAttackDefence,
            ),
            // `Skill.isMagic()` — `magicType == 1`.
            Some(s) if s.magic_type == 1 => {
                (Stat::PvpMagicalSkillDamage, Stat::PvpMagicalSkillDefence)
            }
            Some(_) => (Stat::PvpPhysicalSkillDamage, Stat::PvpPhysicalSkillDefence),
        };
        // Java folds in the class-balance config multipliers and a dragon
        // weapon's `DRAGON_WEAPON_DEFENCE` here; the former are blank on this
        // dist (every class 1.0) and dragon weapons post-date Interlude.
        return formulas::calculate_pvp_pve_bonus(
            mul(attacker_oid, atk_stat),
            mul(target_oid, def_stat),
            1.0,
            1.0,
            1.0,
        )
        .max(0.05);
    }

    // PvE: either side is an `Attackable`.
    if is_attackable(target_oid) || is_attackable(attacker_oid) {
        let (atk_stat, def_stat, raid_def_stat) = match skill {
            None => (
                Stat::PvePhysicalAttackDamage,
                Stat::PvePhysicalAttackDefence,
                Stat::PveRaidPhysicalAttackDefence,
            ),
            Some(s) if s.magic_type == 1 => (
                Stat::PveMagicalSkillDamage,
                Stat::PveMagicalSkillDefence,
                Stat::PveRaidMagicalSkillDefence,
            ),
            Some(_) => (
                Stat::PvePhysicalSkillDamage,
                Stat::PvePhysicalSkillDefence,
                Stat::PveRaidPhysicalSkillDefence,
            ),
        };
        // Java reads the raid pair off the **attacker** for both halves; there
        // is no `PVE_RAID_*_DAMAGE` source on this dist, so only the defence
        // half can ever move, and only while the attacker is a raid.
        let attacker_is_raid = template(attacker_oid).is_some_and(|t| t.is_raid());
        let raid_defence = if attacker_is_raid {
            mul(attacker_oid, raid_def_stat)
        } else {
            1.0
        };
        let penalty = formulas::npc_level_damage_penalty(
            &world.cfg.npc.skill_dmg_penalty_for_lvl_differences,
            creature_level(world, target_oid),
            creature_level(world, attacker_oid),
            template(target_oid).is_some_and(|t| t.is_raid()),
            world.cfg.npc.min_npc_level_for_dmg_penalty,
        );
        return formulas::calculate_pvp_pve_bonus(
            mul(attacker_oid, atk_stat),
            mul(target_oid, def_stat),
            1.0,
            raid_defence,
            penalty,
        )
        .max(0.05);
    }

    1.0
}

pub(crate) fn calc_attack_trait_bonus(world: &World, attacker_oid: i32, target_oid: i32) -> f64 {
    let weapon = calc_weapon_trait_bonus(world, attacker_oid, target_oid);
    if weapon == 0.0 {
        return 0.0;
    }
    let mut weakness = 1.0;
    for t in crate::model::skill::TraitType::ALL_WEAKNESS {
        weakness *= calc_general_trait_bonus(world, attacker_oid, target_oid, t, true);
        if weakness == 0.0 {
            return 0.0;
        }
    }
    (weapon * weakness).max(0.05)
}

/// `DefenceTrait.onStart` — merge this buff's resistances into the bearer.
pub(crate) fn merge_defence_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::DefenceTraits;
    if traits.is_empty() {
        return;
    }
    if world
        .objects
        .get_component::<DefenceTraits>(&target_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&target_oid, DefenceTraits::default());
    }
    if let Some(dt) = world
        .objects
        .get_component_mut::<DefenceTraits>(&target_oid)
    {
        for &(t, value) in traits {
            // Java: `< 1.0` merges a resistance, otherwise it is outright
            // invulnerability — a 100 in the XML is not "100 % resist".
            if value < 1.0 {
                *dt.resist.entry(t).or_insert(0.0) += value;
            } else {
                dt.invulnerable.insert(t);
            }
        }
    }
}

/// `AttackTrait.onStart` — `mergeAttackTrait(trait, value)` onto a table whose
/// identity is **1.0**, so a `<BEAST_WEAKNESS>30</BEAST_WEAKNESS>` reads as
/// 1.30.
pub(crate) fn merge_attack_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::AttackTraits;
    if traits.is_empty() {
        return;
    }
    if world
        .objects
        .get_component::<AttackTraits>(&target_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&target_oid, AttackTraits::default());
    }
    if let Some(at) = world.objects.get_component_mut::<AttackTraits>(&target_oid) {
        for &(t, value) in traits {
            *at.values.entry(t).or_insert(1.0) += value;
        }
    }
}

/// `AttackTrait.onExit`. Java's `removeAttackTrait` drops the trait from the
/// *set* once the value is back to 1 — i.e. `hasAttackTrait` goes false again —
/// which is exactly what removing the map entry does here.
pub(crate) fn remove_attack_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::AttackTraits;
    let Some(at) = world.objects.get_component_mut::<AttackTraits>(&target_oid) else {
        return;
    };
    for &(t, value) in traits {
        if let Some(cur) = at.values.get_mut(&t) {
            *cur -= value;
            if (*cur - 1.0).abs() < 1e-9 {
                at.values.remove(&t);
            }
        }
    }
}

/// `DefenceTrait.onExit` — unmerge them again.
pub(crate) fn remove_defence_traits(
    world: &mut World,
    target_oid: i32,
    traits: &[(crate::model::skill::TraitType, f64)],
) {
    use crate::model::components::DefenceTraits;
    let Some(dt) = world
        .objects
        .get_component_mut::<DefenceTraits>(&target_oid)
    else {
        return;
    };
    for &(t, value) in traits {
        if value < 1.0 {
            if let Some(cur) = dt.resist.get_mut(&t) {
                *cur -= value;
                // Float subtraction can leave a hair above zero; drop the entry
                // rather than leaving a phantom 1e-17 resistance behind.
                if *cur <= 1e-9 {
                    dt.resist.remove(&t);
                }
            }
        } else {
            dt.invulnerable.remove(&t);
        }
    }
}

/// `MagicMpCost.onStart` / `Reuse.onStart` — merge this buff's rates into the
/// bearer's per-`magicType` tables. Java merges with `mul`, so overlapping
/// songs compound rather than add.
pub(crate) fn merge_skill_rates(world: &mut World, target_oid: i32, skill: &Skill) {
    use crate::model::components::SkillRateStats;
    let rates = skill_rate_factors(skill);
    if rates.is_empty() {
        return;
    }
    if world
        .objects
        .get_component::<SkillRateStats>(&target_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&target_oid, SkillRateStats::default());
    }
    if let Some(rs) = world
        .objects
        .get_component_mut::<SkillRateStats>(&target_oid)
    {
        for (kind, magic_type, factor) in rates {
            let table = match kind {
                RateKind::MpConsume => &mut rs.mp_consume,
                RateKind::Reuse => &mut rs.reuse,
            };
            *table.entry(magic_type).or_insert(1.0) *= factor;
        }
    }
}

/// `MagicMpCost.onExit` / `Reuse.onExit` — Java's `div`, the exact inverse of
/// the `mul` above, so unmerging out of order still lands back on 1.
pub(crate) fn remove_skill_rates(world: &mut World, target_oid: i32, skill: &Skill) {
    use crate::model::components::SkillRateStats;
    let rates = skill_rate_factors(skill);
    if rates.is_empty() {
        return;
    }
    let Some(rs) = world
        .objects
        .get_component_mut::<SkillRateStats>(&target_oid)
    else {
        return;
    };
    for (kind, magic_type, factor) in rates {
        let table = match kind {
            RateKind::MpConsume => &mut rs.mp_consume,
            RateKind::Reuse => &mut rs.reuse,
        };
        if let Some(cur) = table.get_mut(&magic_type) {
            *cur /= factor;
            // Back to the identity → drop the entry, so a bearer with no live
            // rate buff reads as "no component state" rather than 0.999999.
            if (*cur - 1.0).abs() < 1e-9 {
                table.remove(&magic_type);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RateKind {
    MpConsume,
    Reuse,
}

/// The `(table, magicType, factor)` triples a skill's effects contribute.
/// Java's factor is `amount / 100 + 1`, so −30 → 0.70 and +200 → 3.0. A factor
/// of exactly 1 (Holy Squad 615's first two levels carry `amount` 0) is dropped
/// — merging it would be a no-op that still forces the component into
/// existence.
fn skill_rate_factors(skill: &Skill) -> Vec<(RateKind, i32, f64)> {
    skill
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::MagicMpCost { magic_type, amount } => {
                Some((RateKind::MpConsume, *magic_type, amount / 100.0 + 1.0))
            }
            SkillEffect::Reuse { magic_type, amount } => {
                Some((RateKind::Reuse, *magic_type, amount / 100.0 + 1.0))
            }
            _ => None,
        })
        .filter(|(_, _, factor)| (factor - 1.0).abs() > 1e-9)
        .collect()
}

/// Java `CreatureStat.getMpConsume(skill)` — the skill's raw cost scaled by the
/// caster's rate for that skill's own `magicType`, **truncated** to an int as
/// Java's `(int)` cast does.
///
/// The dance-stacking surcharge is the other half of Java's method: each dance
/// already running adds `ceil(mpConsume / 2)`. It is gated on
/// `DanceConsumeAdditionalMP`, which this dist sets to **False**, so it stays
/// off here — but it is wired to the config rather than assumed away.
pub(crate) fn mp_consume_for(world: &World, caster_oid: i32, skill: &Skill) -> i32 {
    let mut mp_consume = skill.mp_consume as f64;
    if skill.is_dance() && world.cfg.character.dance_consume_additional_mp {
        let dances = world
            .objects
            .get_component::<Buffs>(&caster_oid)
            .map(|b| {
                b.0.iter()
                    .filter(|x| x.slot == crate::model::skill::BuffSlot::Dance)
                    .count()
            })
            .unwrap_or(0);
        if dances > 0 {
            mp_consume += dances as f64 * (skill.mp_consume as f64 / 2.0).ceil();
        }
    }
    (mp_consume * skill_rate(world, caster_oid, skill, RateKind::MpConsume)) as i32
}

/// Java `CreatureStat.getReuseTime(skill)` — the raw delay scaled by the
/// caster's reuse rate for that skill's `magicType`. **Static and static-reuse
/// skills return before the multiply**, which is what keeps Super Haste's −99 %
/// off the fixed cooldowns.
pub(crate) fn reuse_time_for(world: &World, caster_oid: i32, skill: &Skill) -> i32 {
    if skill.static_reuse || skill.is_static() {
        return skill.reuse_delay;
    }
    (skill.reuse_delay as f64 * skill_rate(world, caster_oid, skill, RateKind::Reuse)) as i32
}

/// `getMpConsumeTypeValue` / `getReuseTypeValue`: the bearer's factor for the
/// bucket this skill belongs to, defaulting to 1.
fn skill_rate(world: &World, caster_oid: i32, skill: &Skill, kind: RateKind) -> f64 {
    world
        .objects
        .get_component::<crate::model::components::SkillRateStats>(&caster_oid)
        .and_then(|rs| {
            let table = match kind {
                RateKind::MpConsume => &rs.mp_consume,
                RateKind::Reuse => &rs.reuse,
            };
            table.get(&skill.magic_type).copied()
        })
        .unwrap_or(1.0)
}

/// The level a live buff was cast at, so its effect list can be looked back up
/// on expiry (a resistance's value is per level).
fn buff_level(world: &World, object_id: i32, skill_id: i32) -> i32 {
    world
        .objects
        .get_component::<Buffs>(&object_id)
        .and_then(|b| {
            b.0.iter()
                .find(|x| x.skill_id == skill_id)
                .map(|x| x.skill_level)
        })
        .unwrap_or(1)
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
