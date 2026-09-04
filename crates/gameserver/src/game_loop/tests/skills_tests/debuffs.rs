//! Debuffs: the land rate, resistances and traits, the damage-over-time
//! burst, and the damage block.

use super::*;

/// A single-target debuff (Decrease Speed 1160) that passes its landing roll
/// slows the mob server-side (base 120 × 0.80 = 96) and shows the caster the
/// computed landing chance. Against the level-5 test mob the rate constrains to
/// the 90 cap; the forced roll (0) is below it, so the debuff lands. The first
/// forced value feeds the unconditional magic-crit roll, the second the land roll.
#[test]
fn single_target_debuff_lands_and_reports_chance() {
    use model::components::stats::Speeds;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    let skill = world
        .data
        .skill_data
        .get(1160, 1)
        .expect("Decrease Speed")
        .clone();
    assert!(skill.is_bad() && skill.affect_scope == AffectScope::Single);
    world.force_rolls([0, 0]); // magic-crit roll, then land roll (0 < 90 → lands)
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    // Debuff applied to the mob: run speed recomputed to base 120 × 0.80 = 96.
    let speed = world
        .objects
        .get_component::<Speeds>(&npc_oid)
        .unwrap()
        .run_spd;
    assert!(
        (speed - 96.0).abs() < 1e-6,
        "run speed debuffed to 96, got {speed}"
    );

    // The caster sees the landed-outcome line (single-target only). It is now
    // this server's own message 9001, so the assertion is on which message was
    // sent rather than on a sentence we formatted.
    let msgs = drain(&mut a_rx);
    assert!(
        msgs.iter()
            .any(|p| sysmsg_id(p) == Some(S1_LANDED_ON_C2_CHANCE_WAS_S3::ID as i16)),
        "caster received the debuff-landed message",
    );
}

/// The same cast that fails its landing roll leaves the mob unslowed and sends
/// the caster the "<target> has resisted <skill>: X%" line. The land roll is
/// forced to 90, which is not below the 90 rate, so it resists.
#[test]
fn single_target_debuff_resisted_leaves_target_and_reports() {
    use model::components::stats::Speeds;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    let skill = world
        .data
        .skill_data
        .get(1160, 1)
        .expect("Decrease Speed")
        .clone();
    world.force_rolls([0, 90]); // magic-crit roll, then land roll (90 >= 90 → resisted)
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);

    // No debuff: run speed stays at the mob's base 120.
    let speed = world
        .objects
        .get_component::<Speeds>(&npc_oid)
        .unwrap()
        .run_spd;
    assert!(
        (speed - 120.0).abs() < 1e-6,
        "run speed unchanged on resist, got {speed}"
    );

    let msgs = drain(&mut a_rx);
    // The resisted-outcome line carries the target, skill and computed chance
    // as typed parameters (message 9000).
    assert!(
        msgs.iter()
            .any(|p| sysmsg_id(p) == Some(C1_HAS_RESISTED_S2_CHANCE_WAS_S3::ID as i16)),
        "caster received the debuff-resisted message",
    );
}

/// **The debuff's `<trait>` meets the target's `DefenceTrait`.** The dist's
/// stuns (Stun Attack 100, Shield Bash 352, …) declare `<trait>SHOCK`, which
/// the parse test above pins; here the fixture debuff borrows that trait so the
/// *consumption* side is visible. Against an unprotected mob the rate
/// constrains to the 90 cap, but a target made invulnerable to SHOCK drags the
/// same cast down to **0** — invulnerability skips the clamp, so the reported
/// chance drops all the way out of the retail range. That number is the proof
/// that `calcGeneralTraitBonus` reaches the landing roll.
#[test]
fn a_shock_debuff_is_scaled_by_the_targets_shock_defence() {
    use crate::game_loop::skills::effects::merge_defence_traits;
    use model::skill::traits::TraitType;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);

    let mut skill = world
        .data
        .skill_data
        .get(1160, 1)
        .expect("Decrease Speed")
        .clone();
    skill.trait_type = TraitType::Shock;

    // Unprotected: (35 - 5 + 3)·30 + 80 + 30 clamps to the 90 cap.
    world.force_rolls([0, 95]); // magic-crit roll, then a losing land roll
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let msgs = drain(&mut a_rx);
    assert!(
        msgs.iter().any(|p| {
            sysmsg_id(p) == Some(C1_HAS_RESISTED_S2_CHANCE_WAS_S3::ID as i16)
                && sysmsg_int(p) == Some(90)
        }),
        "unprotected, the stun is offered at the 90 cap",
    );

    // Invulnerable to SHOCK: the same cast is offered at the 10 floor.
    merge_defence_traits(&mut world, npc_oid, &[(TraitType::Shock, 1.0)]);
    world.force_rolls([0, 95]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let msgs = drain(&mut a_rx);
    assert!(
        msgs.iter().any(|p| {
            sysmsg_id(p) == Some(C1_HAS_RESISTED_S2_CHANCE_WAS_S3::ID as i16)
                && sysmsg_int(p) == Some(0)
        }),
        "SHOCK invulnerability refuses the debuff outright",
    );
}

