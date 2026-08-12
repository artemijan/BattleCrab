use super::*;
use crate::data::dist;

/// `Skill.getName()` — `<skill name="…">` is parsed and kept per id, for
/// the messages that quote a skill back to the player. The name lives on
/// the `<skill>` element, so it must answer for every level of the skill,
/// not just level 1.
#[test]
fn skill_names_load_from_the_dist() {
    let sd = dist::skills();
    assert_eq!(sd.name(1177), Some("Wind Strike"));
    assert_eq!(sd.name(1), Some("Triple Slash"));
    // A level above 1 resolves through the same per-id entry (Wind Strike
    // is `toLevel="5"`).
    assert!(sd.get(1177, 5).is_some(), "sanity: Wind Strike 5 parses");
    assert!(sd.get(1177, 6).is_none(), "sanity: and 6 does not");
    assert_eq!(sd.name(1177), Some("Wind Strike"));

    // **The dist ships 15 skills declaring `name=""`.** They are stored as
    // "no name" rather than as an empty string, so a caller can choose its
    // own fallback instead of printing Java's literal "…casting ." Pinned
    // as a census: if this moves, the datapack changed, not the parser.
    let mut nameless: Vec<i32> = sd
        .skills
        .keys()
        .map(|(id, _)| *id)
        .filter(|id| sd.name(*id).is_none())
        .collect();
    nameless.sort_unstable();
    nameless.dedup();
    assert_eq!(
        nameless,
        vec![
            392, 393, 394, 397, 398, 399, 1377, 1378, 1379, 4217, 5103, 14579, 15241, 23401, 23483
        ],
        "the dist's blank-named skills"
    );
}

/// Skill-enchant sub-levels against the real dist (PLAN_G19_SKILL_ENCHANT.md).
/// Sonic Storm 7 at level 40 declares all three routes: route 1 enchants
/// the `EnergyAttack` power (`{base + base/100*subIndex}` off base 20732),
/// route 2 the crit chance (base 15 — itself a *ranged* `fromLevel 1–44`
/// row, the shape the parser used to mis-key), route 3 the pDefMod
/// (`{0.99 − 0.006·(subIndex−1)}`).
#[test]
fn skill_enchant_sublevels_resolve() {
    let sd = dist::skills();

    assert_eq!(
        sd.enchant_routes(7, 40),
        &[(1001, 1020), (2001, 2020), (3001, 3020)],
        "Sonic Storm 40's three routes"
    );
    assert!(
        sd.enchant_routes(7, 39).is_empty(),
        "the routes open at level 40"
    );
    assert!(
        sd.enchant_routes(1177, 1).is_empty(),
        "Wind Strike is not enchantable"
    );

    let base = sd.get(7, 40).expect("Sonic Storm 40");
    let (p0, c0, d0) = match base.effects.as_slice() {
        [
            SkillEffect::EnergyAttack {
                power,
                critical_chance,
                p_def_mod,
                ..
            },
        ] => (*power, *critical_chance, *p_def_mod),
        other => panic!("EnergyAttack expected: {other:?}"),
    };
    assert_eq!((p0, c0, d0), (20732.0, 15.0, 1.0));
    assert_eq!(base.sub_level, 0);

    // Route 1, +1 and +10: power scales, the other params hold their base.
    let e1 = sd.get_enchanted(7, 40, 1001).expect("+1 power route");
    assert_eq!(e1.sub_level, 1001);
    match e1.effects.as_slice() {
        [
            SkillEffect::EnergyAttack {
                power,
                critical_chance,
                p_def_mod,
                ..
            },
        ] => {
            assert!(
                (power - (20732.0 + 20732.0 / 100.0)).abs() < 1e-6,
                "+1: {power}"
            );
            assert_eq!((*critical_chance, *p_def_mod), (15.0, 1.0));
        }
        other => panic!("{other:?}"),
    }
    let e10 = sd.get_enchanted(7, 40, 1010).expect("+10 power route");
    match e10.effects.as_slice() {
        [SkillEffect::EnergyAttack { power, .. }] => {
            assert!((power - (20732.0 * 1.10)).abs() < 1e-6, "+10: {power}");
        }
        other => panic!("{other:?}"),
    }

    // Route 2 enchants the crit chance; route 3 the pDefMod.
    match sd
        .get_enchanted(7, 40, 2001)
        .expect("+1 crit route")
        .effects
        .as_slice()
    {
        [
            SkillEffect::EnergyAttack {
                power,
                critical_chance,
                ..
            },
        ] => {
            assert!((critical_chance - 15.15).abs() < 1e-6, "{critical_chance}");
            assert_eq!(*power, 20732.0, "power keeps its base on route 2");
        }
        other => panic!("{other:?}"),
    }
    match sd
        .get_enchanted(7, 40, 3005)
        .expect("+5 pdef route")
        .effects
        .as_slice()
    {
        [SkillEffect::EnergyAttack { p_def_mod, .. }] => {
            assert!(
                (p_def_mod - (0.99 - 0.006 * 4.0)).abs() < 1e-6,
                "{p_def_mod}"
            );
        }
        other => panic!("{other:?}"),
    }

    // A skill-FIELD route (not an effect param): Curse Gloom 1263's
    // duration route — `abnormalTime` base 10 (itself a ranged 1–24 row),
    // `{base + 0.5 * subIndex}` on 2001–2020. Java's `StatSet.getInt`
    // truncates the fractional +1 (10.5 → 10); +2 is a clean 11. The
    // fragmented power-route rows (1001–1005, 1006–1006, …) bucket-merge
    // into one (1001, 1020) route.
    assert_eq!(sd.enchant_routes(1263, 20), &[(1001, 1020), (2001, 2020)]);
    let cg = sd.get(1263, 20).expect("Curse Gloom 20");
    assert_eq!(cg.abnormal_time, 10);
    assert_eq!(
        sd.get_enchanted(1263, 20, 2001)
            .expect("+1 duration")
            .abnormal_time,
        10
    );
    assert_eq!(
        sd.get_enchanted(1263, 20, 2002)
            .expect("+2 duration")
            .abnormal_time,
        11
    );
    assert_eq!(
        sd.get_enchanted(1263, 20, 2020)
            .expect("+20 duration")
            .abnormal_time,
        20
    );

    // The cost table (data/EnchantSkillGroups.xml): 30 levels; +1 costs
    // 90% NORMAL with a Superior Giant's Codex 30297 and adena.
    let groups = crate::data::enchant_skill_groups::EnchantSkillGroups::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    assert_eq!(groups.len(), 30);
    let one = groups.cost_for(1).expect("level 1");
    assert_eq!(one.chance.get("NORMAL"), Some(&90));
    assert_eq!(one.sp.get("NORMAL"), Some(&4_250_000));
    let items = one.items.get("NORMAL").expect("NORMAL items");
    assert!(
        items.contains(&(30297, 1)) && items.contains(&(57, 2_380_000)),
        "{items:?}"
    );
}

