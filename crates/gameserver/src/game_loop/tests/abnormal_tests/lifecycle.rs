//! When an effect fires and stops: next-action chaining, a caster shrugging
//! one off, second-stage and end-effect firing, and the castable operate
//! types.

use super::*;

/// **The `operateType` fail-closed bug.** `use_magic_on` returns outright for
/// anything that is neither `Active` nor `Channeling`, and the parser dropped
/// every unmapped `operateType` to `Other` — so **A3** (Blinding Blow 321,
/// Vengeance 368, Evade Shot 369, Critical Blow 409, Aura Flare 1231) and
/// **CA5** (Battle Stance 426, Spell Stance 427) were seven learnable skills
/// that could not be cast at all.
///
/// Unlike the rest of this epic's gaps, this one fails *closed*: the skill did
/// nothing because the cast never started, not because an effect was missing.
#[test]
fn a3_and_ca5_skills_parse_as_castable_operate_types() {
    use crate::model::skill::target::OperateType;

    let skills = dist::skills();
    for (id, name, expected) in [
        (321, "Blinding Blow", OperateType::Active),
        (409, "Critical Blow", OperateType::Active),
        (1231, "Aura Flare", OperateType::Active),
        (426, "Battle Stance", OperateType::Channeling),
        (427, "Spell Stance", OperateType::Channeling),
    ] {
        let s = skills.get(id, 1).unwrap_or_else(|| panic!("{name} ({id})"));
        assert_eq!(
            s.operate_type, expected,
            "{name} ({id}) must be castable — `Other` makes `use_magic_on` bail"
        );
    }
    // A3 is continuous, which is read off the string and not this enum.
    assert!(
        skills.get(321, 1).unwrap().is_continuous,
        "A3 is one of Java's continuous types"
    );
}

/// **`<nextAction>`** — `SkillCaster.finishSkill`'s "attack target after skill
/// use" block, on **339** skills with `ATTACK` and 11 with `CAST`. Without it
/// every offensive skill *ends* your combat: you fire Power Strike and stand
/// there. Java gates it on a real target that is neither you nor
/// un-attackable.
#[test]
fn a_next_action_attack_skill_leaves_the_caster_swinging() {
    let skills = dist::skills();
    use crate::model::skill::NextAction;
    // Power Strike (3) is the archetype; Wind Strike (1177-style nukes) too.
    let ps = skills.get(3, 1).expect("Power Strike");
    assert_eq!(
        ps.next_action,
        NextAction::Attack,
        "Power Strike declares nextAction=ATTACK"
    );
    // And the tag is not universally set — a buff must not drag you into melee.
    let wind_walk = skills.get(1204, 1).expect("Wind Walk");
    assert_eq!(
        wind_walk.next_action,
        NextAction::None,
        "a self-buff has no next action"
    );
}

/// The behavioural half: a finished `ATTACK` cast has to leave a live attack
/// intent behind. A parsed tag that never reaches the intent is the failure
/// this epic keeps finding.
#[test]
fn finishing_a_next_action_cast_starts_the_attack_intent() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 20, 60, 0, 0);

    let mut strike = cc_skill(9450, SkillEffect::Root, "ROOT");
    strike.next_action = model::skill::NextAction::Attack;
    world.data.skill_data.insert_for_test(strike.clone());

    let intent_target = |world: &World| {
        world
            .objects
            .get_component::<Intent>(&CASTER)
            .and_then(|i| match i.0 {
                model::PlayerIntent::Attack { target_object_id } => Some(target_object_id),
                _ => None,
            })
    };
    assert_eq!(intent_target(&world), None, "not attacking to begin with");

    resume_action_after_cast_for_test(&mut world, CASTER, mob, 9450, 1);

    assert_eq!(
        intent_target(&world),
        Some(mob),
        "the cast leaves the caster swinging at its target"
    );
}

