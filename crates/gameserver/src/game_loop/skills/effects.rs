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
            SkillEffect::MagicalAttack { power } => {
                let power = *power;
                let (m_atk, caster_name) = {
                    let m_atk =
                        world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                    (m_atk, world.objects.get_component::<crate::model::Player>(&caster_oid).expect("player").name.clone())
                };
                let m_def = target_m_def(world, target_oid);
                let damage = formulas::calc_magic_dam(m_atk, m_def, power, mcrit, magic_shots_bonus);
                apply_skill_damage(world, caster_oid, target_oid, damage, mcrit, true, &caster_name);
            }
            SkillEffect::PhysicalAttack { power, p_atk_mod, p_def_mod, critical_chance } => {
                // `PhysicalAttack.instant()`: crit is rolled here (per-effect in
                // Java), not the once-per-cast magic roll above.
                let (p_atk, level, str_bonus, random_dmg, caster_name) = {
                    let cs = world.objects.get_component::<CombatStats>(&caster_oid);
                    let p_atk = cs.map(|c| c.p_atk).unwrap_or(0.0);
                    let random_dmg = cs.map(|c| c.random_dmg).unwrap_or(0);
                    let player =
                        world.objects.get_component::<crate::model::Player>(&caster_oid).expect("player");
                    let str_bonus = world
                        .objects
                        .get_component::<BaseStats>(&caster_oid)
                        .map(|b| world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Str, b.str_))
                        .unwrap_or(1.0);
                    (p_atk, player.level, str_bonus, random_dmg, player.name.clone())
                };
                let p_def = target_p_def(world, target_oid);
                let crit = formulas::calc_physical_skill_crit(*critical_chance, str_bonus, world.roll(100));
                let rand_roll = if random_dmg > 0 { world.roll(2 * random_dmg + 1) - random_dmg } else { 0 };
                let damage = formulas::calc_physical_skill_damage(
                    p_atk,
                    *p_atk_mod,
                    p_def,
                    *p_def_mod,
                    *power,
                    formulas::level_mod(level),
                    formulas::random_damage_multiplier(rand_roll),
                    crit,
                    ss,
                );
                apply_skill_damage(world, caster_oid, target_oid, damage, crit, false, &caster_name);
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

                let (p_atk, crit_rate, str_bonus, random_dmg, caster_name) = {
                    let cs = world.objects.get_component::<CombatStats>(&caster_oid);
                    let p_atk = cs.map(|c| c.p_atk).unwrap_or(0.0);
                    let crit_rate = cs.map(|c| c.crit_hit).unwrap_or(0.0);
                    let random_dmg = cs.map(|c| c.random_dmg).unwrap_or(0);
                    let str_bonus = world
                        .objects
                        .get_component::<BaseStats>(&caster_oid)
                        .map(|b| world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Str, b.str_))
                        .unwrap_or(1.0);
                    let name =
                        world.objects.get_component::<crate::model::Player>(&caster_oid).expect("player").name.clone();
                    (p_atk, crit_rate, str_bonus, random_dmg, name)
                };

                // `calcBlowSuccess`: does the blow land? A miss is silent
                // (Java's `calcSuccess == false` skips the whole effect).
                let landed = formulas::calc_blow_success(
                    crit_rate / 10.0,
                    position,
                    a.z,
                    t.z,
                    *chance_boost,
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
                // FatalBlow/Backstab double on a `calcCrit` roll; SoulBlow
                // (`critical_chance == None`) doesn't.
                if let Some(cc) = critical_chance {
                    if formulas::calc_physical_skill_crit(*cc, str_bonus, world.roll(100)) {
                        damage *= 2.0;
                    }
                }
                // Java passes `critical = true` to `doAttack` for every blow, so
                // it always shows as a critical hit.
                apply_skill_damage(world, caster_oid, target_oid, damage, true, false, &caster_name);
            }
            SkillEffect::HpDrain { power, percentage } => {
                let power = *power;
                let (m_atk, caster_name) = {
                    let m_atk =
                        world.objects.get_component::<CombatStats>(&caster_oid).map(|c| c.m_atk).unwrap_or(0.0);
                    (m_atk, world.objects.get_component::<crate::model::Player>(&caster_oid).expect("player").name.clone())
                };
                let m_def = target_m_def(world, target_oid);
                let damage = formulas::calc_magic_dam(m_atk, m_def, power, mcrit, magic_shots_bonus);

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
                apply_skill_damage(world, caster_oid, target_oid, damage, mcrit, true, &caster_name);
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
                        apply_skill_damage(world, caster_oid, target_oid, damage, true, true, &caster_name);
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
                        .is_some_and(|bs| dispel.iter().any(|ty| bs.abnormal_type == *ty));
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
            | SkillEffect::BlockControl => {}
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
            // Periodic effects do nothing on application; their work happens on
            // the tick chain armed by `schedule_dam_over_time`.
            SkillEffect::HealOverTime { .. } | SkillEffect::ManaDamOverTime { .. } => {}
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
            SkillEffect::ProtectionBlessing => {}
            // DefenceTrait (Mental Shield / Resist Shock) and VampiricAttack
            // (Vampiric Rage): no instant action — they land purely as an
            // icon-only timed buff (kept off the empty-`buff_effects` bail via
            // `has_iconless_buff`). Their real mechanics (trait resistances /
            // melee HP absorb) aren't modeled yet.
            // TODO(G16/G20): honor the trait-defense and HP-absorb effects.
            SkillEffect::DefenceTrait | SkillEffect::VampiricAttack => {}
            // Community-board dance/song buffs (Dance of Light, Song of Champion/
            // Renewal/Vengeance, Gift of Seraphim): no instant action — they land
            // purely as icon-only timed buffs (kept off the empty-`buff_effects`
            // bail via `has_iconless_buff`). Their real mechanics (attack element /
            // MP-consume rate / reuse rate / damage reflect) aren't modeled yet.
            // TODO(G16/G20): honor the element/MP-cost/reuse/reflect effects.
            SkillEffect::AttackAttribute
            | SkillEffect::MagicMpCost
            | SkillEffect::Reuse
            | SkillEffect::DamageShield => {}
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
    // Spend the soulshot on a physical/thrown skill (Java `unchargeShot(SOULSHOTS)`).
    if ss {
        if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&caster_oid) {
            p.uncharge_shot(crate::model::ShotType::Soulshots);
        }
    }

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
    let has_iconless_buff = skill.effects.iter().any(|e| {
        matches!(
            e,
            SkillEffect::ProtectionBlessing
                | SkillEffect::DefenceTrait
                | SkillEffect::VampiricAttack
                | SkillEffect::AttackAttribute
                | SkillEffect::MagicMpCost
                | SkillEffect::Reuse
                | SkillEffect::DamageShield
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
                .and_then(|m| m.mul.get(&crate::model::stats::Stat::ResistAbnormalDebuff).copied())
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
        );
        // Java: resisted when `finalRate <= Rnd.get(100)` (0-99). Roll before the
        // message so the outcome line reflects it and the roll order stays stable.
        let resisted = rate <= world.roll(100) as f64;
        if skill.affect_scope == crate::model::skill::AffectScope::Single {
            let target_name = creature_name(world, target_oid);
            let text = if resisted {
                format!("{} has resisted {}: {}%", target_name, skill.name, rate as i64)
            } else {
                format!("{} landed with {}% chance on {}", skill.name, rate as i64, target_name)
            };
            if let Some(client_id) = client_for_player(world, caster_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::system_message_with(sm_ids::S1_TEXT, &[SmParam::Text(text)]));
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
    if skill.is_debuff && caster_oid != target_oid && crate::game_loop::abnormal::is_debuff_blocked(world, target_oid) {
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
            .is_some_and(|b| b.0.iter().any(|x| x.blocked_abnormals.iter().any(|t| *t == skill.abnormal_type)));
        if blocked {
            return;
        }
    }

    let permanent = skill.abnormal_time <= 0;
    let expires_at_tick = if permanent { u64::MAX } else { world.tick + skill.abnormal_time as u64 * 10 };
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
            world
                .scheduler
                .schedule(expires_at_tick, ScheduledTask::BuffExpire { player_object_id: target_oid, skill_id: skill.id });
        }
        return;
    }
    {
        let landed = if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) = world
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
            target.apply_buff(&world.data, &base, &mut mods, &inventory, &mut buffs, &mut speeds, &mut combat, buff)
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
    }
}