/// Regression guard: the real dist XMLs are `<list>`-rooted, which the
/// original parser mis-indexed (it tracked the root on the tag stack and
/// loaded 0 skills). Wind Strike 1177 is the canonical probe.
#[test]
fn loads_real_dist_files() {
    let sd = dist::skills();
    assert!(
        sd.skills.len() > 10_000,
        "expected thousands of skill levels, got {}",
        sd.skills.len()
    );
    let ws = sd.get(1177, 1).expect("Wind Strike lvl 1");
    assert_eq!(ws.target_type, TargetType::EnemyOnly);
    assert_eq!(ws.cast_range, 600);
    assert!(
        matches!(ws.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 12.0)
    );
    assert_eq!(
        ws.reuse_delay_group, -1,
        "no <reuseDelayGroup> must stay -1, never 0"
    );
    assert_eq!(ws.reuse_key(), 1177);

    // Prominence 1230: a ranged nuke backed by the `MagicalAttackRange`
    // effect — `MagicalAttack` plus a shield roll (power 108 at lvl 28,
    // a block adding 40 % of the shield's def to m.def). Before the
    // handler existed the effect fell through and was dropped, so the
    // skill cast but dealt zero damage.
    let prominence = sd.get(1230, 28).expect("Prominence lvl 28");
    assert!(matches!(
        prominence.effects.as_slice(),
        [SkillEffect::MagicalAttackRange { power, shield_def_percent }]
            if *power == 108.0 && *shield_def_percent == 40.0
    ));

    // Power Strike 3: the canonical `PhysicalAttack` skill. Before the
    // handler existed every physical attack skill (1164 XML entries) cast
    // but dealt zero damage. Power 30 at lvl 1, default mods/crit chance.
    let power_strike = sd.get(3, 1).expect("Power Strike lvl 1");
    assert!(matches!(
        power_strike.effects.as_slice(),
        [SkillEffect::PhysicalAttack { power, p_atk_mod, p_def_mod, critical_chance, .. }]
            if *power == 30.0 && *p_atk_mod == 1.0 && *p_def_mod == 1.0 && *critical_chance == 10.0
    ));

    // Vampiric Touch 1147: an `HpDrain` skill — magic damage + 40% self-heal.
    // Before the handler existed it fell through and dealt no damage.
    let vampiric = sd.get(1147, 1).expect("Vampiric Touch lvl 1");
    assert!(matches!(
        vampiric.effects.as_slice(),
        [SkillEffect::HpDrain { power, percentage }] if *power == 18.0 && *percentage == 40.0
    ));

    // Dagger blows: Mortal Blow 16 (FatalBlow, crit-double, no flank),
    // Backstab 30 (flank-required), Shining Edge 505 (SoulBlow, no crit).
    let mortal_blow = sd.get(16, 1).expect("Mortal Blow lvl 1");
    assert!(matches!(
        mortal_blow.effects.as_slice(),
        [SkillEffect::Blow { power, chance_boost, critical_chance: Some(_), backstab: false }]
            if *power == 73.0 && *chance_boost == 200.0
    ));
    let backstab = sd.get(30, 1).expect("Backstab lvl 1");
    assert!(matches!(
        backstab.effects.first(),
        Some(SkillEffect::Blow { power, chance_boost, critical_chance: Some(cc), backstab: true })
            if *power == 1107.0 && *chance_boost == 400.0 && *cc == 5.0
    ));
    let shining_edge = sd.get(505, 1).expect("Shining Edge lvl 1");
    assert!(matches!(
        shining_edge.effects.first(),
        Some(SkillEffect::Blow { power, critical_chance: None, backstab: false, .. }) if *power == 1853.0
    ));

    // Decrease Speed 1160: single-target (`affectScope SINGLE`) bad skill
    // with a `Speed` PER -20% debuff, and the landing-rate inputs the
    // caster-feedback + resist roll read (`activateRate` 80, `lvlBonusRate` 30).
    let decrease_speed = sd.get(1160, 1).expect("Decrease Speed lvl 1");
    assert!(decrease_speed.affect_scope == AffectScope::Single && decrease_speed.is_bad());
    assert_eq!(decrease_speed.activate_rate, 80);
    assert_eq!(decrease_speed.lvl_bonus_rate, 30);
    // An area skill (`affectScope RANGE`) is not single-target.
    let sonic_storm = sd.get(7, 1).expect("Sonic Storm lvl 1");
    assert!(sonic_storm.affect_scope != AffectScope::Single);

    // Tempest 1176 — the canonical AoE nuke, and the reference case for the
    // whole affect-scope block: RANGE scope, NOT_FRIEND filter, a 200-unit
    // sweep around the target, and a 5-12 target cap.
    let tempest = sd.get(1176, 1).expect("Tempest lvl 1");
    assert_eq!(tempest.affect_scope, AffectScope::Range);
    assert_eq!(tempest.affect_object, AffectObject::NotFriend);
    assert_eq!(tempest.affect_range, 200);
    assert_eq!(tempest.affect_limit, (5, 12));
    // `getAffectLimit()` is `min + Rnd.get(max)`, so the "5-12" above can
    // actually yield up to 16 targets — verified at both roll extremes.
    assert_eq!(tempest.affect_limit(|_| 0), 5);
    assert_eq!(tempest.affect_limit(|bound| bound - 1), 16);
    // Sonic Storm carries the same 5-12 cap over a tighter 150 sweep.
    assert_eq!(sonic_storm.affect_range, 150);
    assert_eq!(sonic_storm.affect_limit, (5, 12));

    // Geometric scopes (PLAN_G19_GEOMETRIC_SCOPES.md). Sonic Buster 9 is
    // the reference FAN: a 180° half-circle of radius 200 —
    // `<fanRange>0;0;200;180</fanRange>` as `unk;startDegree;radius;angle`.
    let sonic_buster = sd.get(9, 1).expect("Sonic Buster lvl 1");
    assert_eq!(sonic_buster.affect_scope, AffectScope::Fan);
    assert_eq!(sonic_buster.fan_range, [0, 0, 200, 180]);
    // Divine Judgment 6314 — RING_RANGE: an annulus of 100..270 around
    // the target; the inner radius rides in `fan_range[2]`.
    let judgment = sd.get(6314, 1).expect("Divine Judgment lvl 1");
    assert_eq!(judgment.affect_scope, AffectScope::RingRange);
    assert_eq!(judgment.affect_range, 270);
    assert_eq!(judgment.fan_range, [0, 0, 100, 0]);
    // Frintezza Charge 5015 — SQUARE with a **level-valued** fanRange
    // (the only leveled tuple in the dist): 400×150 at level 1, 700×200
    // at level 3. A skill with no `<fanRange>` parses to all zeroes.
    assert_eq!(
        sd.get(5015, 1).expect("Frintezza Charge lvl 1").fan_range,
        [0, 0, 400, 150]
    );
    assert_eq!(
        sd.get(5015, 3).expect("Frintezza Charge lvl 3").fan_range,
        [0, 0, 700, 200]
    );
    assert_eq!(tempest.fan_range, [0; 4]);

    // Over-hit (G20): 59 learnable skills carry `<overHit>true</overHit>` —
    // a killing blow with one pays bonus XP for the excess damage.
    assert!(
        sd.get(1, 1).expect("Triple Slash").over_hit,
        "Triple Slash over-hits"
    );
    assert!(sd.get(7, 1).expect("Sonic Storm").over_hit);
    assert!(!sd.get(1068, 1).expect("Might").over_hit, "a buff does not");

    // Polearm Mastery 216 raises ATTACK_COUNT_MAX to 5 (`HitNumber`) —
    // this is what turns a polearm into a sweep weapon; the weapon type
    // alone does not.
    let mastery = sd.get(216, 1).expect("Polearm Mastery lvl 1");
    assert!(
        mastery
            .stat_modifier_effects()
            .iter()
            .any(|m| m.stat == Stat::AttackCountMax && m.amount == 5.0),
        "got {:?}",
        mastery.effects
    );

    // Abnormal *visual* effects — the cosmetic half of everything above.
    // Shield Stun 92 draws STUN(7), Bleed 96 draws DOT_BLEEDING(1), Horror
    // 65 draws TURN_FLEE(32); Might 1068 draws nothing.
    assert_eq!(
        sd.get(92, 1).expect("Shield Stun").abnormal_visuals,
        vec![7]
    );
    assert_eq!(sd.get(96, 1).expect("Bleed").abnormal_visuals, vec![1]);
    assert_eq!(sd.get(65, 1).expect("Horror").abnormal_visuals, vec![32]);
    assert!(sd.get(1068, 1).expect("Might").abnormal_visuals.is_empty());
    // An unknown enum name resolves to nothing rather than panicking.
    assert_eq!(
        crate::model::skill::abnormal_visual_client_id("NOT_A_REAL_AVE"),
        None
    );
    assert_eq!(
        crate::model::skill::abnormal_visual_client_id("STUN"),
        Some(7)
    );

    // The rest of the CC family, against the real Interlude skills.
    // Seal of Silence 1246 silences (magic only); Shield Slam 353 is the
    // physical twin; Mystic Immunity 1411 blocks incoming debuffs; Horror
    // 65 blocks control; Trick 11 cancels the target.
    use crate::model::skill::effect_flag;
    assert_eq!(
        sd.get(1246, 1).expect("Seal of Silence").effect_flags(),
        effect_flag::MUTED
    );
    assert_eq!(
        sd.get(353, 1).expect("Shield Slam").effect_flags() & effect_flag::PHYSICAL_MUTED,
        effect_flag::PHYSICAL_MUTED
    );
    assert_eq!(
        sd.get(1411, 1).expect("Mystic Immunity").effect_flags() & effect_flag::DEBUFF_BLOCK,
        effect_flag::DEBUFF_BLOCK
    );
    assert_eq!(
        sd.get(65, 1).expect("Horror").effect_flags() & effect_flag::BLOCK_CONTROL,
        effect_flag::BLOCK_CONTROL
    );
    assert!(
        matches!(
            sd.get(11, 1)
                .expect("Trick")
                .effects
                .iter()
                .find(|e| matches!(e, SkillEffect::TargetCancel { .. })),
            Some(SkillEffect::TargetCancel { .. })
        ),
        "Trick cancels its target"
    );
    // A silence must not also block physical skills, and vice versa.
    assert_eq!(
        sd.get(1246, 1).unwrap().effect_flags() & effect_flag::PHYSICAL_MUTED,
        0
    );

    // Noblesse Blessing 1323 — its only effect is the flag the death path
    // reads; without the parse arm the buff would be dropped whole.
    let bless = sd.get(1323, 1).expect("Noblesse Blessing");
    assert!(matches!(
        bless.effects.as_slice(),
        [SkillEffect::NoblesseBless]
    ));
    assert_eq!(bless.effect_flags(), effect_flag::NOBLESS_BLESSING);
    assert!(
        !bless.stay_after_death,
        "the blessing itself is what death consumes"
    );
    // `<stayAfterDeath>` is parsed case-insensitively — the dist writes both
    // spellings: Final Flying Form 840 `true`, Report Status 6038 `True`.
    // Might 1068 is untagged.
    assert!(sd.get(840, 1).expect("Final Flying Form").stay_after_death);
    assert!(
        sd.get(6038, 1).expect("Report Status").stay_after_death,
        "`True` parses too"
    );
    assert!(!sd.get(1068, 1).expect("Might").stay_after_death);

    // Fury Fists 222 — an upkeep toggle: `HealOverTime` with a *negative*
    // power, i.e. an HP cost per tick, not a heal. Silent Move 221 is the
    // MP-cost twin. Both are toggles, so their upkeep also drives the
    // toggle-off-on-exhaustion path.
    let fury_fists = sd.get(222, 1).expect("Fury Fists lvl 1");
    assert_eq!(fury_fists.operate_type, OperateType::Toggle);
    assert!(
        matches!(
            fury_fists.effects.iter().find(|e| matches!(e, SkillEffect::HealOverTime { .. })),
            Some(SkillEffect::HealOverTime { power, ticks }) if *power == -12.0 && *ticks == 2
        ),
        "got {:?}",
        fury_fists.effects
    );
    let silent_move = sd.get(221, 1).expect("Silent Move lvl 1");
    assert!(
        matches!(
            silent_move.effects.iter().find(|e| matches!(e, SkillEffect::ManaDamOverTime { .. })),
            Some(SkillEffect::ManaDamOverTime { power, ticks }) if *power == 9.0 && *ticks == 5
        ),
        "got {:?}",
        silent_move.effects
    );

    // Braveheart 440 grants a flat +1000 CP; Touch of Death 342 takes CP as
    // a percentage.
    let braveheart = sd.get(440, 1).expect("Braveheart lvl 1");
    assert!(
        matches!(
            braveheart.effects.iter().find(|e| matches!(e, SkillEffect::Cp { .. })),
            Some(SkillEffect::Cp { amount, percent: false }) if *amount == 1000.0
        ),
        "got {:?}",
        braveheart.effects
    );
    assert!(matches!(
        sd.get(342, 1).expect("Touch of Death").effects.iter().find(|e| matches!(e, SkillEffect::Cp { .. })),
        Some(SkillEffect::Cp { amount, percent: true }) if *amount == -90.0
    ));
    // Touch of Life 341 raises the healing its target receives (PER → the
    // multiplicative stat); Touch of Death 342 lowers it.
    assert!(
        sd.get(341, 1)
            .expect("Touch of Life")
            .stat_modifier_effects()
            .iter()
            .any(|m| m.stat == Stat::HealEffect && m.amount == 30.0)
    );

    // Guts 139 — the debuff-resistance buff: a negative `amount` on
    // `ResistAbnormalByCategory` means *more* resistant, and it must parse
    // as a PER modifier (the XML carries no <mode>, so a naive read would
    // make it DIFF and mean something entirely different).
    let guts = sd.get(139, 1).expect("Guts lvl 1");
    let resist = guts
        .stat_modifier_effects()
        .into_iter()
        .find(|m| m.stat == Stat::ResistAbnormalDebuff)
        .expect("Guts pumps ResistAbnormalDebuff");
    assert_eq!(resist.mode, StatModifierType::Per);
    assert_eq!(
        resist.amount, -50.0,
        "Guts lvl 1 is -50 → x0.5 debuff chance"
    );
    // Touch of Death 342 is the same effect with the sign flipped.
    let touch_of_death = sd.get(342, 1).expect("Touch of Death lvl 1");
    assert_eq!(
        touch_of_death
            .stat_modifier_effects()
            .into_iter()
            .find(|m| m.stat == Stat::ResistAbnormalDebuff)
            .map(|m| m.amount),
        Some(30.0)
    );
    // Ultimate Defense 110 resists *dispel* rather than debuffs.
    let ultimate_defense = sd.get(110, 1).expect("Ultimate Defense lvl 1");
    assert!(
        ultimate_defense
            .stat_modifier_effects()
            .iter()
            .any(|m| m.stat == Stat::ResistDispelBuff && m.amount == -80.0)
    );

    // Prophecy of Water 1355 blocks the BUFF_SPECIAL_* slots, which is how
    // the Prophecies stay mutually exclusive.
    let prophecy = sd.get(1355, 1).expect("Prophecy of Water lvl 1");
    let blocked = prophecy.blocked_abnormals();
    assert!(
        blocked.contains(&"BUFF_SPECIAL_ATTACK".to_string()),
        "got {blocked:?}"
    );
    assert_eq!(blocked.len(), 5, "all five BUFF_SPECIAL slots: {blocked:?}");
    // An ordinary buff blocks nothing.
    assert!(
        sd.get(1068, 1)
            .expect("Might")
            .blocked_abnormals()
            .is_empty()
    );

    // Warrior Bane 1350 / Mass Warrior Bane 1344 — probabilistic dispel.
    let bane = sd.get(1350, 1).expect("Warrior Bane lvl 1");
    match bane
        .effects
        .iter()
        .find(|e| matches!(e, SkillEffect::DispelBySlotProbability { .. }))
    {
        Some(SkillEffect::DispelBySlotProbability { dispel, rate }) => {
            assert_eq!(*rate, 80, "single-target Bane is 80%");
            assert!(dispel.contains(&"SPEED_UP".to_string()), "got {dispel:?}");
        }
        other => panic!("expected DispelBySlotProbability, got {other:?}"),
    }
    let mass_bane = sd.get(1344, 1).expect("Mass Warrior Bane lvl 1");
    assert!(
        mass_bane.effects.iter().any(|e| matches!(
            e,
            SkillEffect::DispelBySlotProbability { rate, .. } if *rate == 40
        )),
        "the mass version trades rate for reach"
    );

    // Shield Stun 92 / Arrest 402 — the crowd-control pair. Neither carries
    // a stat modifier: the whole mechanic is the abnormal-state flag.
    let shield_stun = sd.get(92, 1).expect("Shield Stun lvl 1");
    assert_eq!(
        shield_stun.effect_flags(),
        crate::model::skill::effect_flag::BLOCK_ACTIONS
    );
    assert_eq!(shield_stun.abnormal_type, "STUN");
    assert!(shield_stun.stat_modifier_effects().is_empty());
    let arrest = sd.get(402, 1).expect("Arrest lvl 1");
    assert_eq!(
        arrest.effect_flags(),
        crate::model::skill::effect_flag::ROOTED
    );
    assert_eq!(arrest.abnormal_type, "ROOT_PHYSICALLY");
    // A root does NOT block actions — only movement.
    assert_eq!(
        arrest.effect_flags() & crate::model::skill::effect_flag::BLOCK_ACTIONS,
        0
    );
    // An ordinary buff contributes no state flags at all.
    assert_eq!(sd.get(1068, 1).expect("Might").effect_flags(), 0);

    // Thunder Storm 48 casts from SELF with a POINT_BLANK sweep — the
    // scope that centres on the *caster* rather than the target, which is
    // why its targetType is SELF even though it is an offensive skill.
    let thunder_storm = sd.get(48, 1).expect("Thunder Storm lvl 1");
    assert_eq!(thunder_storm.affect_scope, AffectScope::PointBlank);
    assert_eq!(thunder_storm.target_type, TargetType::Self_);
    assert_eq!(thunder_storm.affect_object, AffectObject::NotFriend);
    assert_eq!(thunder_storm.affect_range, 150);
    // ...and it is *also* a stun, so it exercises both G19 slices at once:
    // a caster-centred sweep that block-actions everything it catches.
    assert_eq!(
        thunder_storm.effect_flags(),
        crate::model::skill::effect_flag::BLOCK_ACTIONS
    );
    // A skill with no `<activateRate>` defaults to -1 (always lands): the
    // buff Might 1068.
    let might = sd.get(1068, 1).expect("Might lvl 1");
    assert_eq!(might.activate_rate, -1);
    // ...and, carrying no <affectLimit>, is uncapped.
    assert_eq!(might.affect_limit(|_| 0), 0);

    // Skill 1011 "Heal": the reference datapack's effect body is
    // `<item>power</item>`, which parses to the param key `item` — so the
    // `power` param is absent. Java still creates the Heal effect with
    // `getDouble("power", 0)` = 0 (healing via the mAtk term); the effect
    // must NOT be dropped. Guard that the effect exists with power 0.
    let heal = sd.get(1011, 3).expect("Heal lvl 3");
    assert!(matches!(heal.effects.as_slice(), [SkillEffect::Heal { power }] if *power == 0.0));

    // "Knight - Individual" shares reuse group 10008 with its siblings.
    let ki = sd.get(10248, 1).expect("Knight - Individual lvl 1");
    assert_eq!(ki.reuse_delay_group, 10008);
    assert_eq!(ki.reuse_key(), 10008);

    // The `/unstuck` escape skills (G15.5): static 5-minute (2099) and
    // GM 1-second (2100) casts whose `Escape TOWN` effect must parse to
    // `EscapeToTown` — an empty effect list would cast and go nowhere.
    let escape = sd.get(2099, 1).expect("Escape (5-minute) lvl 1");
    assert_eq!(escape.magic_type, 2, "static skill");
    assert_eq!(escape.hit_time, 300_000);
    assert_eq!(escape.target_type, TargetType::Self_);
    assert!(matches!(
        escape.effects.as_slice(),
        [SkillEffect::Escape {
            dest: EscapeDest::Town
        }]
    ));
    let gm_escape = sd.get(2100, 1).expect("Escape: 1 Second lvl 1");
    assert!(matches!(
        gm_escape.effects.as_slice(),
        [SkillEffect::Escape {
            dest: EscapeDest::Town
        }]
    ));

    // G15 item-cast slice: `ItemSkillsTemplate` picks the instant vs cast
    // branch from `withoutAction` + the item's `immediate_effect`, and
    // `checkConsume` reads the skill's `itemConsumeId`. Scroll of Escape
    // (2013) declares neither `withoutAction` nor a short hit time, so it
    // must cast for its full 20 s and name its reagent.
    let soe = sd.get(2013, 1).expect("Scroll of Escape lvl 1");
    assert_eq!(soe.hit_time, 20_000);
    assert!(!soe.without_action, "no <withoutAction> -> cast branch");
    assert_eq!(soe.item_consume_id, 736, "the scroll itself");
    assert_eq!(soe.item_consume_count, 1);
    // Scroll: Might (2057) is the 4 s buff-scroll shape.
    let might = sd.get(2057, 1).expect("Scroll: Might lvl 1");
    assert_eq!(might.hit_time, 4000);
    assert!(!might.without_action);
    assert_eq!(might.item_consume_id, 3933);
    // A potion skill carries no reagent — the item handler consumes it via
    // the item's own `immediate_effect`.
    assert_eq!(
        sd.get(2031, 1)
            .expect("Healing Potion lvl 1")
            .item_consume_id,
        0
    );

    // Blessing of Protection 5182 (Newbie Helper): its `ProtectionBlessing`
    // effect carries no stat modifier — before this arm it fell through to
    // an empty effect list and never landed as a buff. It must parse to the
    // marker so `apply_skill_effects` still creates the icon-only PK_PROTECT
    // buff (7200 s).
    let blessing = sd.get(5182, 1).expect("Blessing of Protection lvl 1");
    assert!(matches!(
        blessing.effects.as_slice(),
        [SkillEffect::ProtectionBlessing]
    ));
    assert_eq!(blessing.abnormal_time, 7200);

    // The Newbie Helper support buffs must all load with their stat effects
    // (empty-effect skills would silently drop and show no icon): Wind Walk
    // 4322 pumps all four move speeds; Shield 4323 is PhysicalDefence;
    // Empower 4331 is MAtk.
    let wind_walk = sd.get(4322, 1).expect("Adventurer's Wind Walk lvl 1");
    assert_eq!(
        wind_walk.stat_modifier_effects().len(),
        4,
        "Speed pumps 4 move stats"
    );
    assert!(
        !sd.get(4323, 1)
            .expect("Shield")
            .stat_modifier_effects()
            .is_empty()
    );
    assert!(
        !sd.get(4331, 1)
            .expect("Empower")
            .stat_modifier_effects()
            .is_empty()
    );

    // Skill 22490 "Mysterious Spiritshot d 5000" — the `Restoration`
    // effect backing the "Mysterious Blessed Spiritshot Pack (5000)
    // (D-grade)" item (22599). Previously parsed with an empty effect
    // list, so using the pack consumed it and granted nothing.
    let spiritshot_pack = sd
        .get(22490, 5)
        .expect("Mysterious Spiritshot d 5000 lvl 5");
    assert!(matches!(
        spiritshot_pack.effects.as_slice(),
        [SkillEffect::GiveItem {
            item_id: 21852,
            item_count: 5000,
            item_enchant_level: 0
        }]
    ));

    // Skill 323 "Quiver of Arrow" — a real `RestorationRandom` skill
    // (three weighted groups of Mithril Arrow).
    let quiver = sd.get(323, 1).expect("Quiver of Arrow lvl 1");
    match quiver.effects.as_slice() {
        [SkillEffect::GiveItemRandom { groups }] => {
            assert_eq!(groups.len(), 3);
            assert_eq!(groups[0].chance, 30.0);
            assert_eq!(
                groups[0].items,
                vec![RestorationItem {
                    item_id: 1344,
                    count: 700,
                    min_enchant: 0,
                    max_enchant: 0
                }]
            );
            assert_eq!(groups[1].chance, 50.0);
            assert_eq!(groups[1].items[0].count, 1400);
            assert_eq!(groups[2].chance, 20.0);
            assert_eq!(groups[2].items[0].count, 2800);
        }
        other => panic!("expected one GiveItemRandom effect, got {other:?}"),
    }

    // Grade-penalty skills (6209 weapon / 6213 armor) back the expertise
    // penalty — each level must carry the registry-known stat maluses so
    // `refresh_expertise_penalty` actually debuffs the over-grade wearer.
    let weapon_pen = sd.get(6209, 1).expect("Weapon Grade Penalty lvl 1");
    assert!(
        !weapon_pen.stat_modifier_effects().is_empty(),
        "6209 must have stat effects"
    );
    assert!(
        weapon_pen
            .stat_modifier_effects()
            .iter()
            .any(|e| e.stat == Stat::PhysicalAttack)
    );
    let armor_pen = sd.get(6213, 4).expect("Armor Grade Penalty lvl 4");
    assert!(
        !armor_pen.stat_modifier_effects().is_empty(),
        "6213 must have stat effects"
    );

    // Clan Advent (19009) — the clan-leader-online aura applied via the clan
    // login/logout hooks. Permanent (`abnormalTime=-1`) with all six stat
    // effects: PAtk/PDef/MDef/MAtk percent buffs + flat HP/MP regen.
    let advent = sd.get(19009, 1).expect("Clan Advent lvl 1");
    assert_eq!(advent.abnormal_time, -1, "Clan Advent is permanent");
    let stats: Vec<Stat> = advent
        .stat_modifier_effects()
        .iter()
        .map(|e| e.stat)
        .collect();
    for want in [
        Stat::PhysicalAttack,
        Stat::PhysicalDefence,
        Stat::MagicalDefence,
        Stat::MagicalAttack,
        Stat::RegenerateHpRate,
        Stat::RegenerateMpRate,
    ] {
        assert!(
            stats.contains(&want),
            "Clan Advent must modify {want:?}, got {stats:?}"
        );
    }

    // Curse Poison 1168: a `DamOverTime` debuff (power 11, ticks 5, no
    // `canKill`) at lvl 1. Before the handler existed the effect fell
    // through `EFFECT_REGISTRY` and was dropped, so the poison landed as a
    // buff icon but never dealt damage.
    let curse_poison = sd.get(1168, 1).expect("Curse Poison lvl 1");
    assert!(matches!(
        curse_poison.effects.as_slice(),
        [SkillEffect::DamOverTime { power, ticks, can_kill: false }] if *power == 11.0 && *ticks == 5
    ));
    assert_eq!(curse_poison.abnormal_time, 30, "poison lasts 30s");

    // Cure Poison 1012: a `DispelBySlot` cleanse whose per-level `<dispel>`
    // string parses to `(POISON, level)` pairs (3/7/9 across levels 1-3).
    // Before the handler existed the effect fell through `EFFECT_REGISTRY`
    // and was dropped, so the cure cast but removed nothing.
    for (lvl, want) in [(1, 3), (2, 7), (3, 9)] {
        let cure = sd
            .get(1012, lvl)
            .unwrap_or_else(|| panic!("Cure Poison lvl {lvl}"));
        assert!(
            matches!(cure.effects.as_slice(), [SkillEffect::DispelBySlot { dispel }] if dispel.as_slice() == [("POISON".to_string(), want)]),
            "Cure Poison lvl {lvl} dispels POISON,{want}, got {:?}",
            cure.effects,
        );
    }

    // Spoil 254: an `ENEMY_ONLY` debuff carrying the `Spoil` effect and a
    // per-level `magicLevel` (10 at lvl 1) the `calcMagicSuccess` roll reads.
    let spoil = sd.get(254, 1).expect("Spoil lvl 1");
    assert_eq!(spoil.target_type, TargetType::EnemyOnly);
    assert_eq!(spoil.magic_level, 10);
    assert!(spoil.is_bad(), "Spoil has negative effectPoint");
    assert!(matches!(spoil.effects.as_slice(), [SkillEffect::Spoil]));

    // Sweeper 42: an `NPC_BODY` (corpse) skill whose effects are
    // `Sweeper` then `ConsumeBody` (order matters — claim loot, then decay).
    let sweeper = sd.get(42, 1).expect("Sweeper lvl 1");
    assert_eq!(sweeper.target_type, TargetType::NpcBody);
    assert!(matches!(
        sweeper.effects.as_slice(),
        [SkillEffect::Sweeper, SkillEffect::ConsumeBody]
    ));

    // Common Craft 1322 / Dwarven Craft 1321: self-target ability skills
    // whose only effect opens the matching recipe window. Both parsed to an
    // empty effect list before `OpenCommonRecipeBook`/`OpenDwarfRecipeBook`
    // were registered, so casting them did nothing at all.
    let common_craft = sd.get(1322, 1).expect("Common Craft lvl 1");
    assert_eq!(common_craft.target_type, TargetType::Self_);
    assert!(matches!(
        common_craft.effects.as_slice(),
        [SkillEffect::OpenRecipeBook { dwarven: false }]
    ));
    let dwarven_craft = sd.get(1321, 1).expect("Dwarven Craft lvl 1");
    assert!(matches!(
        dwarven_craft.effects.as_slice(),
        [SkillEffect::OpenRecipeBook { dwarven: true }]
    ));

    // Community-board buffer skills that previously loaded with an empty
    // effect list (every effect unregistered → dropped whole at the
    // empty-`buff_effects` bail) and so never landed / showed no icon.
    //
    // Blessed Shield 1243 (`ShieldDefenceRate`, PER +5% at lvl 1) and Death
    // Whisper 1242 (`CriticalDamage`, PER +25% at lvl 1) carry real stat
    // modifiers now. Death Whisper's PER mode must pick `CRITICAL_DAMAGE`
    // (not the `CRITICAL_DAMAGE_ADD` diff-mode sibling).
    let blessed_shield = sd.get(1243, 1).expect("Blessed Shield lvl 1");
    assert!(matches!(
        blessed_shield.effects.as_slice(),
        [SkillEffect::StatModifier(m)]
            if m.stat == Stat::ShieldDefenceRate && m.mode == StatModifierType::Per && m.amount == 5.0
    ));
    let death_whisper = sd.get(1242, 1).expect("Death Whisper lvl 1");
    assert!(matches!(
        death_whisper.effects.as_slice(),
        [SkillEffect::StatModifier(m)]
            if m.stat == Stat::CriticalDamage && m.mode == StatModifierType::Per && m.amount == 25.0
    ));

    // Mental Shield 1035 and Stun Resistance ("Resist Shock") 1259 carry a
    // `DefenceTrait` marker; Vampiric Rage 1268 carries a `VampiricAttack`
    // marker. No stat modifier, but the marker keeps the buff off the
    // empty-effects bail so it lands icon-only for its 1200 s.
    let mental_shield = sd.get(1035, 1).expect("Mental Shield lvl 1");
    assert!(matches!(
        mental_shield.effects.as_slice(),
        [SkillEffect::DefenceTrait { .. }]
    ));
    assert_eq!(mental_shield.abnormal_time, 1200);
    let resist_shock = sd.get(1259, 1).expect("Stun Resistance lvl 1");
    assert!(matches!(
        resist_shock.effects.as_slice(),
        [SkillEffect::DefenceTrait { .. }]
    ));
    let vampiric_rage = sd.get(1268, 1).expect("Vampiric Rage lvl 1");
    assert!(matches!(
        vampiric_rage.effects.as_slice(),
        [SkillEffect::VampiricAttack { amount, chance }] if *amount == 6.0 && *chance == 80.0
    ));
}

