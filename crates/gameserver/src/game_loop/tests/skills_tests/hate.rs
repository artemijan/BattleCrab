//! Hate effects and the aggro a heal draws.

use super::*;

/// Regression: casting a *bad* skill at a monster must aggro it — the mob's AI
/// wakes and switches to the attack intention — **even when the debuff is
/// resisted**. Java `SkillCaster.callSkill` runs `addDamageHate(caster, 0,
/// -effectPoint)` + `notifyEvent(EVT_ATTACKED)` for every bad skill on an
/// attackable, right after `activateSkill` and independent of whether the
/// effects landed. The port used to wake the mob only from the damage/spoil
/// effect handlers, so a pure or resisted debuff never made the monster
/// retaliate ("when using a debuff and it doesn't land, the monster doesn't
/// attack back"). This drives the full network cast path (where the fix lives,
/// in `handle_skill_finish`) and forces the land roll to fail.
#[test]
fn resisted_debuff_still_aggros_monster() {
    use model::components::skills::SkillBook;
    use model::npc::{AggroList, NpcAi, NpcIntention};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    // Teach the caster Decrease Speed (1160) so the network cast path accepts it.
    world
        .objects
        .get_component_mut::<SkillBook>(&3001)
        .unwrap()
        .0
        .insert(1160, 1);

    // Target the mob, then cast the debuff, forcing the resist (crit roll 0,
    // land roll 90 ≥ the 90 rate → resisted, as in `single_target_debuff_
    // resisted_leaves_target_and_reports`).
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);
    world.force_rolls([0, 90]);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1160, false));

    // Run the cast to completion (launch + finish phases).
    advance_ticks(&mut world, 60);

    // The debuff resisted (the resist line fired above), yet the mob is now
    // attacking the caster: `callSkill` woke its AI + added hate regardless.
    let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
    assert_eq!(
        ai.intention,
        NpcIntention::Attack,
        "resisted debuff still wakes the mob"
    );
    let aggro = world.objects.get_component::<AggroList>(&npc_oid).unwrap();
    assert!(
        aggro.0.contains_key(&3001),
        "the caster is on the mob's aggro list"
    );
}

/// G19 hate-manipulation effects (`GetAgro`/`AddHate`/`DeleteHate`/
/// `DeleteHateOfMe`): before this slice all four effect names fell through
/// unregistered, so Aggression/Charm/Peace/Trick — and every other skill on
/// the same 6-effect family — cast but did nothing to the target's aggro
/// list. The underlying `AggroList`/`NpcAi` primitives were already ported
/// (used by combat/`faction_call`); these effects are thin wiring onto them.
mod hate_effects {
    use super::*;
    use model::npc::{AggroList, NpcAi, NpcIntention};
    use model::skill::effects::SkillEffect;

    const DECOY: i32 = 90001;

    /// Build a synthetic instant skill by cloning a known-good fixture skill
    /// (avoids repeating `Skill`'s ~35 fields) and swapping in the id/effect
    /// under test.
    fn hate_skill(world: &World, id: i32, name: &str, effect: SkillEffect) -> Skill {
        let mut skill = world
            .data
            .skill_data
            .get(1160, 1)
            .expect("fixture base")
            .clone();
        skill.id = id;
        skill.name = name.into();
        skill.effects = vec![effect];
        skill
    }

    /// `GetAgro` (Aggression 28/Aggression Aura 18/Judgment 401/Tribunal 400):
    /// the effected NPC intends to attack the caster, and the caster's hate
    /// becomes dominant over whoever it was already fighting — the ported
    /// AI re-derives its attack target from `AggroList::most_hated` every
    /// think tick, so "force intend-attack" has to mean "become the top
    /// entry," not just flipping the intention flag.
    #[test]
    fn get_agro_forces_the_npc_onto_the_caster() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        // A decoy already has strong hate — the NPC is mid-fight with someone else.
        add_hate(&mut world, npc_oid, DECOY, 500.0, 500.0);

