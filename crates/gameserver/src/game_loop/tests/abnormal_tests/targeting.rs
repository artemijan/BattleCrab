//! Effects that move or refuse a target: target cancel and the resistances
//! that lower it, target me, locked targets, untargetable, the undead aura.

use super::*;

/// `TargetCancel` drops the victim's target and aborts what they were doing.
#[test]
fn target_cancel_clears_the_target_and_aborts() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    // The victim is targeting the mob and casting.
    world
        .objects
        .get_component_mut::<TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    use_magic(&mut world, VICTIM_CID, VICTIM, 91, false, false);
    assert!(world.objects.has_component::<Casting>(&VICTIM));
    drain(&mut vout);

    // `TargetCancel` rolls through `calcProbability` (`magicLevel + chance −
    // targetLevel`), so even a 100-chance skill has a threshold below 100 and
    // an unforced roll makes this flaky. Force the magic-crit throwaway and a
    // winning probability roll.
    world.force_rolls([0, 0]);
    land(&mut world, TCANCEL_ID, VICTIM);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
        None,
        "the target is dropped"
    );
    assert!(
        !world.objects.has_component::<Casting>(&VICTIM),
        "and the cast is aborted"
    );
}

/// A 0% `TargetCancel` does nothing — proof the chance roll is consulted.
#[test]
fn zero_chance_target_cancel_does_nothing() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9315,
        SkillEffect::TargetCancel { chance: 0 },
        "NONE",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    world
        .objects
        .get_component_mut::<TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    land(&mut world, 9315, VICTIM);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
        Some(NPC_OID),
        "a 0% target-cancel leaves the target alone"
    );
}

// ---------------------------------------------------------------------------
// Abnormal visual effects
// ---------------------------------------------------------------------------

/// **An invincible target is never shaken off its mark.** Java's
/// `TargetCancel.calcSuccess` vetoes on `ABNORMAL_INVINCIBILITY`,
/// `INVINCIBILITY_SPECIAL` or `INVINCIBILITY` *before* the chance is rolled —
/// so no amount of Shield Bash moves a target under Celestial Shield.
#[test]
fn an_invincible_target_ignores_target_cancel() {
    for abnormal in [
        "ABNORMAL_INVINCIBILITY",
        "INVINCIBILITY_SPECIAL",
        "INVINCIBILITY",
    ] {
        let (mut world, _db, _l) = cc2_world();
        let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
        let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
        world
            .objects
            .get_component_mut::<TargetRef>(&VICTIM)
            .unwrap()
            .0 = Some(NPC_OID);

        // A buff whose only relevant property is its abnormal type.
        world
            .data
            .skill_data
            .insert_for_test(cc_skill(9320, SkillEffect::BlockControl, abnormal));
        land(&mut world, 9320, VICTIM);

        land(&mut world, TCANCEL_ID, VICTIM);
        assert_eq!(
            world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
            Some(NPC_OID),
            "{abnormal} vetoes the cancel"
        );
    }
}

/// **The victim's level counts.** Java rolls `TargetCancel` through
/// `Formulas.calcProbability` (`magicLevel + chance − targetLevel`), not
/// against the raw percentage — so the same 100 %-on-paper Shield Bash slides
/// off a target far above the skill's magic level.
#[test]
fn target_cancel_slides_off_a_much_higher_level_target() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
    world
        .objects
        .get_component_mut::<TargetRef>(&VICTIM)
        .unwrap()
        .0 = Some(NPC_OID);
    // `cc_skill` carries `magic_level: 0` and the fixture chance is 100, so the
    // threshold is `0 + 100 - level`: put the victim past it.
    world
        .objects
        .get_component_mut::<Player>(&VICTIM)
        .unwrap()
        .level = 100;

    land(&mut world, TCANCEL_ID, VICTIM);
    assert_eq!(
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0,
        Some(NPC_OID),
        "a level-100 target keeps its mark against a magic-level-0 skill"
    );
}