/// A trimmed Wind Strike (1177): per-level `targetType` and
/// `MagicalAttack` power, scalar `isMagic`/`castRange`, per-level
/// `effectPoint` — the exact shapes in `01100-01199.xml`.
#[test]
fn parses_wind_strike_shaped_skill() {
    let xml = r#"
    <list>
        <skill id="1177" toLevel="2" name="Wind Strike">
            <castRange>600</castRange>
            <effectPoint>
                <value level="1">-92</value>
                <value level="2">-106</value>
            </effectPoint>
            <effectRange>1100</effectRange>
            <hitTime>4000</hitTime>
            <isMagic>1</isMagic>
            <mpConsume>
                <value level="1">7</value>
                <value level="2">7</value>
            </mpConsume>
            <mpInitialConsume>
                <value level="1">2</value>
                <value level="2">2</value>
            </mpInitialConsume>
            <operateType>A1</operateType>
            <reuseDelay>1200</reuseDelay>
            <targetType>
                <value level="1">ENEMY_ONLY</value>
                <value level="2">ENEMY</value>
            </targetType>
            <effects>
                <effect name="MagicalAttack">
                    <power>
                        <value level="1">12</value>
                        <value level="2">13</value>
                    </power>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let l1 = out.skills.get(&(1177, 1)).expect("level 1 parsed");
    assert_eq!(l1.target_type, TargetType::EnemyOnly);
    assert_eq!(l1.magic_type, 1);
    assert_eq!(l1.effect_point, -92);
    assert!(l1.is_bad());
    assert_eq!(l1.cast_range, 600);
    assert_eq!(l1.effect_range, 1100);
    assert_eq!(l1.hit_time, 4000);
    assert_eq!(l1.reuse_delay, 1200);
    assert_eq!(l1.mp_consume, 7);
    assert_eq!(l1.mp_initial_consume, 2);
    assert!(
        matches!(l1.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 12.0)
    );

    let l2 = out.skills.get(&(1177, 2)).expect("level 2 parsed");
    assert_eq!(l2.target_type, TargetType::Enemy);
    assert!(
        matches!(l2.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 13.0)
    );
}