        let skill = hate_skill(&world, 28, "Aggression", SkillEffect::GetAgro);
        effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

        let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
        assert_eq!(
            ai.intention,
            NpcIntention::Attack,
            "the mob intends to attack"
        );
        let aggro = world.objects.get_component::<AggroList>(&npc_oid).unwrap();
        let caster_hate = aggro.0.get(&3001).map(|i| i.hate).unwrap_or(0.0);
        let decoy_hate = aggro.0.get(&DECOY).map(|i| i.hate).unwrap_or(0.0);
        assert!(
            caster_hate > decoy_hate,
            "caster hate ({caster_hate}) must outrank the decoy ({decoy_hate})"
        );
    }

    /// `AddHate` (Charm 15/Lure 51): a flat hate change with no damage.
    /// Positive raises hate and wakes the AI; negative (unused on this dist,
    /// but Java supports it) lowers it, floored at 0.
    #[test]
    fn add_hate_raises_then_lowers_caster_hate() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

        let up = hate_skill(&world, 15, "Charm", SkillEffect::AddHate { power: 500.0 });
        effects::apply_skill_effects(&mut world, 3001, npc_oid, &up);
        assert_eq!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0[&3001]
                .hate,
            500.0
        );
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Attack,
            "positive power wakes the AI"
        );

        let down = hate_skill(&world, 15, "Charm", SkillEffect::AddHate { power: -800.0 });
        effects::apply_skill_effects(&mut world, 3001, npc_oid, &down);
        assert_eq!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0[&3001]
                .hate,
            0.0,
            "floored at 0, not negative"
        );
    }

    /// **An aggro-shedding skill must not wake the mob it just calmed.** Java
    /// gates `callSkill`'s `EVT_ATTACKED` notify on
    /// `!skill.hasEffectType(HATE)`; the port used to skip the gate (the note
    /// claimed no HATE effect was modelled, which stopped being true when
    /// `DeleteHate`/`DeleteHateOfMe` landed). The result was Bluff and Forget
    /// re-aggroing the mob on the same cast that made it forget you.
    ///
    /// The hate *addition* is not gated — Bluff really does carry
    /// `effectPoint -1`, so 1 hate is still added. Only the wake is skipped.
    #[test]
    fn a_hate_shedding_skill_does_not_wake_the_mob() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

        // An idle mob, and a Bluff-shaped skill: DeleteHateOfMe + effectPoint -1.
        let mut bluff = hate_skill(
            &world,
            9600,
            "Bluff",
            SkillEffect::DeleteHateOfMe { chance: 0 },
        );
        bluff.effect_point = -1;
        assert!(bluff.has_hate_effect());
        assert!(bluff.is_bad(), "still a bad skill, so it reaches the gate");

        apply_bad_skill_aggro_for_test(&mut world, 3001, npc_oid, &bluff);

        let ai = world.objects.get_component::<NpcAi>(&npc_oid).unwrap();
        assert_ne!(
            ai.intention,
            NpcIntention::Attack,
            "the mob was not woken by the skill that calmed it"
        );
        // The -effectPoint hate still landed.
        assert_eq!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0[&3001]
                .hate,
            1.0,
            "only the AI wake is suppressed, not the hate"
        );
    }

    /// The control: the very same call with an ordinary debuff *does* wake it.
    #[test]
    fn an_ordinary_bad_skill_still_wakes_the_mob() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

        let mut plain = hate_skill(&world, 9601, "Plain Debuff", SkillEffect::Root);
        plain.effect_point = -1;
        assert!(!plain.has_hate_effect());

        apply_bad_skill_aggro_for_test(&mut world, 3001, npc_oid, &plain);
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Attack,
        );
    }

    /// `DeleteHate` (Eva's Serenade 1273/Peace 1075/Repose 1034): a
    /// chance-rolled effect that wipes the target's *entire* aggro list and
    /// disengages its AI, even though only the caster cast the skill —
    /// whoever else was fighting it gets forgotten too (Java's own
    /// behaviour, not an approximation).
    #[test]
    fn delete_hate_wipes_the_whole_list_and_disengages() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        add_hate(&mut world, npc_oid, 3001, 50.0, 50.0);
        add_hate(&mut world, npc_oid, DECOY, 900.0, 900.0);
        world
            .objects
            .get_component_mut::<NpcAi>(&npc_oid)
            .unwrap()
            .intention = NpcIntention::Attack;
        // The first roll is `apply_skill_effects`' unconditional magic-crit
        // roll (999_999 → no crit, irrelevant here); the second is the
        // effect's own chance roll (0, well under the 80/100 chance).
        world.force_rolls([999_999, 0]);

        let skill = hate_skill(
            &world,
            1273,
            "Eva's Serenade",
            SkillEffect::DeleteHate { chance: 80 },
        );
        effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

        assert!(
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .unwrap()
                .0
                .is_empty(),
            "the whole list is wiped, not just the caster's entry"
        );
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Active,
            "the mob disengages"
        );
    }

    /// The gate on both hate wipes is **`calcSuccess`**, not a bare roll:
    ///
    /// ```java
    /// public boolean calcSuccess(Creature effector, Creature effected, Skill skill)
    /// {
    ///     return Formulas.calcProbability(_chance, effector, effected, skill);
    /// }
    /// ```
    ///
    /// So the declared `<chance>` is a *base* that the level difference moves —
    /// `(magicLevel + baseChance − targetLevel) − abnormalResist`, times the
    /// element and trait mods. The port rolled the base flat, which made Repose
    /// land on a level-80 boss exactly as often as on a level-1 rat.
    ///
    /// Six learnable skills ride it: Repose (1034), Peace (1075), Eva's
    /// Serenade (1273), Trick (11), Bluff (358) and Forget (1156).
    #[test]
    fn a_hate_wipe_is_gated_on_the_level_difference_not_a_flat_roll() {
        use crate::model::npc::Npc;

        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);

        let skill = {
            let mut s = hate_skill(
                &world,
                1034,
                "Repose",
                SkillEffect::DeleteHate { chance: 80 },
            );
            s.magic_level = 40;
            s
        };

        // `calcProbability` = `((magicLevel + chance) − targetLevel) − …`, so a
        // roll of 50 clears the level-5 fixture (40 + 80 − 5 = 115) and fails a
        // level-110 one (40 + 80 − 110 = 10). Nothing but the target's level
        // moves between the two casts.
        // The level lives on the **template**, not on the spawn — `for_test`'s
        // third argument is `x`.
        let mut high = world
            .data
            .npc_data
            .get(40001)
            .cloned()
            .expect("the fixture template");
        high.id = 40002;
        high.level = 110;
        world.data.npc_data.insert_for_test(high);

        let wiped_at_level = |world: &mut World, oid_offset: i32, npc_id: i32| -> bool {
            let npc_oid = NPC_OID + 30 + oid_offset;
            let (npc, extra) = Npc::for_test(npc_oid, npc_id, 0, 0, 0, 1_000_000, 30);
            world
                .npc_regions
                .entry(extra.1.0)
                .or_default()
                .push(npc_oid);
            world.objects.spawn(npc_oid, (npc, extra));
            add_hate(world, npc_oid, 3001, 50.0, 50.0);
            // Roll 1: the per-cast magic-crit roll. Roll 2: this effect's own.
            world.clear_forced_rolls();
            world.force_rolls([999_999, 50]);
            effects::apply_skill_effects(world, 3001, npc_oid, &skill);
            world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .is_some_and(|a| a.0.is_empty())
        };

        assert_eq!(
            (
                world.data.npc_data.get(40001).map(|t| t.level),
                world.data.npc_data.get(40002).map(|t| t.level)
            ),
            (Some(5), Some(110)),
            "the fixture's two templates have to straddle the roll"
        );
        assert!(
            wiped_at_level(&mut world, 0, 40001),
            "well below the caster's magic level, the base chance carries it"
        );
        assert!(
            !wiped_at_level(&mut world, 1, 40002),
            "far above it the same roll fails — a flat roll could not tell them apart"
        );
        let _ = &mut a_rx;
    }

    /// `DeleteHateOfMe` (Bluff 358/Forget 1156/Trick 11): chance-rolled,
    /// zeroes only the caster's own aggro entry — but, matching Java
    /// exactly, still disengages the AI wholesale even though the decoy's
    /// hate is untouched and still in the list.
    #[test]
    fn delete_hate_of_me_clears_only_the_casters_entry() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        add_hate(&mut world, npc_oid, 3001, 50.0, 50.0);
        add_hate(&mut world, npc_oid, DECOY, 900.0, 900.0);
        world
            .objects
            .get_component_mut::<NpcAi>(&npc_oid)
            .unwrap()
            .intention = NpcIntention::Attack;
        world.force_rolls([999_999, 0]);

        let skill = hate_skill(
            &world,
            358,
            "Bluff",
            SkillEffect::DeleteHateOfMe { chance: 80 },
        );
        effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

        let aggro = world.objects.get_component::<AggroList>(&npc_oid).unwrap();
        assert_eq!(
            aggro.0[&3001].hate, 0.0,
            "only the caster's own hate is zeroed"
        );
        assert_eq!(aggro.0[&DECOY].hate, 900.0, "the decoy's hate is untouched");
        assert_eq!(
            world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .unwrap()
                .intention,
            NpcIntention::Active,
            "the AI still disengages wholesale, matching Java"
        );
    }
}

