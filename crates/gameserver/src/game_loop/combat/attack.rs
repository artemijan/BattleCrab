use super::apply_attack_damage;
use super::apply_door_damage;
use super::combatant;
use super::crit_damage_auto;
use super::crit_rate_position_mul;
use super::defence_crit_rate;
use super::is_npc_oid;
use super::refresh_attack_stance;
use super::shots_bonus_of;
use super::vitals_of;
use super::wields_two_handed;
use crate::game_loop::net::broadcast;
use crate::game_loop::space::position;
use crate::game_loop::space::position::maybe_position;
use crate::game_loop::{helpers, npc};

use crate::model::components::combat::AttackState;
use crate::model::components::combat::Intent;
use crate::model::components::space::Position;
use crate::model::formulas;
use crate::model::movement;
use crate::model::movement::get_position;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::scheduler::ScheduledTask;
use crate::scheduler::ms_to_ticks;
use crate::world::World;

/// Port of `Creature.doAutoAttack` + `generateAttackTargetData`/`generateHit`
/// for the melee single-hit case, shared by players and NPCs: roll the hit
/// now, broadcast `Attack`, land it at `timeToHit` via the scheduler.
pub(crate) fn do_auto_attack(world: &mut World, attacker_oid: i32, target_oid: i32) {
    let Some(attacker) = combatant(world, attacker_oid) else {
        return;
    };
    let Some(target) = combatant(world, target_oid) else {
        return;
    };
    if attacker.dead || target.dead {
        return;
    }

    // GeoData LOS check (`doAutoAttack`).
    if !world.geo.can_see_target(
        attacker.x, attacker.y, attacker.z, target.x, target.y, target.z,
    ) {
        if let Some(client_id) = helpers::client_for_player(world, attacker_oid) {
            crate::game_loop::helpers::send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::CANNOT_SEE_TARGET,
                &[],
            );
        }
        world.objects.remove_component::<Intent>(&attacker_oid);
        return;
    }

    // Ranged weapons run Java's `doAttack` BOW/CROSSBOW gate before the swing:
    // reload delay, ammunition, MP. Only players carry ammunition — an NPC
    // archer shoots freely, as in Java (the whole block is `isPlayer()`-gated
    // apart from the reuse timer).
    let weapon_type = super::ranged::equipped_weapon_type(world, attacker_oid).unwrap_or_default();
    if super::ranged::is_ranged(weapon_type)
        && world
            .objects
            .has_component::<crate::model::Player>(&attacker_oid)
        && let Err(why) = super::ranged::prepare_ranged_shot(world, attacker_oid, weapon_type)
    {
        super::ranged::report_refusal(world, attacker_oid, why);
        return;
    }

    let time_atk = formulas::timing::calculate_time_between_attacks(attacker.p_atk_spd);
    // Two-handed timing needs the weapon's body part — item kinds are parsed
    // (G5), so check the equipped right hand for SLOT_LR_HAND.
    let two_handed = wields_two_handed(world, attacker_oid);
    let time_to_hit = formulas::timing::calculate_time_to_hit(time_atk, two_handed);

    // Face the target (Java `setHeading(calculateHeadingFrom(...))`).
    let heading = movement::calculate_heading(
        (target.x - attacker.x) as f64,
        (target.y - attacker.y) as f64,
    );
    if let Some(pos) = world.objects.get_component_mut::<Position>(&attacker_oid) {
        pos.heading = heading;
    } else if let Some(pos) = world.objects.get_component_mut::<Position>(&attacker_oid) {
        pos.heading = heading;
    }

    // Java `doAttack`: "Always try to charge soulshots" before the swing —
    // the auto-use toggle re-charges the physical shot if the player set one.
    if is_npc_oid(attacker_oid) {
        // A **summon** charges from its owner's Beast shots (Java
        // `Summon.rechargeShots`); a plain monster has no owner and no-ops.
        crate::game_loop::servitor::recharge_shots(world, attacker_oid, true);
    } else if !world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.is_charged_shot(crate::model::ShotType::Soulshots))
    {
        crate::game_loop::items::recharge_shots(world, attacker_oid, true, false, false);
    }

    // Roll the hit (`generateHit`): miss → everything else skipped.
    let position = get_position(attacker.x, attacker.y, target.x, target.y, target.heading);
    let condition = world.data.hit_condition_bonus.condition_bonus(
        attacker.z,
        target.z,
        position,
        // `World::now_millis` rather than the free function: the night flag is
        // a decision the client can observe, and tests pin the clock with it.
        crate::game_loop::upkeep::game_time::is_night_at(world.now_millis()),
    );
    // `generateAttackTargetData` — one swing can carry several hits, and a
    // **dual** weapon rolls the whole ladder twice (miss, shield, crit,
    // damage), each hit at half damage; the soulshot is consumed by the first
    // non-missing hit and its boost rides the rest of the swing (Java threads
    // `shotConsumed` through `generateHit`).
    let weapon_id = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&attacker_oid)
        .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
        .unwrap_or(0);
    let is_dual = matches!(
        world.data.item_data.weapon_type(weapon_id),
        crate::data::item_data::kinds::WeaponType::Dual
            | crate::data::item_data::kinds::WeaponType::DualBlunt
            | crate::data::item_data::kinds::WeaponType::DualDagger
            | crate::data::item_data::kinds::WeaponType::DualFist
    );

    // Rolled per hit: (miss, crit, damage, ss, shield).
    let mut rolled: Vec<(bool, bool, i32, bool, u8)> = Vec::new();
    let mut shot_consumed = false;
    for _ in 0..if is_dual { 2 } else { 1 } {
        let miss_roll = world.roll(1000);
        let miss = formulas::physical::calc_hit_miss(
            attacker.accuracy,
            target.evasion,
            condition,
            miss_roll,
        );
        // `generateHit`: a charged soulshot is spent on the first non-miss and
        // doubles the swing (`unchargeShot(SOULSHOTS)`); a later hit of the
        // same swing reuses the charge.
        if !shot_consumed && !miss {
            shot_consumed = if is_npc_oid(attacker_oid) {
                crate::game_loop::servitor::uncharge_soulshot(world, attacker_oid)
            } else {
                world
                    .objects
                    .get_component_mut::<crate::model::Player>(&attacker_oid)
                    .is_some_and(|p| p.uncharge_shot(crate::model::ShotType::Soulshots))
            };
        }
        let ss = shot_consumed;
        // `Stat.SHOTS_BONUS`: an enchanted weapon lifts the soulshot multiplier
        // above the flat 2 (`ShotsBonusFinalizer`).
        let shots_bonus = shots_bonus_of(world, attacker_oid);
        let (crit, damage, shield) = if miss {
            (false, 0, formulas::physical::SHIELD_NONE)
        } else {
            // Shield block (`calcShldUse`): a back attack (attacker outside the
            // 120° front arc) can't be blocked, and a **bow** attacker raises
            // the block rate by 30 % (`attacker.getAttackType().isRanged()`).
            // Java's `degreeside` is 360 rather than 120 while the defender is
            // affected by `PHYSICAL_SHIELD_ANGLE_ALL` (Aegis), which makes every
            // angle a front angle — so the back-attack exemption simply drops.
            let from_behind = matches!(position, crate::model::movement::Position::Back)
                && !crate::game_loop::abnormal::shields_from_all_angles(world, target_oid);
            let shield = formulas::physical::calc_shield_use(
                target.shield_rate,
                target.con_bonus,
                super::ranged::is_ranged(weapon_type),
                from_behind,
                world.roll(100),
                world.roll(100),
            );
            let crit_roll = world.roll(100);
            // `DEFENCE_CRITICAL_RATE`/`_ADD` are read off the **defender** — Light
            // Armor Mastery 233 makes its wearer harder to crit, it does not make
            // its wearer crit less.
            let (def_crit_mul, def_crit_add) = defence_crit_rate(world, target_oid);
            let crit = formulas::physical::calc_auto_attack_crit(
                attacker.crit_stat,
                def_crit_mul,
                def_crit_add,
                position,
                crit_rate_position_mul(world, attacker_oid, position),
                attacker.z,
                target.z,
                // `calcCrit`'s level term fires when either side is 78+.
                crate::game_loop::skills::effects::creature_level(world, attacker_oid),
                crate::game_loop::skills::effects::creature_level(world, target_oid),
                crit_roll,
            );
            let r = attacker.random_dmg;
            let rand_roll = if r > 0 { world.roll(2 * r + 1) - r } else { 0 };
            // A normal block adds the shield's defence to pDef; a perfect block
            // reduces the hit to 1 (Java `SHIELD_DEFENSE_PERFECT_BLOCK`).
            let eff_pdef = target.p_def
                + if shield == formulas::physical::SHIELD_SUCCEED {
                    target.shield_def
                } else {
                    0.0
                };
            let dmg = if shield == formulas::physical::SHIELD_PERFECT {
                1.0
            } else {
                // `calcAutoAttackDamage`'s own `damage *= calcAttackTraitBonus(...)`
                // — the weapon trait plus every group-2 weakness, which is what
                // makes the Hunter's "Detect … Weakness" line pay off.
                formulas::physical::calc_auto_attack_damage(
                    attacker.p_atk,
                    formulas::physical::random_damage_multiplier(rand_roll),
                    position,
                    eff_pdef,
                    crit,
                    crit_damage_auto(world, attacker_oid, target_oid, position),
                    ss,
                    shots_bonus,
                    // A bow/crossbow swings on Java's **154** weapon mod, and
                    // its crits split across both halves of the expression.
                    super::ranged::is_ranged(weapon_type),
                    // `calcAutoAttackDamage`'s own `damage *= calcAttackTraitBonus(...)`
                    // — the weapon trait plus every group-2 weakness, which is what
                    // makes the Hunter's "Detect … Weakness" line pay off.
                    crate::game_loop::skills::effects::calc_attack_trait_bonus(
                        world,
                        attacker_oid,
                        target_oid,
                    ),
                    // `calcAttributeBonus(attacker, target, **null**)`: with no
                    // skill to name an element the attacker's strongest POWER
                    // stat elects one, so an elemental weapon buff reaches
                    // plain swings too.
                    crate::game_loop::skills::effects::attribute_mod_no_skill(
                        world,
                        attacker_oid,
                        target_oid,
                    ),
                    // `calcAutoAttackDamage`'s own `pvpPveMod`, passed a **null
                    // skill** — so an auto-attack reads the PHYSICAL_ATTACK pair,
                    // never either skill pair.
                    crate::game_loop::skills::effects::pvp_pve_bonus(
                        world,
                        attacker_oid,
                        target_oid,
                        None,
                    ),
                )
            };
            // Java `generateHit`'s `halfDamage` — each dual hit is half a swing.
            let dmg = if is_dual { dmg / 2.0 } else { dmg };
            (crit, dmg as i32, shield)
        };
        // Notify a shielding player their block landed (Interlude has only the
        // "succeeded" message; the perfect block reuses it) — per hit, like
        // Java's `calcShldUse`.
        if shield != formulas::physical::SHIELD_NONE {
            helpers::send_sm_bare_to_player(world, target_oid, sm_ids::SHIELD_DEFENSE_SUCCEEDED);
        }
        rolled.push((miss, crit, damage, ss, shield));
    }
    let (miss, crit, damage, ss, _shield) = rolled[0];

    // `Hit.getGrade()`: the equipped weapon's crystal-grade ordinal, only when
    // a soulshot was actually spent.
    let ss_grade = if shot_consumed {
        world
            .data
            .item_data
            .get(weapon_id)
            .map(|t| t.crystal_type.level())
            .unwrap_or(0)
    } else {
        0
    };

    let mut hits: Vec<server_packets::AttackHit> = Vec::new();
    for &(miss, crit, damage, ss, _) in &rolled {
        hits.push(server_packets::AttackHit {
            target_object_id: target_oid,
            damage,
            miss,
            crit,
            soulshot: ss,
            ss_grade,
        });
    }
    for extra in sweep_targets(world, attacker_oid, target_oid, weapon_id) {
        hits.push(server_packets::AttackHit {
            target_object_id: extra,
            damage,
            miss,
            crit,
            soulshot: ss,
            ss_grade,
        });
    }

    let now = world.tick;
    // Anything that swings needs the component: it carries both the swing
    // period and the counter an abort invalidates the pending hit with, and a
    // creature that acquired one only lazily could not be aborted mid-swing.
    if !world.objects.has_component::<AttackState>(&attacker_oid) {
        world
            .objects
            .add_components(&attacker_oid, AttackState::default());
    }
    if let Some(st) = world
        .objects
        .get_component_mut::<AttackState>(&attacker_oid)
    {
        st.attack_end_tick = now + ms_to_ticks(time_atk);
    }
    // Stamped onto every hit of this swing so an abort can invalidate them.
    let swing_seq = world
        .objects
        .get_component::<AttackState>(&attacker_oid)
        .map_or(0, |st| st.swing_seq);
    for hit in &hits {
        world.scheduler.schedule(
            now + ms_to_ticks(time_to_hit),
            ScheduledTask::AttackHit {
                swing_seq,
                attacker: attacker_oid,
                target: hit.target_object_id,
                damage: hit.damage,
                miss: hit.miss,
                crit: hit.crit,
            },
        );
    }
    // Swing-end hook (Java's `EVT_READY_TO_ACT` schedule in `doAttack`).
    // Players: fire the action the swing held back, if any. NPCs: re-run the AI
    // think at the swing's end so it re-swings at the weapon's attack rate
    // instead of stalling until the coarse 1 s `AttackableAI` tick — the fix for
    // the "attack, pause a second, attack again" stutter (siege guards & mobs).
    if !is_npc_oid(attacker_oid) {
        world.scheduler.schedule(
            now + ms_to_ticks(time_atk),
            ScheduledTask::AttackFinish {
                object_id: attacker_oid,
            },
        );
    } else {
        world.scheduler.schedule(
            now + ms_to_ticks(time_atk),
            ScheduledTask::NpcAttackReady {
                npc_oid: attacker_oid,
            },
        );
    }

    // Broadcast the swing (all of its hits in one packet).
    let pkt = server_packets::attack(
        attacker_oid,
        &hits,
        attacker.x,
        attacker.y,
        attacker.z,
        target.x,
        target.y,
        target.z,
    );
    if is_npc_oid(attacker_oid) {
        let Some(region) = position::region_cell_of(world, attacker_oid) else {
            return;
        };
        broadcast::broadcast_near_region_in(
            world,
            region,
            helpers::instance_of(world, attacker_oid),
            &pkt,
        );
    } else {
        broadcast::broadcast_including_self(world, attacker_oid, &pkt);
    }

    // `Creature.doAttack` tail: outside a PVP zone, and not self-targeting, the
    // attacker enters stance and flags against a player target.
    //
    // Java hangs both off `getActingPlayer()`, which is **not** the same as
    // "the attacker is a player": a summon's acting player is its owner, so a
    // pet's swing flags and stances the *owner*. This block used to live in the
    // player-only `else` above, so a summon attacking a player flagged nobody —
    // a player could attack through their pet and never go purple, leaving the
    // victim unable to retaliate without taking the karma.
    //
    // A plain monster resolves to itself, is not a player, and so still flags
    // nobody.
    let actor = crate::game_loop::combat::pvp::acting_player(world, attacker_oid);
    if world.objects.has_component::<crate::model::Player>(&actor) {
        refresh_attack_stance(world, actor);
        crate::game_loop::combat::pvp::update_pvp_status_target(world, actor, target_oid);
    }
}