/// A Heal-shaped effect parses to `SkillEffect::Heal`; a stat-modifier
/// effect still lands in `StatModifier` with `<amount>`; an unregistered
/// effect name is dropped without dropping the skill.
#[test]
fn parses_heal_stat_and_unknown_effects() {
    let xml = r#"
    <list>
        <skill id="1015" toLevel="1" name="Battle Heal">
            <operateType>A1</operateType>
            <targetType>TARGET</targetType>
            <effects>
                <effect name="Heal">
                    <power>83</power>
                </effect>
                <effect name="PAtk">
                    <amount>10</amount>
                    <mode>PER</mode>
                </effect>
                <effect name="SomeUnportedEffect">
                    <power>5</power>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let s = out.skills.get(&(1015, 1)).expect("skill parsed");
    assert_eq!(s.target_type, TargetType::Target);
    assert_eq!(s.effects.len(), 2, "unknown effect dropped");
    assert!(matches!(s.effects[0], SkillEffect::Heal { power } if power == 83.0));
    assert!(matches!(
        s.effects[1],
        SkillEffect::StatModifier(StatModifierEffect { stat: Stat::PhysicalAttack, mode: StatModifierType::Per, amount, .. }) if amount == 10.0
    ));
}

/// Concentration-shaped skill: a lone `ReduceCancel` effect must parse to a
/// `StatModifier(ATTACK_CANCEL)`. Before it was registered, the effect fell
/// through, the effect list was empty, and `apply_skill_effects` dropped the
/// whole buff — so Concentration never landed from the community board.
#[test]
fn reduce_cancel_parses_to_attack_cancel_stat() {
    let xml = r#"
    <list>
        <skill id="1078" toLevel="1" name="Concentration">
            <operateType>A2</operateType>
            <abnormalType>CANCEL_PROB_DOWN</abnormalType>
            <abnormalTime>1200</abnormalTime>
            <targetType>TARGET</targetType>
            <effects>
                <effect name="ReduceCancel">
                    <amount>-18</amount>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let s = out.skills.get(&(1078, 1)).expect("skill parsed");
    assert_eq!(s.effects.len(), 1, "the ReduceCancel effect is not dropped");
    assert!(matches!(
        s.effects[0],
        SkillEffect::StatModifier(StatModifierEffect { stat: Stat::AttackCancel, mode: StatModifierType::Diff, amount, .. }) if amount == -18.0
    ));
}

