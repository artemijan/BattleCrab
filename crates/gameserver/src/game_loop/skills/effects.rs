//! Effect application: instant damage/heal effects, continuous (buff)
//! effects, and buff expiry.

use crate::game_loop::helpers::client_for_player;
use crate::model::components::{
    BaseStats, Buffs, CombatStats, RegionCell, Speeds, StatModifiers, Vitals,
};
use crate::model::formulas;
use crate::model::skill::{
    abnormal_type_client_id, ActiveBuff, BuffSlot, DispelSlot, RestorationGroup, Skill, SkillEffect,
};
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// The `callSkill` → `activateSkill` → effect-handler chain for the effect
/// kinds ported so far. Continuous stat modifiers land as an `ActiveBuff` on
/// the target; `MagicalAttack`/`Heal` are instant.
pub(crate) fn apply_skill_effects(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
) {
    use server_packets::{sm_ids, SmParam};

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
        match effect {
            // Pet food. The generic cast path never reaches this — a pet eats
            // through `servitor::apply_food_skill`, which targets the pet
            // rather than the caster — so it is a no-op here rather than a
            // second, divergent implementation.
            SkillEffect::Feed { .. } => {}
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
                    // TODO(G19): Java's Decoy and default-spawn branches
                    // (`SummonNpc.java` `switch (npcTemplate.getType())`).
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
                ) * attribute_mod(world, caster_oid, target_oid, skill);
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
                if !confuse_chance_passes(world, target_oid, skill, *chance) {
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
                if !confuse_chance_passes(world, target_oid, skill, *chance) {
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
                    if let Some(cid) = client_for_player(world, caster_oid) {
                        if let Some(cs) = world.clients.get(&cid) {
                            cs.send(server_packets::system_message_with(sm_ids::YOUR_ATTACK_HAS_FAILED, &[]));
                        }
                    }
                    if let Some(cid) = client_for_player(world, target_oid) {
                        if let Some(cs) = world.clients.get(&cid) {
                            cs.send(server_packets::system_message_with(
                                sm_ids::C1_RESISTED_C2_S_DRAIN,
                                &[
                                    SmParam::Text(caster_display_name(world, target_oid)),
                                    SmParam::Text(caster_display_name(world, caster_oid)),
                                ],
                            ));
                        }
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
                    )
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
                if drain_crit {
                    if let Some(cid) = client_for_player(world, caster_oid) {
                        if let Some(cs) = world.clients.get(&cid) {
                            cs.send(server_packets::system_message_with(sm_ids::M_CRITICAL, &[]));
                        }
                    }
                }
                if let Some(cid) = client_for_player(world, target_oid) {
                    if let Some(cs) = world.clients.get(&cid) {
                        cs.send(server_packets::system_message_with(
                            sm_ids::S2_S_MP_HAS_BEEN_DRAINED_BY_C1,
                            &[
                                SmParam::Text(caster_display_name(world, caster_oid)),
                                SmParam::Int(drained as i32),
                            ],
                        ));
                    }
                }
                if let Some(cid) = client_for_player(world, caster_oid) {
                    if let Some(cs) = world.clients.get(&cid) {
                        cs.send(server_packets::system_message_with(
                            sm_ids::YOUR_OPPONENT_S_MP_WAS_REDUCED_BY_S1,
                            &[SmParam::Int(drained as i32)],
                        ));
                    }
                }
                broadcast_vitals(world, target_oid);
            }
            SkillEffect::PhysicalAttack { power, p_atk_mod, p_def_mod, critical_chance } => {
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
                let p_def = target_p_def(world, target_oid);
                let crit = formulas::calc_physical_skill_crit(*critical_chance, str_bonus, world.roll(100));
                let rand_roll = if random_dmg > 0 { world.roll(2 * random_dmg + 1) - random_dmg } else { 0 };
                // `PhysicalAttack.instant`'s `attributeMod` term.
                let damage = formulas::calc_physical_skill_damage(
                    p_atk,
                    *p_atk_mod,
                    p_def,
                    *p_def_mod,
                    *power,
                    formulas::level_mod(level),
                    formulas::random_damage_multiplier(rand_roll),
                    crit,
                    crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, false),
                    ss,
                ) * attribute_mod(world, caster_oid, target_oid, skill);
                apply_skill_damage(world, caster_oid, target_oid, damage, crit, false, &caster_name, skill.over_hit, false, skill.id);
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

                let p_def = target_p_def(world, target_oid);
                let rand_roll = if random_dmg > 0 { world.roll(2 * random_dmg + 1) - random_dmg } else { 0 };
                let mut damage = formulas::calc_blow_damage(
                    p_atk,
                    *power,
                    p_def,
                    position,
                    formulas::random_damage_multiplier(rand_roll),
                    ss,
                );
                // `calcBlowDamage`'s `attributeMod` term.
                damage *= attribute_mod(world, caster_oid, target_oid, skill);
                // FatalBlow/Backstab double on a `calcCrit` roll; SoulBlow
                // (`critical_chance == None`) doesn't.
                if let Some(cc) = critical_chance {
                    if formulas::calc_physical_skill_crit(*cc, str_bonus, world.roll(100)) {
                        damage *= 2.0;
                    }
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
                // `apply_mute_interrupt` already uses. Grand-boss/door
                // immunity isn't modeled, so it's not checked here.
                let is_raid = world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&target_oid)
                    .and_then(|n| n.template(world))
                    .is_some_and(|t| t.is_raid());
                if is_raid {
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
                // `Lethal.instant`'s `chanceMultiplier` — the attribute half
                // (its trait half stays unported with the trait system).
                let lethal_amod = attribute_mod(world, caster_oid, target_oid, skill);
                if world.roll(100) < ((*full_lethal) * lethal_amod) as i32 {
                    if is_player_target {
                        if let Some(v) = world.objects.get_component_mut::<crate::model::components::PlayerVitals>(&target_oid) {
                            v.cur_cp = 1.0;
                        }
                        if let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                            v.cur_hp = 1.0;
                        }
                        if let Some(client_id) = client_for_player(world, target_oid) {
                            if let Some(cs) = world.clients.get(&client_id) {
                                cs.send(server_packets::system_message_with(sm_ids::LETHAL_STRIKE, &[]));
                            }
                        }
                    } else if crate::game_loop::combat::is_npc_oid(target_oid) {
                        if let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                            v.cur_hp = 1.0;
                        }
                    }
                    broadcast_vitals(world, target_oid);
                    if let Some(client_id) = caster_client {
                        if let Some(cs) = world.clients.get(&client_id) {
                            cs.send(server_packets::system_message_with(sm_ids::HIT_WITH_LETHAL_STRIKE, &[]));
                        }
                    }
                } else if world.roll(100) < ((*half_lethal) * lethal_amod) as i32 {
                    if is_player_target {
                        if let Some(v) = world.objects.get_component_mut::<crate::model::components::PlayerVitals>(&target_oid) {
                            v.cur_cp = 1.0;
                        }
                        if let Some(client_id) = client_for_player(world, target_oid) {
                            if let Some(cs) = world.clients.get(&client_id) {
                                cs.send(server_packets::system_message_with(sm_ids::HALF_KILL, &[]));
                                cs.send(server_packets::system_message_with(
                                    sm_ids::YOUR_CP_WAS_DRAINED_BECAUSE_YOU_WERE_HIT_WITH_A_HALF_KILL_SKILL,
                                    &[],
                                ));
                            }
                        }
                    } else if crate::game_loop::combat::is_npc_oid(target_oid) {
                        if let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                            v.cur_hp *= 0.5;
                        }
                    }
                    broadcast_vitals(world, target_oid);
                    if let Some(client_id) = caster_client {
                        if let Some(cs) = world.clients.get(&client_id) {
                            cs.send(server_packets::system_message_with(sm_ids::HALF_KILL, &[]));
                        }
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
                ) * attribute_mod(world, caster_oid, target_oid, skill);

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
                let healed = {
                    let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else { continue };
                    // Overheal clamp (`Heal.java`).
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
            SkillEffect::EnergyAttack { power, critical_chance, p_def_mod, charge_consume } => {
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
                let p_def = target_p_def(world, target_oid);
                let crit = formulas::calc_physical_skill_crit(*critical_chance, str_bonus, world.roll(100));
                // `energyChargesBoost = 1 + (charge * 0.1)` — 10% bonus damage
                // per charge spent, the whole point of building Force first.
                let energy_charges_boost = 1.0 + charge as f64 * 0.1;
                let damage = formulas::calc_physical_skill_damage(
                    p_atk,
                    1.0, // no separate pAtkMod term in Java's EnergyAttack formula
                    p_def,
                    *p_def_mod,
                    *power,
                    formulas::level_mod(level),
                    1.0, // no random-damage term in Java's EnergyAttack formula
                    crit,
                    crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, false),
                    ss,
                ) * energy_charges_boost
                    // `EnergyAttack.instant`'s `attributeMod` term.
                    * attribute_mod(world, caster_oid, target_oid, skill);
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
            SkillEffect::ConsumeBody => {
                apply_consume_body(world, caster_oid, target_oid);
            }
            SkillEffect::DamOverTime { power, ticks, can_kill } => {
                // `DamOverTime.onStart`: a magic (non-toggle) DoT bursts for
                // `power * 10` on a magic-crit roll ("10 times HP DOT is taken
                // during magic critical"), clamped to leave the target alive
                // unless `canKill`. The periodic ticks are armed once below via
                // `schedule_dam_over_time`, after the buff lands.
                // TODO(G16): Java notes m.crit can land even when the debuff is
                // resisted — the port has no land-rate/resist roll yet, so the
                // two are tied here.
                if skill.magic_type == 1 && mcrit && *ticks > 0 {
                    let mut damage = *power * 10.0;
                    if !*can_kill {
                        let cur_hp =
                            world.objects.get_component::<Vitals>(&target_oid).map(|v| v.cur_hp).unwrap_or(0.0);
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
                        apply_skill_damage(world, caster_oid, target_oid, damage, true, true, &caster_name, skill.over_hit, false, skill.id);
                    }
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
            }
            SkillEffect::DispelBySlotProbability { dispel, rate } => {
                // Java `DispelBySlotProbability.instant`: the same cleanse as
                // `DispelBySlot`, except the `rate`% roll is evaluated **per
                // buff** inside the predicate — so a 40% Mass Warrior Bane
                // strips roughly two of five matching buffs rather than all or
                // nothing. The spec carries no per-type level, so every level
                // of a listed abnormal type is a candidate.
                //
                // Java also skips `isIrreplacableBuff()` effects; no skill on
                // this dist sets that flag, so it is not modelled. TODO(G19).
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
                if world.roll(100) >= *chance {
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
                if *power > 0.0 {
                    if let Some(ai) = world.objects.get_component_mut::<crate::model::npc::NpcAi>(&target_oid) {
                        if ai.intention != crate::model::npc::NpcIntention::Attack {
                            ai.intention = crate::model::npc::NpcIntention::Attack;
                            ai.attack_timeout_tick = world.tick + crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
                        }
                    }
                }
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
                if let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&target_oid) {
                    if let Some(entry) = aggro.0.get_mut(&caster_oid) {
                        entry.hate = 0.0;
                    }
                }
                crate::game_loop::npc_ai::set_active(world, target_oid);
            }
            // Periodic effects do nothing on application; their work happens on
            // the tick chain armed by `schedule_dam_over_time`.
            SkillEffect::HealOverTime { .. } | SkillEffect::ManaDamOverTime { .. } | SkillEffect::MpConsumePerLevel { .. } => {}
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
            // no instant action — they land purely as an icon-only timed buff
            // (kept off the empty-`buff_effects` bail via `has_iconless_buff`).
            // DefenceTrait/VampiricAttack's real mechanics (trait resistances /
            // melee HP absorb) aren't modeled yet; AttackTrait is inert on the
            // real server too (see its doc comment) — nothing to model.
            // TODO(G16/G20): honor the trait-defense and HP-absorb effects.
            SkillEffect::DefenceTrait | SkillEffect::VampiricAttack | SkillEffect::AttackTrait => {}
            // Community-board dance/song buffs (Song of Champion/Renewal/
            // Vengeance, Gift of Seraphim): no instant action — they land
            // purely as icon-only timed buffs (kept off the empty-`buff_effects`
            // bail via `has_iconless_buff`). Their real mechanics (MP-consume
            // rate / reuse rate / damage reflect) aren't modeled yet.
            // TODO(G16/G20): honor the MP-cost/reuse/reflect effects.
            SkillEffect::MagicMpCost
            | SkillEffect::Reuse
            | SkillEffect::DamageShield => {}
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
    if ss {
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&caster_oid)
        {
            p.uncharge_shot(crate::model::ShotType::Soulshots);
        }
    }

    apply_continuous_effects(world, caster_oid, target_oid, skill, None);
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
) {
    use server_packets::{sm_ids, SmParam};

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
                | SkillEffect::MpConsumePerLevel { .. }
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
                | SkillEffect::DefenceTrait
                | SkillEffect::VampiricAttack
                | SkillEffect::MagicMpCost
                | SkillEffect::Reuse
                | SkillEffect::DamageShield
                | SkillEffect::Transform { .. }
                | SkillEffect::AttackTrait
        )
    });
    if buff_effects.is_empty() && !has_periodic && !has_iconless_buff && !has_state_flag {
        return;
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
    // TODO(G16): a magic-crit `DamOverTime` burst is applied in the effect loop
    // above before this roll — Java gates that burst on landing too.
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
        );
        // Java: resisted when `finalRate <= Rnd.get(100)` (0-99). Roll before the
        // message so the outcome line reflects it and the roll order stays stable.
        let resisted = rate <= world.roll(100) as f64;
        if skill.affect_scope == crate::model::skill::AffectScope::Single {
            let target_name = creature_name(world, target_oid);
            let text = if resisted {
                format!(
                    "{} has resisted {}: {}%",
                    target_name, skill.name, rate as i64
                )
            } else {
                format!(
                    "{} landed with {}% chance on {}",
                    skill.name, rate as i64, target_name
                )
            };
            if let Some(client_id) = client_for_player(world, caster_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(
                        sm_ids::S1_TEXT,
                        &[SmParam::Text(text)],
                    ));
                }
            }
        }
        if resisted {
            return;
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
        return;
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
            return;
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

    // Arm the poison/bleed damage-over-time ticks (Java `BuffInfo.
    // scheduleEffects` → `scheduleAtFixedRate`). The recurring `DamOverTimeTick`
    // self-terminates once this buff's `BuffExpire` removes it or the target
    // dies; done here so it covers both NPC and player targets.
    schedule_dam_over_time(world, caster_oid, target_oid, skill);

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
        return;
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
            return;
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
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(buffs) = world.objects.get_component::<Buffs>(&target_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(crate::network::enter_world::abnormal_status_update(
                        buffs, now,
                    ));
                }
            }
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
fn confuse_chance_passes(world: &mut World, target_oid: i32, skill: &Skill, chance: i32) -> bool {
    let level = target_level(world, target_oid);
    let roll = world.roll(100);
    formulas::calc_probability(skill.magic_level, chance, level, roll)
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
/// `maxMp`). **No skill on this dist grants that stat** — the `LimitMp` handler
/// exists but nothing uses it — so the ceiling is plain `maxMp` here.
fn restore_mp(world: &mut World, caster_oid: i32, target_oid: i32, amount: f64) {
    use server_packets::{sm_ids, SmParam};
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

/// How far one fear shove throws the victim — Java `Fear.FEAR_RANGE`.
const FEAR_RANGE: f64 = 500.0;

/// `Fear.canStart` — who can be feared at all. Raid bosses are immune (the
/// same `isRaid()` bail `Mute` has), and on the NPC side only the `Attackable`
/// subtree qualifies, minus the siege-defence family: a fear must not scatter
/// stationed defenders off a castle wall or push a siege golem around.
/// A player is always fearable. (Java's `isSummon()` leg has no ported
/// counterpart — servitors are `TODO(G29)` — and folds into the player case
/// once they exist.)
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
    if world
        .objects
        .has_component::<crate::model::components::Casting>(&target_oid)
    {
        crate::game_loop::skills::cast::stop_casting(world, target_oid);
    }
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

/// `Creature.stopMove` + `abortCast` on the freshly-stunned victim: a skill
/// that lands `BLOCK_ACTIONS` interrupts whatever the target was doing, rather
/// than only preventing the *next* action. Without this a stun landing
/// mid-cast would let the cast finish.
///
/// A root deliberately does not do this — it stops movement (the movement
/// primitives refuse it from the next tick) but leaves a cast running.
fn apply_block_actions_interrupt(world: &mut World, target_oid: i32) {
    // Order matters: abort the cast *first*. `stop_casting` resumes the move
    // the cast interrupted (`start_casting` stashes it), so clearing movement
    // before the cast would see it immediately restored — the victim would keep
    // walking while stunned.
    if world
        .objects
        .has_component::<crate::model::components::Casting>(&target_oid)
    {
        crate::game_loop::skills::cast::stop_casting(world, target_oid);
    }
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
        {
            if let Some(region) = world
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
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE,
                    &[],
                ));
            }
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
        if let Some(client_id) = client_for_player(world, target_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE,
                    &[],
                ));
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
        {
            if let Some(inv) = world.objects.get_component_mut::<Inventory>(&target_oid) {
                for &oid in &changed_oids {
                    inv.set_item_enchant(oid, enchant);
                }
            }
        }
        let Some(inventory) = world.objects.get_component::<Inventory>(&target_oid) else {
            continue;
        };
        if let Some(client_id) = client_for_player(world, target_oid) {
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
                cs.send(crate::network::enter_world::inventory_update(
                    inventory,
                    &world.data,
                    &changed_oids,
                ));
            }
        }
    }
}