/// **A resisted DoT does not burst.** Java puts the magic-crit burst in
/// `DamOverTime.onStart`, which `EffectList.add(info)` only reaches when
/// `calcEffectSuccess` passed — so a resisted poison deals nothing at all,
/// crit or no crit. The port used to apply the burst in the *instant* effect
/// pass, before the land roll, so a resisted debuff still hit for `power * 10`.
///
/// (Java carries an inline note that `M.Crit can occur even if this skill is
/// resisted` at that exact spot. It is aspirational — the shipped code does
/// not do it, and neither does this.)
#[test]
fn a_magic_crit_dot_bursts_only_when_the_debuff_lands() {
    let dot_skill = |world: &World| {
        let mut s = world.data.skill_data.get(1160, 1).expect("fixture").clone();
        s.id = 9610;
        s.name = "Test Poison".into();
        s.magic_type = 1;
        s.effects = vec![model::skill::effects::SkillEffect::DamOverTime {
            power: 5.0,
            ticks: 5,
            can_kill: false,
        }];
        s
    };
    let hp = |w: &World, oid: i32| w.objects.get_component::<Vitals>(&oid).unwrap().cur_hp;
    // `Npc::for_test` seeds a 1 000 000 HP pool, but the damage path's stat
    // recalculation clamps it to template 40001's real 100 — which alone would
    // read as 999 900 "damage". Start at that real max so the before/after
    // difference is the burst and nothing else.
    let normalise = |w: &mut World, oid: i32| {
        w.objects.get_component_mut::<Vitals>(&oid).unwrap().cur_hp = 100.0;
    };
    // The crit roll reads the *caster's* `m_crit_hit`; the fixture player has
    // none, so nothing would ever crit.
    let make_critter = |w: &mut World| {
        w.objects
            .get_component_mut::<CombatStats>(&3001)
            .unwrap()
            .m_crit_hit = 200.0;
    };

    // --- resisted: crit rolled, land roll lost ---
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
    let skill = dot_skill(&world);
    make_critter(&mut world);
    normalise(&mut world, npc_oid);
    let before = hp(&world, npc_oid);
    world.force_rolls([0, 90]); // crit, then 90 >= the 90 rate -> resisted
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    assert_eq!(
        hp(&world, npc_oid),
        before,
        "a resisted DoT deals no burst, even on a magic crit"
    );

    // --- landed: the same crit now bursts for power * 10 ---
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
    let skill = dot_skill(&world);
    make_critter(&mut world);
    normalise(&mut world, npc_oid);
    let before = hp(&world, npc_oid);
    world.force_rolls([0, 0]); // crit, then 0 < 90 -> lands
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    assert_eq!(
        before - hp(&world, npc_oid),
        50.0,
        "power 5 x 10 on the magic crit"
    );
}

/// G19 `AttackTrait` effect: "Detect Beast Weakness" (80, real dist data,
/// self-target, `abnormalTime` 600 s) lands as an icon-only buff — before
/// this slice the effect name wasn't recognized, so the whole "Detect
/// <Category> Weakness" family (Insect/Beast/Animal/Dragon/Plant, Eye of
/// Hunter/Slayer) fell through the empty-effects guard and never landed at
/// all (no icon, nothing). It's a peculiar effect to port: even on the real
/// Java server it's functionally inert (see the doc comment on
/// `SkillEffect::AttackTrait` — `Formulas.calcWeaknessBonus` needs a
/// matching NPC-side `DefenceTrait`, and nothing in this datapack ever sets
/// one), so this test only checks that the buff *lands and expires*
/// correctly, not any damage change.
#[test]
fn attack_trait_lands_as_an_icon_only_buff() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut rx = ingame_player_access(&mut world, 1, 5701, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5701)
        .unwrap()
        .0
        .insert(80, 1);
    world
        .objects
        .get_component_mut::<Vitals>(&5701)
        .unwrap()
        .cur_mp = 200.0;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(80, false));
    advance_world(&mut world, 30); // hitTime 1500 ms
    assert_eq!(
        pbuffs(&world, 5701),
        1,
        "Detect Beast Weakness lands as one buff"
    );

    world.tick += 6000; // abnormalTime 600 s
    apply_due_tasks(&mut world);
    assert_eq!(pbuffs(&world, 5701), 0, "expires after abnormalTime");
}