/// Community-board dance/song buffs whose only effects are `AttackAttribute`
/// (Dance of Light), `MagicMpCost`/`Reuse` (Song of Champion/Renewal),
/// `Reuse` (Gift of Seraphim) or `DamageShield` (Song of Vengeance) must parse
/// to their icon-only marker rather than being dropped. Before these arms
/// existed, every effect fell through `EFFECT_REGISTRY`, the effect list was
/// empty, and `apply_skill_effects` dropped the whole buff — so none of these
/// landed from the community board.
#[test]
fn dance_song_buffs_parse_to_iconless_markers() {
    let xml = r#"
    <list>
        <skill id="277" toLevel="1" name="Dance of Light">
            <operateType>A2</operateType>
            <abnormalTime>120</abnormalTime>
            <targetType>SELF</targetType>
            <effects>
                <effect name="AttackAttribute">
                    <amount>20</amount>
                    <attribute>HOLY</attribute>
                </effect>
            </effects>
        </skill>
        <skill id="8547" toLevel="1" name="Song of Champion">
            <operateType>A2</operateType>
            <abnormalTime>120</abnormalTime>
            <targetType>SELF</targetType>
            <effects>
                <effect name="MagicMpCost">
                    <amount>-20</amount>
                    <mode>PER</mode>
                    <magicType>0</magicType>
                </effect>
                <effect name="Reuse">
                    <amount>-10</amount>
                    <mode>PER</mode>
                    <magicType>0</magicType>
                </effect>
            </effects>
        </skill>
        <skill id="305" toLevel="1" name="Song of Vengeance">
            <operateType>A2</operateType>
            <abnormalTime>120</abnormalTime>
            <targetType>SELF</targetType>
            <effects>
                <effect name="DamageShield">
                    <amount>20</amount>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let dol = out.skills.get(&(277, 1)).expect("Dance of Light parsed");
    // `AttackAttribute` graduated from icon-only marker to a real element
    // POWER modifier in the G19 attributes slice.
    assert!(
        matches!(
            dol.effects.as_slice(),
            [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::HolyPower, amount, .. })] if *amount == 20.0
        ),
        "Dance of Light grants HolyPower +20: {:?}",
        dol.effects
    );
    let soc = out.skills.get(&(8547, 1)).expect("Song of Champion parsed");
    // Both carry their bucket and percentage; `<mode>PER</mode>` is
    // decorative (Java's handlers read only `magicType` and `amount`).
    assert!(
        matches!(
            soc.effects.as_slice(),
            [
                SkillEffect::MagicMpCost {
                    magic_type: 0,
                    amount: a
                },
                SkillEffect::Reuse {
                    magic_type: 0,
                    amount: b
                }
            ] if *a == -20.0 && *b == -10.0
        ),
        "MagicMpCost/Reuse carry their magicType and amount: {:?}",
        soc.effects
    );
    let sov = out.skills.get(&(305, 1)).expect("Song of Vengeance parsed");
    assert!(
        matches!(sov.effects.as_slice(), [SkillEffect::DamageShield { amount }] if *amount == 20.0),
        "DamageShield carries its reflect percentage"
    );
}