/// Send a bare (no-argument) system message to `player_oid`, if online.
fn send_sm(world: &World, player_oid: i32, sm_id: i16) {
    if let Some(client_id) = client_for_player(world, player_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_id, &[]));
        }
    }
}

/// Send a system message with parameters to `player_oid`, if online.
fn send_sm_with(world: &World, player_oid: i32, sm_id: i16, params: &[server_packets::SmParam]) {
    if let Some(client_id) = client_for_player(world, player_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_id, params));
        }
    }
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
    use server_packets::{sm_ids, SmParam};

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
            send_sm(
                world,
                caster_oid,
                if is_drain {
                    sm_ids::DRAIN_WAS_ONLY_50_SUCCESSFUL
                } else {
                    sm_ids::YOUR_ATTACK_HAS_FAILED
                },
            );
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
            &[SmParam::Text(caster_name)],
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
    // skipped — item weight/slot limits aren't modeled for this path yet.
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
    use server_packets::{sm_ids, SmParam};
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
    use server_packets::{sm_ids, SmParam};

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
        if let Some(client_id) = client_for_player(world, caster_oid) {
            if let Some(cs) = world.clients.get(&client_id) {
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

    if let Some(client_id) = client_for_player(world, caster_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
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
    }

    // Victim-side application: CP soak/HP/death/cast-break for players
    // (including the C1_HAS_RECEIVED message), hate + AI wake + death for
    // NPCs — the same receivers the auto-attack hits go through. The skill id
    // rides on the world for the duration of the hit so quest `onAttack` can
    // read it (Java threads `Skill` straight into the notification).
    world.quest_attack_skill = Some(skill_id);
    crate::game_loop::combat::apply_physical_damage(world, caster_oid, target_oid, damage, is_dot);
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
    let Some(t) = world.data.npc_data.get(npc_id) else {
        return;
    };
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
fn broadcast_vitals(world: &World, target_oid: i32) {
    if let Some(client_id) = client_for_player(world, target_oid) {
        if let Some((v, cs)) = world
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
            // computed identically here. TODO(G19): split them out if a skill
            // ever needs `MpConsumePerLevel`'s level-scaled `abnormalTime > 0`
            // branch (`((level-1)/7.5) * base * abnormalTime`), unexercised
            // today.
            // `Fear.onActionTime` — keep running. Java passes `null` for the
            // effector here (not the caster it had at `onStart`), so every
            // repeat steers by the victim's current heading: they keep going
            // the way the first shove threw them instead of being re-aimed
            // away from a caster who may be dead, gone or long out of range.
            SkillEffect::Fear { ticks } if *ticks > 0 => {
                interval = dot_interval_ticks(*ticks);
                fear_action(world, None, target_oid);
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
                    if let Some(client_id) = client_for_player(world, target_oid) {
                        if let Some(cs) = world.clients.get(&client_id) {
                            cs.send(server_packets::system_message_with(
                                server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP,
                                &[],
                            ));
                        }
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
pub(crate) fn handle_buff_expire(world: &mut World, player_object_id: i32, skill_id: i32) {
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
        if was_afraid {
            if let Some(ai) = world
                .objects
                .get_component_mut::<crate::model::npc::NpcAi>(&player_object_id)
            {
                if ai.intention == crate::model::npc::NpcIntention::MoveTo {
                    ai.intention = crate::model::npc::NpcIntention::Active;
                }
            }
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
    if let Some(buffs) = world.objects.get_component::<Buffs>(&player_object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::enter_world::abnormal_status_update(
                buffs, now,
            ));
        }
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