/// G19 `DamageBlock` effect: Celestial Shield (1418, real dist data,
/// `isMagic`, self-targetable via `targetType TARGET`) sets `HP_BLOCK` +
/// `MP_BLOCK` — previously silently dropped, so this and the other four
/// short invulnerability shields (Flames of Invincibility, Dance of Medusa,
/// Sonic/Force Barrier) did nothing on cast. `HP_BLOCK`'s real consumer is
/// `game_loop::combat::apply_physical_damage`'s new choke-point gate: a huge
/// non-DoT hit does nothing while the shield is up, but a DoT tick (Java's
/// one exemption besides a skill's own HP cost) still lands.
#[test]
fn damage_block_refuses_incoming_hp_damage_except_a_dot() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    // isMagic: needs a real magic casting speed, like the EnemyNot slice's
    // own test found — a level-1 default (Fighter, class 0) stretches the
    // nominal 4 s cast into minutes.
    let mut chr = dummy_char(5801, "Shielded");
    chr.class_id = 10;
    chr.base_class_id = 10;
    chr.skills = vec![(1418, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));
    world
        .objects
        .get_component_mut::<Vitals>(&5801)
        .unwrap()
        .cur_mp = 200.0;
    drain(&mut rx);

    handle_action(&mut world, 1, &action_body(5801, 0)); // self-target
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1418, false));
    advance_world(&mut world, 60); // hitTime 4000 ms

    assert_eq!(
        pbuffs(&world, 5801),
        1,
        "Celestial Shield lands as one buff"
    );
    assert_eq!(
        world.objects.get_component::<Buffs>(&5801).unwrap().0[0].effect_flags
            & (model::skill::effect_flag::HP_BLOCK | model::skill::effect_flag::MP_BLOCK),
        model::skill::effect_flag::HP_BLOCK | model::skill::effect_flag::MP_BLOCK,
        "both HP_BLOCK and MP_BLOCK set"
    );

    // Zero CP so a landed hit reduces HP directly, not absorbed by CP first
    // (the synthetic attacker oid below reads as "playable", which triggers
    // that absorb branch in `player_receive_damage`).
    world
        .objects
        .get_component_mut::<PlayerVitals>(&5801)
        .unwrap()
        .cur_cp = 0.0;
    let hp_before = pvit(&world, 5801).cur_hp;
    // A huge non-DoT hit: refused outright.
    combat::apply_physical_damage(&mut world, 90001, 5801, 999_999.0, false, false);
    assert_eq!(
        pvit(&world, 5801).cur_hp,
        hp_before,
        "HP_BLOCK refuses a normal hit"
    );
    // A DoT tick: Java's one exemption besides a skill's own HP cost.
    combat::apply_physical_damage(&mut world, 90001, 5801, 5.0, true, false);
    assert_eq!(
        pvit(&world, 5801).cur_hp,
        hp_before - 5.0,
        "a DoT tick still lands through HP_BLOCK"
    );
}

/// **A trait resistance makes you harder to execute.** Java's `Lethal` scales
/// both kill chances by `calcAttributeBonus * calcGeneralTraitBonus(…, false)`;
/// the port applied only the attribute half, with a comment saying the trait
/// half "stays unported with the trait system". The trait system landed, and
/// nothing existed to make that claim fail.
#[test]
fn a_trait_resistance_lowers_the_lethal_chance() {
    use model::skill::effects::SkillEffect;
    use model::skill::traits::TraitType;

    let lethal = |resist: bool| {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = spawn_debuff_target(&mut world, &mut a_rx);
        let max = world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .max_hp as f64;
        world
            .objects
            .get_component_mut::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp = max;

        let mut skill = world.data.skill_data.get(1160, 1).expect("fixture").clone();
        skill.id = 9900;
        skill.magic_type = 0;
        skill.activate_rate = -1;
        skill.magic_level = 80; // well above the mob, so the level gate passes
        skill.trait_type = TraitType::Shock;
        skill.effects = vec![SkillEffect::Lethal {
            full_lethal: 100.0,
            half_lethal: 0.0,
        }];
        if resist {
            effects::merge_defence_traits(&mut world, npc_oid, &[(TraitType::Shock, 0.5)]);
        }
        // magic-crit throwaway, then the full-lethal roll: 60 is under the
        // unresisted 100 but over the halved 50.
        world.force_rolls([0, 60]);
        effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
        world
            .objects
            .get_component::<Vitals>(&npc_oid)
            .unwrap()
            .cur_hp
    };

    assert_eq!(lethal(false), 1.0, "unresisted, the lethal lands");
    assert!(
        lethal(true) > 1.0,
        "a 50% SHOCK resistance halves the kill chance and the same roll misses"
    );
}