/// G19 `EnlargeSlot`: the craftsman-guild storage passives (real dist
/// shapes — Expand Inventory has no `<type>`, defaulting to
/// `INVENTORY_NORMAL`; Expand Dwarven Craft picks `RECIPE_DWARVEN`; Expand
/// Trade carries two effect blocks, one `TRADE_BUY` one `TRADE_SELL`).
/// Before this arm the effect fell through to `EFFECT_REGISTRY` (a
/// 1-name-1-stat table that can't express the type-selected stat) and
/// these skills did nothing.
#[test]
fn enlarge_slot_picks_stat_by_type_param() {
    let xml = r#"
    <list>
        <skill id="1372" toLevel="1" name="Expand Inventory">
            <operateType>P</operateType>
            <targetType>SELF</targetType>
            <effects>
                <effect name="EnlargeSlot">
                    <amount>6</amount>
                    <mode>DIFF</mode>
                </effect>
            </effects>
        </skill>
        <skill id="1368" toLevel="1" name="Expand Dwarven Craft">
            <operateType>P</operateType>
            <targetType>SELF</targetType>
            <effects>
                <effect name="EnlargeSlot">
                    <amount>6</amount>
                    <mode>DIFF</mode>
                    <type>RECIPE_DWARVEN</type>
                </effect>
            </effects>
        </skill>
        <skill id="1370" toLevel="1" name="Expand Trade">
            <operateType>P</operateType>
            <targetType>SELF</targetType>
            <effects>
                <effect name="EnlargeSlot">
                    <amount>1</amount>
                    <mode>DIFF</mode>
                    <type>TRADE_BUY</type>
                </effect>
                <effect name="EnlargeSlot">
                    <amount>1</amount>
                    <mode>DIFF</mode>
                    <type>TRADE_SELL</type>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let inv = out.skills.get(&(1372, 1)).expect("Expand Inventory parsed");
    assert!(
        matches!(inv.effects.as_slice(), [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::InventoryNormal, amount, .. })] if *amount == 6.0),
        "no <type> defaults to INVENTORY_NORMAL: {:?}",
        inv.effects
    );
    let dwc = out
        .skills
        .get(&(1368, 1))
        .expect("Expand Dwarven Craft parsed");
    assert!(
        matches!(dwc.effects.as_slice(), [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::RecipeDwarven, amount, .. })] if *amount == 6.0),
        "type=RECIPE_DWARVEN picked: {:?}",
        dwc.effects
    );
    let trade = out.skills.get(&(1370, 1)).expect("Expand Trade parsed");
    assert!(
        matches!(
            trade.effects.as_slice(),
            [
                SkillEffect::StatModifier(StatModifierEffect {
                    stat: Stat::TradeBuy,
                    ..
                }),
                SkillEffect::StatModifier(StatModifierEffect {
                    stat: Stat::TradeSell,
                    ..
                }),
            ]
        ),
        "both TRADE_BUY and TRADE_SELL land: {:?}",
        trade.effects
    );
}