/// **`<abnormalResists>`** — `calcEffectSuccess`'s first resist clause: a
/// target part-way through a cast whose skill names this abnormal type shrugs
/// the debuff off outright, before any roll. That is what makes the long-ritual
/// skills uninterruptible; 176 skills on this dist declare a list.
#[test]
fn a_caster_shrugs_off_an_abnormal_its_own_cast_resists() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);

    // The ritual the victim is casting: immune to STUN while it runs.
    let mut ritual = cc_skill(9451, SkillEffect::Root, "NONE");
    ritual.abnormal_resists = vec!["STUN".into()];
    world.data.skill_data.insert_for_test(ritual);
    // The stun aimed at them.
    let mut stun = cc_skill(
        9452,
        SkillEffect::BlockActions { conditional: false },
        "STUN",
    );
    stun.is_debuff = true;
    stun.activate_rate = 100;
    world.data.skill_data.insert_for_test(stun.clone());

    let stunned = |world: &World| has_buff(world, victim, 9452);

    // Not casting: the stun lands.
    world.clear_forced_rolls();
    world.force_rolls([0; 8]);
    effects::apply_skill_effects(&mut world, CASTER, victim, &stun);
    assert!(stunned(&world), "an idle target takes the stun");

    // Mid-ritual: shrugged off before any roll.
    effects::handle_buff_expire(&mut world, victim, 9452);
    world.objects.add_components(
        &victim,
        Casting(model::CastState {
            skill_id: 9451,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: victim,
            seq: 1,
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );
    world.clear_forced_rolls();
    world.force_rolls([0; 8]);
    effects::apply_skill_effects(&mut world, CASTER, victim, &stun);
    assert!(
        !stunned(&world),
        "mid-ritual the same stun is resisted outright"
    );
}

/// **Anchor (1170) was doing half its job.** Its own description promises the
/// body goes "completely rigid for 5 seconds **and** causes paralysis for 5
/// seconds" — and that second half is an `<endEffects>` block firing
/// `CallSkill(6091)` when the first stage comes off. Neither the `END` effect
/// scope nor `CallSkill` existed, so the second stage never happened.
///
/// This is the last learnable gap G34's census had left, and it only surfaced
/// because the S8 gate forced every residual entry to be *examined* rather
/// than counted.
#[test]
fn anchors_second_stage_fires_when_the_first_expires() {
    let skills = dist::skills();
    let anchor = skills.get(1170, 1).expect("Anchor");
    assert_eq!(
        anchor.end_effects.len(),
        1,
        "the <endEffects> block is parsed, not dropped"
    );
    assert!(
        matches!(
            anchor.end_effects[0],
            SkillEffect::CallSkill { skill_id: 6091, .. }
        ),
        "and it calls Anchor's paralysis stage, got {:?}",
        anchor.end_effects[0]
    );
}

/// The behavioural half: the called skill has to actually land when the first
/// stage expires.
#[test]
fn an_end_effect_call_skill_lands_on_expiry() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = CASTER + 1;
    let _v = ingame_player(&mut world, CID + 1, victim, 40, 0, 0);

    // The second stage, and a first stage whose *end* calls it.
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9461, SkillEffect::Root, "PARALYZE"));
    let mut first = cc_skill(9460, SkillEffect::Root, "RIGID");
    first.end_effects = vec![SkillEffect::CallSkill {
        skill_id: 9461,
        skill_level: 1,
        chance: 100,
    }];
    world.data.skill_data.insert_for_test(first.clone());

    let has = |world: &World, id: i32| has_buff(world, victim, id);

    effects::apply_skill_effects(&mut world, CASTER, victim, &first);
    assert!(has(&world, 9460), "the first stage is up");
    assert!(!has(&world, 9461), "the second has not fired yet");

    effects::handle_buff_expire(&mut world, victim, 9460);

    assert!(!has(&world, 9460), "the first stage is gone");
    assert!(
        has(&world, 9461),
        "and its expiry fired the second — the half Anchor was missing"
    );
}
