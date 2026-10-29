//! `TriggerSkillByDamageDealt` — chance-on-hit skill triggers (G19).
//!
//! Sword/Blunt Weapon Mastery 205, Dagger Mastery 209 and Dance of Shadows 366
//! all carry one. Each fires a short self- or party-buff when the carrier lands
//! a qualifying hit; before this slice the effect was unparsed, so the masteries
//! were passives whose on-hit half did nothing.

use super::*;
use crate::game_loop::abnormal::has_buff;

use crate::model::components::skills::SkillBook;
use crate::model::skill::effects::{DamageTrigger, SkillEffect};

const PLAYER: i32 = 8001;
const CID: u32 = 1;
const MOB_ID: i32 = 48000;
const MOB_OID: i32 = NPC_OID;
const CARRIER: i32 = 9900;
const TRIGGERED: i32 = 9901;

fn trigger_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(MOB_ID);
    t.type_name = "Monster".into();
    t.name = "Dummy".into();
    t.level = 5;
    t.base_hp_max = 5000.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    (world, db, l)
}

/// Build a carrier skill holding one `TriggerSkillByDamageDealt`, plus the skill it
/// triggers (a plain 60s stat buff, so "did it fire" is just "is the buff up").
fn install(world: &mut World, effect: SkillEffect) {
    use crate::model::skill::Skill;
    use crate::model::skill::effects::StatModifierEffect;
    use crate::model::skill::target::OperateType;
    use crate::model::stats::{Stat, StatModifierType};
    let base = |id: i32, effects: Vec<SkillEffect>, abnormal_time: i32, op: OperateType| Skill {
        id,
        name: format!("T{id}"),
        operate_type: op,
        magic_level: 1,
        abnormal_time,
        abnormal_level: 1,
        abnormal_type: format!("TRIG{id}"),
        effects,
        ..Default::default()
    };
    world
        .data
        .skill_data
        .insert_for_test(base(CARRIER, vec![effect], 0, OperateType::Passive));
    world.data.skill_data.insert_for_test(base(
        TRIGGERED,
        vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Diff,
            amount: 10.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        })],
        60,
        OperateType::Active,
    ));
}

fn a_trigger(chance: i32, is_critical: bool) -> SkillEffect {
    dealt(
        DamageTrigger {
            chance,
            ..damage_trigger(TRIGGERED)
        },
        is_critical,
    )
}

/// A dealt trigger with Java's defaults for the hit-kind flags: normal attacks
/// only, no reflect, any weapon.
fn dealt(trigger: DamageTrigger, is_critical: bool) -> SkillEffect {
    SkillEffect::TriggerSkillByDamageDealt {
        trigger,
        is_critical,
        allow_normal_attack: true,
        allow_skill_attack: false,
        allow_reflect: false,
        allow_weapons: 0,
    }
}

fn know(world: &mut World, oid: i32) {
    world
        .objects
        .get_component_mut::<SkillBook>(&oid)
        .unwrap()
        .0
        .insert(CARRIER, 1);
}

fn triggered(world: &World, oid: i32) -> bool {
    has_buff(world, oid, TRIGGERED)
}

/// Land one normal hit of `damage`, critical or not.
fn hit(world: &mut World, damage: i32, crit: bool) {
    combat::handle_attack_hit(world, PLAYER, MOB_OID, damage, false, crit, 0);
}

// ---------------------------------------------------------------------------
// Firing
// ---------------------------------------------------------------------------

/// The baseline: a carrier with a 100% non-crit trigger fires on an ordinary
/// hit, and the triggered skill's buff really lands.
#[test]
fn a_qualifying_hit_fires_the_trigger() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);

    assert!(!triggered(&world, PLAYER), "nothing up before the hit");
    hit(&mut world, 50, false);
    assert!(
        triggered(&world, PLAYER),
        "the trigger fired and its buff landed"
    );
}

/// Without the carrier skill nothing fires — proving the trigger comes from the
/// attacker's skill book and not from the attack itself.
#[test]
fn an_attacker_without_the_carrier_triggers_nothing() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    // deliberately not `know(...)`

    hit(&mut world, 50, false);
    assert!(!triggered(&world, PLAYER), "no carrier, no trigger");
}