/// G19 hate-manipulation effects — real dist shapes: `GetAgro` is a
/// self-closing no-param tag (Aggression 28, paired with `TargetMe`,
/// which stays unported — no locked-target UI concept on this port);
/// `AddHate` reads `power`; `DeleteHate`/`DeleteHateOfMe` read `chance`.
/// Before this arm all four fell through to `EFFECT_REGISTRY`, weren't
/// found, and were silently dropped.
#[test]
fn hate_effects_parse_getagro_addhate_deletehate() {
    let xml = r#"
    <list>
        <skill id="28" toLevel="1" name="Aggression">
            <operateType>A1</operateType>
            <targetType>ENEMY_ONLY</targetType>
            <effects>
                <effect name="TargetMe" />
                <effect name="GetAgro" />
            </effects>
        </skill>
        <skill id="15" toLevel="1" name="Charm">
            <operateType>A1</operateType>
            <targetType>ENEMY_ONLY</targetType>
            <effects>
                <effect name="AddHate">
                    <power>500</power>
                </effect>
            </effects>
        </skill>
        <skill id="1273" toLevel="1" name="Eva's Serenade">
            <operateType>A2</operateType>
            <targetType>SELF</targetType>
            <effects>
                <effect name="DeleteHate">
                    <chance>80</chance>
                </effect>
            </effects>
        </skill>
        <skill id="1156" toLevel="1" name="Forget">
            <operateType>A2</operateType>
            <targetType>ENEMY_ONLY</targetType>
            <effects>
                <effect name="DeleteHateOfMe">
                    <chance>80</chance>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let aggression = out.skills.get(&(28, 1)).expect("Aggression parsed");
    // G34 S4: `TargetMe` is ported now, so Aggression keeps **both** its
    // effects — the playable-side target lock and the monster-side hate
    // grab. They are not alternatives: Java runs `TargetMe` only for
    // playables and `GetAgro` only for attackables, so one skill needs the
    // pair to taunt both kinds of target.
    assert!(
        matches!(
            aggression.effects.as_slice(),
            [SkillEffect::TargetMe, SkillEffect::GetAgro]
        ),
        "TargetMe + GetAgro, in datapack order: {:?}",
        aggression.effects
    );
    let charm = out.skills.get(&(15, 1)).expect("Charm parsed");
    assert!(
        matches!(charm.effects.as_slice(), [SkillEffect::AddHate { power }] if *power == 500.0),
        "AddHate power=500: {:?}",
        charm.effects
    );
    let eva = out.skills.get(&(1273, 1)).expect("Eva's Serenade parsed");
    assert!(
        matches!(
            eva.effects.as_slice(),
            [SkillEffect::DeleteHate { chance: 80 }]
        ),
        "DeleteHate chance=80: {:?}",
        eva.effects
    );
    let forget = out.skills.get(&(1156, 1)).expect("Forget parsed");
    assert!(
        matches!(
            forget.effects.as_slice(),
            [SkillEffect::DeleteHateOfMe { chance: 80 }]
        ),
        "DeleteHateOfMe chance=80: {:?}",
        forget.effects
    );
}