/// **A beneficial skill cast near a fighting mob pulls it onto the caster** —
/// Java's "On Skill See logic", the rule that makes healing the tank aggro the
/// healer.
///
/// Two halves are being checked. The witness scan is Java's
/// `forEachVisibleObjectInRange(player, Npc.class, 1000, …)`, so the mob reacts
/// to a cast it was never a target of; and the hate is
/// `effectPoint * 150 / (level + 7)` credited to the caster. Until 2026-08-05
/// the port only notified the skill's own targets, so neither happened.
#[test]
fn healing_beside_a_fighting_mob_draws_its_hate_onto_the_healer() {
    use model::npc::{AggroList, NpcAi, NpcIntention};

    let (mut world, ..) = cast_test_world();
    // The healer, the tank it heals, and a mob already fighting the tank.
    let mut healer_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _tank_rx = ingame_caster(&mut world, 2, 3002, 60, 0);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 10, 80, 0, 0);
    world
        .objects
        .get_component_mut::<NpcAi>(&mob)
        .unwrap()
        .intention = NpcIntention::Attack;
    // The mob's target is the tank — never the healer.
    crate::game_loop::npc::minions::add_hate(&mut world, mob, 3002, 500.0);
    assert_eq!(
        world
            .objects
            .get_component::<AggroList>(&mob)
            .and_then(|a| a.most_hated()),
        Some(3002),
        "the mob is on the tank to begin with"
    );
    let hate_on_healer = |w: &World| -> f64 {
        w.objects
            .get_component::<AggroList>(&mob)
            .and_then(|a| a.0.get(&3001).map(|i| i.hate))
            .unwrap_or(0.0)
    };
    assert_eq!(
        hate_on_healer(&world),
        0.0,
        "the healer has drawn no hate yet"
    );

    // Heal the tank. The mob is not a target of the cast at all.
    set_target(&mut world, 1, 3001, Some(3002));
    drain(&mut healer_rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    advance_ticks(&mut world, 45);

    let after = hate_on_healer(&world);
    assert!(
        after > 0.0,
        "the mob noticed the heal and now hates the healer ({after})"
    );
}