/// **The trait resistance reaches the roll.** `calcProbability` multiplies the
/// threshold by `calcGeneralTraitBonus(…, ignoreResistance = false)`, so a
/// victim resisting the skill's trait shrugs off a `TargetCancel` that would
/// otherwise land. (`calcAttributeBonus` rides the same call, on the line
/// above.)
#[test]
fn a_trait_resistance_lowers_the_target_cancel_chance() {
    use crate::model::skill::traits::TraitType;

    let cancel = |resist: bool| {
        let (mut world, _db, _l) = cc2_world();
        let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
        let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
        world
            .objects
            .get_component_mut::<TargetRef>(&VICTIM)
            .unwrap()
            .0 = Some(NPC_OID);
        // Give the cancel a trait the victim can resist.
        let mut skill = world.data.skill_data.get(TCANCEL_ID, 1).unwrap().clone();
        skill.trait_type = TraitType::Shock;
        world.data.skill_data.insert_for_test(skill.clone());
        if resist {
            effects::merge_defence_traits(&mut world, VICTIM, &[(TraitType::Shock, 0.5)]);
        }
        // Threshold is `0 + 100 - level` (~99 here); halve it and a 60 roll
        // stops landing.
        world.force_rolls([0, 60]);
        effects::apply_skill_effects(&mut world, CASTER, VICTIM, &skill);
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0
    };

    assert_eq!(cancel(false), None, "unresisted, the cancel lands");
    assert_eq!(
        cancel(true),
        Some(NPC_OID),
        "a 50% SHOCK resistance halves the threshold and the same roll misses"
    );
}

/// And so does the **attribute** bonus, the sibling term: a victim resisting
/// the skill's element pulls the same threshold down.
#[test]
fn an_element_resistance_lowers_the_target_cancel_chance() {
    use crate::model::stats::{Element, Stat};

    let cancel = |resist: bool| {
        let (mut world, _db, _l) = cc2_world();
        let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
        let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);
        world
            .objects
            .get_component_mut::<TargetRef>(&VICTIM)
            .unwrap()
            .0 = Some(NPC_OID);
        let mut skill = world.data.skill_data.get(TCANCEL_ID, 1).unwrap().clone();
        skill.attribute_type = Some(Element::Fire);
        skill.attribute_value = 20;
        world.data.skill_data.insert_for_test(skill.clone());
        if resist {
            // A heavy fire resistance drags `calcAttributeBonus` below 1.
            let mods = world
                .objects
                .get_component_mut::<model::components::stats::StatModifiers>(&VICTIM)
                .unwrap();
            *mods.add.entry(Stat::FireRes).or_insert(0.0) += 300.0;
        }
        // `calcAttributeBonus` floors at 0.75, so the resisted threshold is
        // ~74 against ~99 unresisted — a roll of 80 separates them.
        world.force_rolls([0, 80]);
        effects::apply_skill_effects(&mut world, CASTER, VICTIM, &skill);
        world.objects.get_component::<TargetRef>(&VICTIM).unwrap().0
    };

    assert_eq!(cancel(false), None, "unresisted, the cancel lands");
    assert_eq!(
        cancel(true),
        Some(NPC_OID),
        "the fire resistance scaled the threshold under the same roll"
    );
}

/// `UNTARGETABLE` and `TARGETING_DISABLED` are the two halves of Java's one
/// gate in `Action`/`AttackRequest`: `(!obj.isTargetable() ||
/// player.isTargetingDisabled())`. The first sits on the *clicked* object, the
/// second on the *clicker* — swapping them would look identical in a
/// single-actor test, so both are asserted from both sides.
#[test]
fn untargetable_sits_on_the_target_and_targeting_disabled_on_the_clicker() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9325,
        SkillEffect::Untargetable,
        "UNTARGETABLE",
    ));
    world.data.skill_data.insert_for_test(cc_skill(
        9326,
        SkillEffect::DisableTargeting,
        "TARGETING_DISABLED",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, 9325, NPC_OID);
    assert!(abnormal::is_untargetable(&world, NPC_OID));
    assert!(
        !abnormal::is_targeting_disabled(&world, NPC_OID),
        "being unclickable does not stop you clicking"
    );

    land(&mut world, 9326, CASTER);
    assert!(abnormal::is_targeting_disabled(&world, CASTER));
    assert!(
        !abnormal::is_untargetable(&world, CASTER),
        "being unable to click does not make you unclickable"
    );
}