/// **`isCritical` is an equality test, not a minimum.** An `isCritical=false`
/// trigger fires only on *non*-crits; `isCritical=true` only on crits. Dance of
/// Shadows 366 ships one of each, so getting this backwards would silently
/// halve or double it.
#[test]
fn is_critical_matches_the_hit_exactly() {
    // isCritical=false: fires on a normal hit, not on a crit.
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);
    hit(&mut world, 50, true);
    assert!(
        !triggered(&world, PLAYER),
        "a non-crit trigger must not fire on a crit"
    );
    hit(&mut world, 50, false);
    assert!(triggered(&world, PLAYER), "but does on a normal hit");

    // isCritical=true: the mirror image.
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, true));
    know(&mut world, PLAYER);
    hit(&mut world, 50, false);
    assert!(
        !triggered(&world, PLAYER),
        "a crit trigger must not fire on a normal hit"
    );
    hit(&mut world, 50, true);
    assert!(triggered(&world, PLAYER), "but does on a crit");
}

/// `damage < minDamage` bails — a hit that scratches for less than the floor
/// never triggers.
#[test]
fn a_hit_below_min_damage_does_not_fire() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(
        &mut world,
        dealt(
            DamageTrigger {
                min_damage: 100,
                ..damage_trigger(TRIGGERED)
            },
            false,
        ),
    );
    know(&mut world, PLAYER);

    hit(&mut world, 99, false);
    assert!(!triggered(&world, PLAYER), "99 < minDamage 100");
    hit(&mut world, 100, false);
    assert!(
        triggered(&world, PLAYER),
        "100 meets it (the check is `<`, so the floor itself qualifies)"
    );
}

/// `chance == 0` is Java's explicit early bail, distinct from "rolled and lost".
#[test]
fn a_zero_chance_trigger_never_fires() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(0, false));
    know(&mut world, PLAYER);

    // 1-damage hits: the dummy has 100 HP, and a corpse fires nothing.
    for _ in 0..20 {
        hit(&mut world, 1, false);
    }
    assert!(!triggered(&world, PLAYER), "chance 0 never fires");
}

/// The refresh guard: Java re-casts only when the buff is absent or at a lower
/// level, so a trigger that is already up is not spammed on every swing.
#[test]
fn an_already_active_trigger_is_not_recast() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);

    hit(&mut world, 1, false);
    let after_first = pbuffs(&world, PLAYER);
    // 1-damage hits keep the 100 HP dummy alive for all of them.
    for _ in 0..5 {
        hit(&mut world, 1, false);
    }
    let after_many = pbuffs(&world, PLAYER);
    assert_eq!(
        after_first, after_many,
        "the buff is not re-applied while it is already up"
    );
}

/// A creature never triggers off hitting itself (`attacker == target` bails).
#[test]
fn a_self_hit_never_triggers() {
    let (mut world, _db, _l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    install(&mut world, a_trigger(100, false));
    know(&mut world, PLAYER);

    combat::handle_attack_hit(&mut world, PLAYER, PLAYER, 50, false, false, 0);
    assert!(!triggered(&world, PLAYER), "self-hits are excluded");
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The three learnable carriers parse with their real parameters, including the
/// `fromLevel="9"` gating the previous slice added — 205 and 209 only gain the
/// trigger from level 9.
#[test]
fn real_dist_carriers_parse() {
    let skills = dist::skills();

    let trigger_of = |id: i32, level: i32| {
        skills.get(id, level).and_then(|s| {
            s.effects.iter().find_map(|e| match e {
                SkillEffect::TriggerSkillByDamageDealt {
                    trigger,
                    is_critical,
                    allow_weapons,
                    ..
                } => Some((
                    trigger.chance,
                    trigger.skill_id,
                    *is_critical,
                    *allow_weapons != 0,
                )),
                _ => None,
            })
        })
    };

    // Sword/Blunt Weapon Mastery 205: 50% on a crit, sword/blunt only, casts 5604.
    assert_eq!(trigger_of(205, 9), Some((50, 5604, true, true)));
    // Dagger Mastery 209: 33% on a crit, dagger only, casts 5603.
    assert_eq!(trigger_of(209, 9), Some((33, 5603, true, true)));
    // Both are `fromLevel="9"`, so level 8 has no trigger at all.
    assert_eq!(trigger_of(205, 8), None, "the level gate still applies");
    assert_eq!(trigger_of(209, 8), None);
}

/// The skills these actually cast carry ported effects, so the trigger has a
/// visible result rather than landing an empty buff: Dagger Mastery's 5603
/// grants `FatalBlowRate` for 5 seconds.
#[test]
fn the_triggered_skills_carry_real_effects() {
    let skills = dist::skills();
    let dagger = skills.get(5603, 1).expect("Dagger Mastery buff loads");
    assert!(
        dagger
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::StatModifier(m) if m.stat == Stat::BlowRate)),
        "5603 grants FatalBlowRate: {:?}",
        dagger.effects
    );
    assert_eq!(dagger.abnormal_time, 5, "for 5 seconds");
}