/// The polearm sweep's extra targets — Java's `ATTACK_COUNT_MAX` loop in
/// `generateAttackTargetData`.
///
/// Returns nothing at all unless the attacker's `ATTACK_COUNT_MAX` exceeds 1,
/// which on this dist means Polearm Mastery 216 (`HitNumber` 5). Candidates
/// must be alive, auto-attackable, inside the **weapon's** attack radius (66
/// for a polearm, 40 for most others) and within its attack angle of the
/// attacker's heading (120° for both — a weapon that declares no
/// `damage_range` falls back to angle 0, which selects nothing).
///
/// Java also skips the sweep when `PHYSICAL_POLEARM_TARGET_SINGLE > 0` — Focus
/// Attack (317), a toggle that trades the sweep for accuracy and crit damage.
/// Its two stat halves landed long before the sweep gate did, so until G34 S4
/// the toggle was a pure bonus with no cost.
fn sweep_targets(world: &World, attacker_oid: i32, main_target: i32, weapon_id: i32) -> Vec<i32> {
    let max_targets = helpers::stat_add(
        world,
        attacker_oid,
        crate::model::stats::Stat::AttackCountMax,
    ) as i32
        + 1; // the base 1 target
    if max_targets <= 1 {
        return Vec::new();
    }
    // Focus Attack: give up the sweep entirely.
    if world
        .objects
        .get_component::<crate::model::components::stats::StatModifiers>(&attacker_oid)
        .map(|m| {
            crate::model::stat_finalize::finalize(
                m,
                crate::model::stats::Stat::PhysicalPolearmTargetSingle,
                0.0,
            )
        })
        .unwrap_or(0.0)
        > 0.0
    {
        return Vec::new();
    }
    let Some(template) = world.data.item_data.get(weapon_id) else {
        return Vec::new();
    };
    let (radius, angle) = (template.attack_radius, template.attack_angle);
    if radius <= 0 || angle <= 0 {
        return Vec::new();
    }
    let Some(origin) = maybe_position(world, attacker_oid) else {
        return Vec::new();
    };
    let Some(region) = position::region_cell_of(world, attacker_oid) else {
        return Vec::new();
    };
    let heading_deg = origin.heading as f64 * 360.0 / 65536.0;

    let mut out = Vec::new();
    let mut room = max_targets - 1; // the main target already used one
    for candidate in world.npcs_visible_from(region) {
        if room <= 0 {
            break;
        }
        if candidate == main_target || candidate == attacker_oid {
            continue;
        }
        if vitals_of(world, candidate).map(|v| v.dead).unwrap_or(true) {
            continue;
        }
        // Only auto-attackable creatures are swept up (Java `isAutoAttackable`).
        let attackable =
            npc::npc_template(world, candidate).is_some_and(|t| t.is_auto_attackable());
        if !attackable {
            continue;
        }
        let Some(pos) = maybe_position(world, candidate) else {
            continue;
        };
        let (dx, dy) = ((pos.x - origin.x) as f64, (pos.y - origin.y) as f64);
        if (dx * dx + dy * dy).sqrt() > radius as f64 {
            continue;
        }
        // `Math.abs(calculateDirectionTo(obj) - headingAngle) > angle` → skip.
        let direction = dy.atan2(dx).to_degrees();
        let mut delta = (direction - heading_deg).abs() % 360.0;
        if delta > 180.0 {
            delta = 360.0 - delta;
        }
        if delta > angle as f64 {
            continue;
        }
        out.push(candidate);
        room -= 1;
    }
    out
}