/// G19 `DispelByCategory` — the "Cancel" family, real dist shapes:
/// Cancellation (`BUFF`/25/5) and Cleanse (`DEBUFF`/100/10, no `<slot>`
/// exercised here since Cancellation already covers the explicit-BUFF
/// path — Cleanse's own tag is DEBUFF). Before this arm the effect fell
/// through to `EFFECT_REGISTRY`, wasn't found, and these skills stripped
/// nothing.
#[test]
fn dispel_by_category_parses_slot_rate_max() {
    let xml = r#"
    <list>
        <skill id="1056" toLevel="1" name="Cancellation">
            <operateType>A1</operateType>
            <targetType>TARGET</targetType>
            <effects>
                <effect name="DispelByCategory">
                    <slot>BUFF</slot>
                    <rate>25</rate>
                    <max>5</max>
                </effect>
            </effects>
        </skill>
        <skill id="1409" toLevel="1" name="Cleanse">
            <operateType>A1</operateType>
            <targetType>SELF</targetType>
            <effects>
                <effect name="DispelByCategory">
                    <slot>DEBUFF</slot>
                    <rate>100</rate>
                    <max>10</max>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let cancellation = out.skills.get(&(1056, 1)).expect("Cancellation parsed");
    assert!(
        matches!(
            cancellation.effects.as_slice(),
            [SkillEffect::DispelByCategory {
                slot: DispelSlot::Buff,
                rate: 25,
                max: 5
            }]
        ),
        "BUFF/25/5: {:?}",
        cancellation.effects
    );
    let cleanse = out.skills.get(&(1409, 1)).expect("Cleanse parsed");
    assert!(
        matches!(
            cleanse.effects.as_slice(),
            [SkillEffect::DispelByCategory {
                slot: DispelSlot::Debuff,
                rate: 100,
                max: 10
            }]
        ),
        "DEBUFF/100/10: {:?}",
        cleanse.effects
    );
}

/// G19 `PhysicalAttackRange`: real dist shapes — Archery (431, `DIFF
/// +50`) and Rapid Fire (413, `PER -50`, a stance trading range for
/// reload speed), both `<weaponType>BOW</weaponType>`-conditioned. Before
/// this it was unregistered in `EFFECT_REGISTRY` and fell through.
#[test]
fn physical_attack_range_parses_diff_and_per_bow_conditioned() {
    let xml = r#"
    <list>
        <skill id="431" toLevel="1" name="Archery">
            <operateType>P</operateType>
            <targetType>SELF</targetType>
            <effects>
                <effect name="PhysicalAttackRange">
                    <amount>50</amount>
                    <mode>DIFF</mode>
                    <weaponType><item>BOW</item></weaponType>
                </effect>
            </effects>
        </skill>
        <skill id="413" toLevel="1" name="Rapid Fire">
            <operateType>T</operateType>
            <targetType>SELF</targetType>
            <effects>
                <effect name="PhysicalAttackRange">
                    <amount>-50</amount>
                    <mode>PER</mode>
                    <weaponType><item>BOW</item></weaponType>
                </effect>
            </effects>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);

    let archery = out.skills.get(&(431, 1)).expect("Archery parsed");
    assert!(
        matches!(
            archery.effects.as_slice(),
            [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::PhysicalAttackRange, mode: StatModifierType::Diff, amount, weapon_condition, .. })]
                if *amount == 50.0 && *weapon_condition != 0
        ),
        "DIFF +50, bow-conditioned: {:?}",
        archery.effects
    );
    let rapid_fire = out.skills.get(&(413, 1)).expect("Rapid Fire parsed");
    assert!(
        matches!(
            rapid_fire.effects.as_slice(),
            [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::PhysicalAttackRange, mode: StatModifierType::Per, amount, weapon_condition, .. })]
                if *amount == -50.0 && *weapon_condition != 0
        ),
        "PER -50, bow-conditioned: {:?}",
        rapid_fire.effects
    );
}

/// `EnableModifySkillDuration`/`SkillDurationList`: an ordinary-level buff in
/// the list has its `abnormalTime` replaced, a toggle in the list is exempt,
/// and a skill absent from the list is untouched (Java `Skill` constructor).
#[test]
fn skill_duration_list_overrides_abnormal_time() {
    let xml = r#"
    <list>
        <skill id="1078" toLevel="1" name="Concentration">
            <operateType>A2</operateType>
            <abnormalTime>1200</abnormalTime>
            <targetType>TARGET</targetType>
        </skill>
        <skill id="9999" toLevel="1" name="A Toggle">
            <operateType>T</operateType>
            <abnormalTime>1200</abnormalTime>
            <targetType>SELF</targetType>
        </skill>
        <skill id="5555" toLevel="1" name="Not Listed">
            <operateType>A2</operateType>
            <abnormalTime>1200</abnormalTime>
            <targetType>TARGET</targetType>
        </skill>
    </list>"#;
    let mut out = ParsedSkills::default();
    parse_str(xml, &mut out);
    let mut sd = SkillData {
        skills: out.skills,
        enchanted: out.enchanted,
        routes: out.routes,
        names: out.names,
        gaps: out.gaps.into_inner(),
    };

    let list = HashMap::from([(1078, 7200), (9999, 7200)]);
    sd.apply_skill_duration_list(&list);

    assert_eq!(
        sd.get(1078, 1).unwrap().abnormal_time,
        7200,
        "active buff time replaced"
    );
    assert_eq!(
        sd.get(9999, 1).unwrap().abnormal_time,
        1200,
        "toggle is exempt"
    );
    assert_eq!(
        sd.get(5555, 1).unwrap().abnormal_time,
        1200,
        "skill not in list is untouched"
    );

    // Enchanted levels (100..=140) add rather than replace.
    let enchanted = Skill {
        self_continuous: false,
        level: 101,
        ..sd.get(1078, 1).unwrap().clone()
    };
    sd.insert_for_test(enchanted);
    sd.apply_skill_duration_list(&HashMap::from([(1078, 100)]));
    assert_eq!(
        sd.get(1078, 101).unwrap().abnormal_time,
        7300,
        "enchanted level adds to base"
    );
}

/// **Every `abnormalVisualEffect` name the dist's own skills use resolves.**
/// The name→client-id table used to stop at id 38 while the datapack
/// references 133 distinct names, so 102 of them (`AURA_BUFF`,
/// `ABSORB_SHIELD`, the grade-change glows…) parsed into nothing and the
/// visual never reached the client. The table is now generated from Java's
/// enum in full, so every name the dist uses resolves.
#[test]
fn every_datapack_abnormal_visual_name_resolves() {
    use crate::model::skill::abnormal_visual_client_id;
    const ROOT: &str = crate::data::DIST_GAME;
    let dir = format!("{ROOT}data/stats/skills");
    let mut names = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("skills dir").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let xml = std::fs::read_to_string(&path).unwrap_or_default();
        for chunk in xml.split("abnormalVisualEffect>").skip(1) {
            let Some(value) = chunk.split('<').next() else {
                continue;
            };
            for n in value.split(';').map(str::trim).filter(|n| !n.is_empty()) {
                names.insert(n.to_string());
            }
        }
    }
    assert!(names.len() > 100, "the dist really does use many of them");
    let unresolved: Vec<&String> = names
        .iter()
        .filter(|n| abnormal_visual_client_id(n).is_none())
        .collect();
    assert!(
        unresolved.is_empty(),
        "every name the dist skills use must resolve, missing: {unresolved:?}"
    );
    // Spot-check a few that the truncated table used to drop.
    assert_eq!(abnormal_visual_client_id("AURA_BUFF"), Some(57));
    assert_eq!(abnormal_visual_client_id("CHANGE_HAIR_B"), Some(39));
    assert_eq!(abnormal_visual_client_id("ABSORB_SHIELD"), Some(152));
}