// ---------------------------------------------------------------------------
// Salvation's full parameter set
// ---------------------------------------------------------------------------

/// Register `id` at `level` as a plain 60 s pump, the shape every triggered
/// skill below takes, with `edit` applied on top.
fn pump(
    world: &mut World,
    id: i32,
    level: i32,
    edit: impl FnOnce(&mut crate::model::skill::Skill),
) {
    use crate::model::skill::Skill;
    use crate::model::skill::effects::StatModifierEffect;
    use crate::model::stats::StatModifierType;
    let mut s = Skill {
        id,
        level,
        name: format!("T{id}"),
        magic_level: 1,
        abnormal_time: 60,
        abnormal_level: level,
        abnormal_type: format!("TRIG{id}"),
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Diff,
            amount: 10.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        })],
        ..Default::default()
    };
    edit(&mut s);
    world.data.skill_data.insert_for_test(s);
}

fn armed(effect: SkillEffect) -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = trigger_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 40, 0, 0);
    install(&mut world, effect);
    know(&mut world, PLAYER);
    (world, db, l)
}

/// One `reduceCurrentHp` from the player onto the mob with an explicit hit
/// context — the shape skill, reflect and DoT damage arrive in.
fn land_hit(world: &mut World, hit: combat::DamageHit) {
    combat::apply_physical_damage(world, PLAYER, MOB_OID, 1.0, hit);
}

/// `allowNormalAttack`/`allowSkillAttack` split the two kinds of hit by
/// `skill == null`. The defaults (normal yes, skill no) ignore a skill hit;
/// a skill-only trigger is the mirror image.
#[test]
fn allow_flags_split_swings_from_skill_hits() {
    let (mut world, _db, _l) = armed(a_trigger(100, false));
    land_hit(&mut world, combat::DamageHit::SKILL);
    assert!(
        !triggered(&world, PLAYER),
        "default: a skill hit is ignored"
    );

    let skill_only = SkillEffect::TriggerSkillByDamageDealt {
        trigger: damage_trigger(TRIGGERED),
        is_critical: false,
        allow_normal_attack: false,
        allow_skill_attack: true,
        allow_reflect: false,
        allow_weapons: 0,
    };
    let (mut world, _db, _l) = armed(skill_only);
    hit(&mut world, 1, false);
    assert!(!triggered(&world, PLAYER), "skill-only: a swing is ignored");
    land_hit(&mut world, combat::DamageHit::SKILL);
    assert!(triggered(&world, PLAYER), "skill-only: a skill hit fires");
}

/// Reflected damage and DoT ticks are both excluded by default; `allowReflect`
/// lets the former through, nothing lets the latter.
#[test]
fn reflect_and_dot_hits_are_gated() {
    let reflect = combat::DamageHit {
        reflect: true,
        ..Default::default()
    };
    let (mut world, _db, _l) = armed(a_trigger(100, false));
    land_hit(&mut world, reflect);
    assert!(!triggered(&world, PLAYER), "reflect is off by default");
    land_hit(
        &mut world,
        combat::DamageHit {
            dot: true,
            ..Default::default()
        },
    );
    assert!(!triggered(&world, PLAYER), "a DoT tick never fires");

    let allow = SkillEffect::TriggerSkillByDamageDealt {
        trigger: damage_trigger(TRIGGERED),
        is_critical: false,
        allow_normal_attack: true,
        allow_skill_attack: false,
        allow_reflect: true,
        allow_weapons: 0,
    };
    let (mut world, _db, _l) = armed(allow);
    land_hit(&mut world, reflect);
    assert!(
        triggered(&world, PLAYER),
        "allowReflect admits reflected damage"
    );
}