/// G34 S4 sub-slice 3 — `TargetMe` (Aggression 28, Aggression Aura 18) and
/// `TargetMeProbability` (Vengeance 368).
///
/// **Both are `if (effected.isPlayable())` in Java**, so taunting a *monster*
/// through them does nothing — a mob's aggro comes from the `AddHate`/`GetAgro`
/// effects the same skills carry. That is why Aggression declares both, and
/// why this asserts the monster case explicitly: implementing `TargetMe` as
/// "force any target" would look right in every player-vs-player test.
#[test]
fn target_me_locks_a_playable_and_ignores_a_monster() {
    let (mut world, _db, _l) = cc2_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9331, SkillEffect::TargetMe, "TARGET_ME"));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let victim = 5951;
    let _v = ingame_player_access(&mut world, 2, victim, 0);

    // A monster: nothing happens, no lock.
    land(&mut world, 9331, NPC_OID);
    assert!(
        !world
            .objects
            .has_component::<model::components::combat::LockedTarget>(&NPC_OID),
        "Java's isPlayable() guard means a mob is never locked by TargetMe"
    );

    // A player: target forced to the caster and locked there.
    land(&mut world, 9331, victim);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&victim)
            .and_then(|t| t.0),
        Some(CASTER),
        "the victim's selection is dragged onto the taunter"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::components::combat::LockedTarget>(&victim)
            .map(|l| l.0),
        Some(CASTER),
        "…and locked"
    );

    // `TargetMe.onExit` — the lock goes with the buff.
    effects::handle_buff_expire(&mut world, victim, 9331);
    assert!(
        !world
            .objects
            .has_component::<model::components::combat::LockedTarget>(&victim),
        "the lock must not outlive the taunt"
    );
}

/// The lock's whole purpose: `Npc.canTarget` refuses a *different NPC* while it
/// holds ("Failed to change enmity"). It is an NPC-side gate only — the victim
/// can still click players and items, which is what stops it from reading as a
/// blanket targeting freeze.
#[test]
fn a_locked_target_cannot_click_a_different_npc() {
    let (mut world, _db, _l) = cc2_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9332, SkillEffect::TargetMe, "TARGET_ME"));
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let other_npc = NPC_OID + 1;
    add_test_npc(&mut world, other_npc, 20001, "Monster", 5, 120, 0, 0);

    // Lock the caster onto the first mob, then try to click the second.
    world
        .objects
        .add_components(&CASTER, model::components::combat::LockedTarget(NPC_OID));
    drain(&mut out);
    handle_action(&mut world, CID, &action_body(other_npc, 0));
    let pkts = drain(&mut out);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::FAILED_TO_CHANGE_ENMITY),
        "the refusal is explained, not silent"
    );
    assert_ne!(
        world
            .objects
            .get_component::<TargetRef>(&CASTER)
            .and_then(|t| t.0),
        Some(other_npc),
        "and the selection did not move"
    );

    // The locked NPC itself is still clickable.
    handle_action(&mut world, CID, &action_body(NPC_OID, 0));
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&CASTER)
            .and_then(|t| t.0),
        Some(NPC_OID),
        "the taunter is exactly who you are allowed to click"
    );
}

/// **`UNDEAD_REAL_ENEMY`** — the priest anti-undead auras (Sanctuary 97, Holy
/// Aura 107, Repose 1034, Requiem 1049) are `SELF` + `POINT_BLANK`, so without
/// this filter they sweep *everything* nearby: friendly players and every
/// non-undead mob alike. That is what made it the one live correctness bug on
/// the affect axis rather than a missing nicety.
#[test]
fn an_undead_aura_spares_the_living_and_the_caster() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let ally = CASTER + 1;
    let _a = ingame_player(&mut world, CID + 1, ally, 40, 0, 0);

    let undead = NPC_OID;
    let living = NPC_OID + 1;
    add_test_npc(&mut world, undead, 90201, "Monster", 20, 60, 0, 0);
    add_test_npc(&mut world, living, 90202, "Monster", 20, 80, 0, 0);
    {
        let mut t = world.data.npc_data.get(90201).cloned().unwrap();
        t.race = Some(crate::enums::Race::Undead.ordinal());
        world.data.npc_data.insert_for_test(t);
    }

    let passes = |world: &World, oid: i32| {
        affect::passes_affect_object(world, CASTER, oid, AffectObject::UndeadRealEnemy)
    };

    assert!(
        passes(&world, undead),
        "an undead mob is the intended target"
    );
    assert!(!passes(&world, living), "a living mob is not");
    assert!(!passes(&world, ally), "and neither is a friendly player");
    assert!(
        !passes(&world, CASTER),
        "\"you are not an enemy of yourself\""
    );
}

/// **`OTHERS`** (Battle Stance 426, Spell Stance 427, Summon Friend 1403) — the
/// current selection, with exactly one rule: it may not be you, and Java
/// refuses with its own message rather than the generic invalid-target one.
#[test]
fn the_others_target_type_refuses_the_caster_with_its_own_message() {
    let skills = dist::skills();
    assert_eq!(
        skills.get(426, 1).unwrap().target_type,
        TargetType::Others,
        "Battle Stance is an OTHERS skill, not an unparsed fallback"
    );
}