/// `ScheduledTask::AttackHit` — `onHitTimeNotDual` + `onHitTarget`: the swing
/// lands (or misses).
/// Java `Creature.abortAttack()` — the swing already in flight never lands.
///
/// Java cancels the scheduled hit through its task handle. The port's
/// scheduler cannot cancel, so this bumps the attacker's swing counter and the
/// stale hit is dropped when it fires (see `AttackState::swing_seq`). The
/// observable behaviour is the same: no damage, no hit packet, no aggro.
///
/// A creature with no `AttackState` has never swung, so there is nothing to
/// abort — the miss is not silent, it is vacuous.
pub(crate) fn abort_attack(world: &mut World, object_id: i32) {
    if let Some(st) = world.objects.get_component_mut::<AttackState>(&object_id) {
        st.swing_seq = st.swing_seq.wrapping_add(1);
    }
}

pub(crate) fn handle_attack_hit(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: i32,
    miss: bool,
    crit: bool,
    swing_seq: u64,
) {
    // Java `abortAttack()` cancels the scheduled hit; the port drops it here
    // instead, because a heap-backed scheduler has no cancel handle. A stale
    // seq means the swing was aborted after this hit was queued — by a stun,
    // a paralyze, or anything else that calls `abort_attack`.
    if world
        .objects
        .get_component::<AttackState>(&attacker)
        .is_some_and(|st| st.swing_seq != swing_seq)
    {
        return;
    }
    // Attacker died mid-swing → EVT_CANCEL (nothing lands).
    let attacker_alive = vitals_of(world, attacker).map(|v| !v.dead);
    if attacker_alive != Some(true) {
        return;
    }
    // A siege-gate swing (`do_door_swing`): doors have no `Vitals`, so they
    // take their own branch — re-checking the siege gate, since the hit can
    // outlive the siege ending mid-swing.
    if world
        .objects
        .has_component::<crate::model::door::Door>(&target)
    {
        if crate::game_loop::siege::attackable_door(world, target) {
            apply_door_damage(world, target, damage);
        }
        return;
    }
    // Target dead/gone → skipped (Java's per-hit target check).
    let target_alive = vitals_of(world, target).map(|v| !v.dead);
    if target_alive != Some(true) {
        return;
    }

    if miss {
        // `sendDamageMessage(miss)` + `notifyAttackAvoid`.
        if let Some(client_id) = helpers::client_for_player(world, attacker) {
            let name = world
                .objects
                .get_component::<crate::model::Player>(&attacker)
                .expect("player")
                .name
                .clone();
            helpers::send_to_client(
                world,
                client_id,
                server_packets::system_message_with(
                    sm_ids::C1_S_ATTACK_WENT_ASTRAY,
                    &[SmParam::PlayerName(name)],
                ),
            );
        }
        if let Some(client_id) = helpers::client_for_player(world, target) {
            let attacker_name = attacker_display_name(world, attacker);
            helpers::send_to_client(
                world,
                client_id,
                server_packets::system_message_with(
                    sm_ids::C1_HAS_EVADED_C2_S_ATTACK,
                    &[
                        SmParam::PlayerName(
                            world
                                .objects
                                .get_component::<crate::model::Player>(&target)
                                .expect("player")
                                .name
                                .clone(),
                        ),
                        attacker_name,
                    ],
                ),
            );
            refresh_attack_stance(world, target);
        }
        return;
    }

    // Crit + damage messages (`Player.sendDamageMessage`).
    if let Some(client_id) = helpers::client_for_player(world, attacker) {
        let attacker_name = world
            .objects
            .get_component::<crate::model::Player>(&attacker)
            .expect("player")
            .name
            .clone();
        // How the *target* shows up in the attacker's damage messages ($c2)
        // — the same name rule the attacker gets in the target's messages.
        let target_name = attacker_display_name(world, target);
        // `Player.sendDamageMessage`: an invul / HP-blocked target silently
        // absorbs the hit — the attacker is told "The attack has been blocked",
        // NOT the damage line (which would falsely claim damage that never
        // lands). Matches Java's `target.isInvul()` branch ahead of the
        // `C1_HAS_INFLICTED` line.
        let target_blocked = world
            .objects
            .get_component::<crate::model::components::player::AdminFlags>(&target)
            .is_some_and(|f| f.invul);
        if crit {
            helpers::send_sm_to_client(
                world,
                client_id,
                sm_ids::C1_LANDED_A_CRITICAL_HIT,
                &[SmParam::PlayerName(attacker_name.clone())],
            );
        }
        if target_blocked {
            helpers::send_sm_bare_to_client(world, client_id, sm_ids::THE_ATTACK_HAS_BEEN_BLOCKED);
        } else {
            helpers::send_sm_to_client(
                world,
                client_id,
                sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
                &[
                    SmParam::PlayerName(attacker_name),
                    target_name,
                    SmParam::Int(damage),
                    // `sendDamageMessage`'s `addPopup(target, attacker,
                    // -damage)` — the on-screen floating damage number.
                    SmParam::Popup {
                        target,
                        attacker,
                        damage: -damage,
                    },
                ],
            );
        }
    }

    // The auto-attack path is `doAttack` proper: vampiric absorb + reflect ride
    // along (`skill_magic = None` — there is no skill).
    apply_attack_damage(world, attacker, target, damage as f64, false, None);

    // Java `OnCreatureDamageDealt` — the event `TriggerSkillByAttack` listens
    // on. Fired after the damage lands, and only for a *normal* attack (this
    // is the autoattack path; `allowSkillAttack` defaults to false, so skill
    // hits would be rejected anyway).
    crate::game_loop::skills::effects::fire_attack_triggers(world, attacker, target, damage, crit);
    // The augment-option procs ride the same event (Java runs both loops in
    // `onHitTarget`): `ATTACK` on a normal hit, `CRITICAL` on a crit.
    crate::game_loop::skills::effects::fire_option_attack_triggers(world, attacker, target, crit);
}

/// How an attacker shows up in the *victim's* damage messages ($c2).
pub(crate) fn attacker_display_name(world: &World, attacker: i32) -> SmParam {
    if let Some(p) = world
        .objects
        .get_component::<crate::model::Player>(&attacker)
    {
        SmParam::PlayerName(p.name.clone())
    } else if let Some(t) = npc::npc_template(world, attacker) {
        SmParam::NpcName(t.id)
    } else {
        SmParam::Text(String::new())
    }
}