/// The attacker-level window and `attackerType` both gate on the attacker.
#[test]
fn attacker_level_and_type_gate_the_attacker() {
    let (mut world, _db, _l) = armed(dealt(
        DamageTrigger {
            min_attacker_level: 99,
            ..damage_trigger(TRIGGERED)
        },
        false,
    ));
    hit(&mut world, 1, false);
    assert!(!triggered(&world, PLAYER), "below minAttackerLevel");

    let (mut world, _db, _l) = armed(dealt(
        DamageTrigger {
            attacker_type: crate::model::skill::effects::AttackerType::Attackable,
            ..damage_trigger(TRIGGERED)
        },
        false,
    ));
    hit(&mut world, 1, false);
    assert!(!triggered(&world, PLAYER), "a player is no Attackable");

    let (mut world, _db, _l) = armed(dealt(
        DamageTrigger {
            attacker_type: crate::model::skill::effects::AttackerType::Playable,
            ..damage_trigger(TRIGGERED)
        },
        false,
    ));
    hit(&mut world, 1, false);
    assert!(triggered(&world, PLAYER), "but is a Playable");
}

/// `targetType=TARGET` resolves from the attacker onto the creature it hit, so
/// the triggered buff lands on the mob rather than the player.
#[test]
fn target_type_target_lands_on_the_creature_hit() {
    let (mut world, _db, _l) = armed(dealt(
        DamageTrigger {
            target_type: TargetType::Target,
            ..damage_trigger(TRIGGERED)
        },
        false,
    ));
    hit(&mut world, 1, false);
    assert!(has_buff(&world, MOB_OID, TRIGGERED), "on the mob");
    assert!(!triggered(&world, PLAYER), "not on the attacker");
}

/// `triggerCast` spends the triggered skill's raw `reuseDelay` through
/// `disableSkill`: no re-fire until it lifts, and no reuse *timestamp* — so
/// nothing for `SkillCoolTime` or the save to carry.
#[test]
fn a_triggered_reuse_locks_the_skill_without_a_timestamp() {
    let (mut world, _db, _l) = armed(a_trigger(100, false));
    pump(&mut world, TRIGGERED, 1, |s| s.reuse_delay = 10_000);

    hit(&mut world, 1, false);
    assert!(triggered(&world, PLAYER), "first fire lands");
    let has_timestamp = world
        .objects
        .get_component::<crate::model::components::skills::Reuses>(&PLAYER)
        .is_some_and(|r| r.0.contains_key(&TRIGGERED));
    assert!(!has_timestamp, "disableSkill writes no reuse timestamp");

    crate::game_loop::skills::effects::expire_buffs_where(&mut world, PLAYER, |_, b| {
        b.skill_id == TRIGGERED
    });
    hit(&mut world, 1, false);
    assert!(!triggered(&world, PLAYER), "still disabled");

    world.tick += crate::scheduler::ms_to_ticks(10_000) + 1;
    hit(&mut world, 1, false);
    assert!(triggered(&world, PLAYER), "fires again once the lock lifts");
}

/// `triggerSkills` is a ladder: the target's current rung is silently removed
/// and the next one cast; standing on the last rung fires nothing.
#[test]
fn trigger_skills_climb_a_ladder() {
    const RUNG2: i32 = 9902;
    let (mut world, _db, _l) = armed(dealt(
        DamageTrigger {
            trigger_skills: vec![(TRIGGERED, 1), (RUNG2, 1)],
            ..damage_trigger(0)
        },
        false,
    ));
    pump(&mut world, RUNG2, 1, |_| {});

    hit(&mut world, 1, false);
    assert!(triggered(&world, PLAYER), "first rung");
    hit(&mut world, 1, false);
    assert!(!triggered(&world, PLAYER), "first rung removed…");
    assert!(has_buff(&world, PLAYER, RUNG2), "…for the second");
    hit(&mut world, 1, false);
    assert!(has_buff(&world, PLAYER, RUNG2), "the top rung stays");
    assert!(
        !triggered(&world, PLAYER),
        "and nothing restarts the ladder"
    );
}