/// Push the creature's current abnormal-visual set to their **own** client
/// (`ExUserInfoAbnormalVisualEffect`). The set other people see rides on the
/// `CharInfo` that `broadcast_user_info` already sends; this is the self-facing
/// half, without which a stunned player sees no swirl on themselves.
fn refresh_abnormal_visuals(world: &World, object_id: i32) {
    let Some(client_id) = client_for_player(world, object_id) else { return };
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
        cs.send(crate::network::user_info::ex_user_info_abnormal_visual_effect(
            object_id, invisible, transform, &visuals,
        ));
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
        & (crate::model::skill::effect_flag::MUTED | crate::model::skill::effect_flag::PHYSICAL_MUTED)
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
    if world.objects.has_component::<crate::model::components::Casting>(&target_oid) {
        crate::game_loop::skills::cast::stop_casting(world, target_oid);
    }
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
    if world.objects.has_component::<crate::model::components::Casting>(&target_oid) {
        crate::game_loop::skills::cast::stop_casting(world, target_oid);
    }
    // Then freeze them where they stand and tell everyone who can see them.
    if world.objects.has_component::<crate::model::components::Movement>(&target_oid) {
        world.objects.remove_component::<crate::model::components::Movement>(&target_oid);
        if let Some(pos) = world.objects.get_component::<crate::model::components::Position>(&target_oid).copied() {
            if let Some(region) = world.objects.get_component::<crate::model::components::RegionCell>(&target_oid).map(|r| r.0) {
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
    if crate::game_loop::combat::is_npc_oid(oid) {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| n.template(world))
            .map(|t| t.level)
            .unwrap_or(1)
    } else {
        world.objects.get_component::<crate::model::Player>(&oid).map(|p| p.level).unwrap_or(1)
    }
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
        world.objects.get_component::<crate::model::Player>(&oid).map(|p| p.name.clone()).unwrap_or_default()
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

/// Send a bare (no-argument) system message to `player_oid`, if online.
fn send_sm(world: &World, player_oid: i32, sm_id: i16) {
    if let Some(client_id) = client_for_player(world, player_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_id, &[]));
        }
    }
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
    let dead = world.objects.get_component::<Vitals>(&target_oid).map(|v| v.dead).unwrap_or(true);
    if !is_monster || dead {
        send_sm(world, caster_oid, sm_ids::INVALID_TARGET);
        return;
    }
    // `target.isSpoiled()` → already spoiled.
    if world.objects.get_component::<Npc>(&target_oid).map(|n| n.spoiler_object_id != 0).unwrap_or(false) {
        send_sm(world, caster_oid, sm_ids::IT_HAS_ALREADY_BEEN_SPOILED);
        return;
    }
    // `calcSuccess` = `Formulas.calcMagicSuccess`. The effective level is the
    // skill's `magicLevel` when `CalculateMagicSuccessBySkillMagicLevel` is on
    // (dist default), else the caster's level.
    let caster_level = world.objects.get_component::<crate::model::Player>(&caster_oid).map(|p| p.level).unwrap_or(1);
    let target_level = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .map(|t| t.level)
        .unwrap_or(1);
    let effective_level = if world.cfg.character.calculate_magic_success_by_skill_magic_level && skill.magic_level > 0 {
        skill.magic_level
    } else {
        caster_level
    };
    if !formulas::calc_magic_success(target_level, effective_level, world.roll(100)) {
        // Magic resisted: `applyEffectScope` skips `instant()` — no effect,
        // and Java sends no message on a failed `calcSuccess`.
        return;
    }
    if let Some(npc) = world.objects.get_component_mut::<Npc>(&target_oid) {
        npc.spoiler_object_id = caster_oid;
    }
    send_sm(world, caster_oid, sm_ids::THE_SPOIL_CONDITION_HAS_BEEN_ACTIVATED);
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
    let spoiler = world.objects.get_component::<Npc>(&target_oid).map(|n| n.spoiler_object_id).unwrap_or(0);
    if spoiler == 0 || (spoiler != caster_oid && !crate::game_loop::party::same_party(world, caster_oid, spoiler)) {
        return;
    }
    // `takeSweep()` — atomically claim the loot (a second sweep gets nothing).
    // TODO(G15): `checkInventorySlotsAndWeight` (inventory-full refusal) is
    // skipped — item weight/slot limits aren't modeled for this path yet.
    let Some(items) = world.objects.get_component_mut::<Npc>(&target_oid).and_then(|n| n.sweep_items.take()) else {
        return;
    };
    let corpse = world.objects.get_component::<Position>(&target_oid).map(|p| (p.x, p.y)).unwrap_or((0, 0));
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
    if world.objects.get_component::<Vitals>(&target_oid).map(|v| !v.dead).unwrap_or(true) {
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
) {
    use server_packets::{sm_ids, SmParam};

    // A siege door: route the hit straight to the gate's HP (no CP/hate/AI
    // receivers) and refresh its HP bar, then report the damage to the caster.
    if world.objects.has_component::<crate::model::door::Door>(&target_oid) {
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
                    &[SmParam::PlayerName(caster_name.to_string()), SmParam::Text(door_name), SmParam::Int(damage as i32)],
                ));
            }
        }
        crate::game_loop::combat::apply_door_damage(world, target_oid, damage as i32);
        return;
    }

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
                    SmParam::Popup { target: target_oid, attacker: caster_oid, damage: -dmg_int },
                ],
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
    if let Some((buffs, mut combat, mut speeds, mut vitals)) = world
        .objects
        .get_many_mut::<(&Buffs, &mut CombatStats, &mut Speeds, &mut crate::model::components::Vitals)>(&target_oid)
    {
        crate::model::recompute_npc_stats_from_buffs(&world.data, t, buffs, &mut combat, &mut speeds, &mut vitals);
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
    let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) else { return };
    let (level, class_id, base_class_id) = (p.level, p.class_id, p.base_class_id);
    let t = world
        .data
        .player_templates
        .get(class_id)
        .or_else(|| world.data.player_templates.get(base_class_id))
        .cloned()
        .unwrap_or_default();
    let (max_hp, max_mp, max_cp) = {
        let Some(mods) = world.objects.get_component::<StatModifiers>(&oid) else { return };
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
    if world.objects.get_component::<Vitals>(&target_oid).is_none_or(|v| v.dead) {
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
            // `ManaDamOverTime.onActionTime` — MP upkeep.
            SkillEffect::ManaDamOverTime { power, ticks } if *ticks > 0 => {
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

        let SkillEffect::DamOverTime { power, ticks, can_kill } = effect else { continue };
        if *ticks <= 0 {
            continue;
        }
        interval = dot_interval_ticks(*ticks);
        let mut damage = dot_tick_damage(*power, *ticks);
        // `!canKill`: a tick may never drop the target below 1 HP.
        if !*can_kill {
            let cur_hp = world.objects.get_component::<Vitals>(&target_oid).map(|v| v.cur_hp).unwrap_or(0.0);
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
            apply_skill_damage(world, caster_oid, target_oid, damage, false, skill.magic_type == 1, &caster_name);
            // A `canKill` tick can kill outright — stop then.
            if world.objects.get_component::<Vitals>(&target_oid).is_none_or(|v| v.dead) {
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
            ScheduledTask::DamOverTimeTick { caster: caster_oid, target: target_oid, skill_id, skill_level },
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
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill_id && !x.abnormal_visuals.is_empty()));
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
    // Reverting a MaxHp/MaxMp/MaxCp buff shrinks the bar (and clamps current).
    recompute_max_vitals(world, player_object_id);
    let now = world.tick;
    // Removing the buff reverted its stat contribution — rebroadcast so the
    // client (and nearby players, for speed) see the stats return to normal.
    crate::game_loop::party::broadcast_user_info(world, player_object_id);
    if had_visuals {
        refresh_abnormal_visuals(world, player_object_id);
    }
    let Some(client_id) = client_for_player(world, player_object_id) else { return };
    if let Some(buffs) = world.objects.get_component::<Buffs>(&player_object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(crate::network::enter_world::abnormal_status_update(buffs, now));
        }
    }
}