/// `skillLevelScaleTo` steps the target's copy one level per fire, to the cap.
#[test]
fn skill_level_scale_to_steps_up_to_the_cap() {
    let (mut world, _db, _l) = armed(dealt(
        DamageTrigger {
            skill_level_scale_to: 3,
            ..damage_trigger(TRIGGERED)
        },
        false,
    ));
    for level in 1..=3 {
        pump(&mut world, TRIGGERED, level, |_| {});
    }
    let level =
        |world: &World| crate::game_loop::skills::cast::buff_level(world, PLAYER, TRIGGERED);
    let mut seen = Vec::new();
    for _ in 0..4 {
        hit(&mut world, 1, false);
        seen.push(level(&world));
    }
    assert_eq!(seen, vec![Some(1), Some(2), Some(3), Some(3)]);
}

/// A passive carrier whose `<passiveConditions>` fail never started its
/// effects in Java, so its trigger is not listening.
#[test]
fn a_passive_carrier_off_its_conditions_does_not_fire() {
    let (mut world, _db, _l) = armed(a_trigger(100, false));
    let mut carrier = world.data.skill_data.get(CARRIER, 1).cloned().unwrap();
    carrier.passive_conditions = vec![
        crate::model::skill::condition::SkillCondition::EquipWeapon {
            mask: crate::data::item_data::kinds::WeaponType::from_name("DAGGER").mask_bit(),
        },
    ];
    world.data.skill_data.insert_for_test(carrier);

    hit(&mut world, 1, false);
    assert!(!triggered(&world, PLAYER), "no dagger, no listener");
}

/// `TriggerSkillByDamageReceived` casts as the *bearer* when the fired skill is
/// itself `SELF`-targeted, and as the attacker otherwise — visible in whose
/// `disableSkill` the fired skill's reuse lands on.
#[test]
fn a_received_trigger_picks_its_caster_by_the_fired_skills_target_type() {
    use crate::model::components::skills::DisabledSkills;
    let locked = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<DisabledSkills>(&oid)
            .is_some_and(|d| d.0.contains_key(&TRIGGERED))
    };
    for (fired_type, caster) in [(TargetType::Self_, PLAYER), (TargetType::Target, MOB_OID)] {
        let (mut world, _db, _l) = armed(SkillEffect::TriggerSkillByDamageReceived {
            trigger: damage_trigger(TRIGGERED),
            hp_percent: 100,
        });
        pump(&mut world, TRIGGERED, 1, |s| {
            s.reuse_delay = 10_000;
            s.target_type = fired_type;
        });
        combat::apply_attack_damage(&mut world, MOB_OID, PLAYER, 1.0, false, false, None);
        assert!(
            triggered(&world, PLAYER),
            "{fired_type:?}: lands on the bearer"
        );
        assert!(locked(&world, caster), "{fired_type:?}: cast by {caster}");
    }
}

/// Java's order inside `Attackable.reduceCurrentHp`: hate first, then the
/// damage events, and only then `reduceHp`'s HP-block refusal. So a hit on an
/// HP-blocked mob still fires the attacker's trigger and still draws hate —
/// it just takes no HP.
#[test]
fn an_hp_blocked_mob_still_hears_the_hit() {
    let (mut world, _db, _l) = armed(a_trigger(100, false));
    let mut shield =
        crate::model::skill::active_buff::ActiveBuff::passive_pump(9999, 1, Vec::new());
    shield.effect_flags = crate::model::skill::effect_flag::HP_BLOCK;
    world
        .objects
        .get_component_mut::<crate::model::components::skills::Buffs>(&MOB_OID)
        .expect("mob buffs")
        .0
        .push(shield);
    let hp_before = nvit(&world, MOB_OID).cur_hp;

    hit(&mut world, 1, false);
    assert_eq!(nvit(&world, MOB_OID).cur_hp, hp_before, "no HP taken");
    assert!(triggered(&world, PLAYER), "the damage event still fired");
    let hate = world
        .objects
        .get_component::<AggroList>(&MOB_OID)
        .and_then(|a| a.0.get(&PLAYER).map(|i| i.hate))
        .unwrap_or(0.0);
    assert!(hate > 0.0, "and the hit still drew hate: {hate}");
}
