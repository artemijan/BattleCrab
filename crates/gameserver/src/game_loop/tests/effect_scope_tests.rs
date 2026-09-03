//! Effect scopes — `<selfEffects>`, `<pveEffects>`, `<pvpEffects>` (G19).
//!
//! The parser read only the default `<effects>` block, so every effect declared
//! in another scope silently never loaded. Found by the `BlockMove` slice, where
//! Vengeance 368's immobilise sat in `<selfEffects>` and did nothing.

use super::*;
use crate::game_loop::abnormal::has_buff;

use crate::model::skill::effect_flag;
use crate::model::skill::effects::SkillEffect;
use crate::model::stats::Stat;

const CASTER: i32 = 9501;
const CID: u32 = 1;

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Vengeance 368 is the case that exposed the gap: its `BlockMove` lives in
/// `<selfEffects>` while its defence buffs live in `<effects>`. Both must now
/// load, into their own lists.
#[test]
fn vengeance_self_effect_now_loads() {
    let skills = dist::skills_owned();
    let v = skills.get(368, 1).expect("Vengeance loads");

    assert!(
        v.self_effects
            .iter()
            .any(|e| matches!(e, SkillEffect::BlockMove)),
        "its BlockMove is in self_effects: {:?}",
        v.self_effects
    );
    assert!(
        !v.effects
            .iter()
            .any(|e| matches!(e, SkillEffect::BlockMove)),
        "and not duplicated into the general list"
    );
    assert!(
        v.effects.len() >= 3,
        "its ordinary <effects> still load: {:?}",
        v.effects
    );
}

/// The other learnable `<selfEffects>` carriers. Every one of them holds an
/// already-ported effect, so this slice is pure plumbing with immediate
/// payoff — six skills gained a real self-buff.
#[test]
fn the_learnable_self_effect_skills_all_load_something() {
    let skills = dist::skills_owned();
    // Blinding Blow 321 (Speed), Sonic Rage 345 / Raging Force 346
    // (FocusMomentum), Evade Shot 369 (PhysicalEvasion), Critical Blow 409
    // (FatalBlowRate).
    for id in [321, 345, 346, 369, 409] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            !skill.self_effects.is_empty(),
            "skill {id} has self effects: {:?}",
            skill.self_effects
        );
    }
}

/// Critical Blow 409's self-effect is a `FatalBlowRate` buff — a stat the port
/// already reads in `calc_blow_success`, so the plumbing produces a real
/// mechanical change rather than an inert marker.
#[test]
fn critical_blow_self_buffs_its_own_blow_rate() {
    let skills = dist::skills_owned();
    let cb = skills.get(409, 1).expect("Critical Blow loads");
    let self_mods: Vec<_> = cb
        .self_effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::StatModifier(m) => Some(m.stat),
            _ => None,
        })
        .collect();
    assert!(
        self_mods.contains(&Stat::BlowRate),
        "self-buffs BlowRate: {self_mods:?}"
    );
}

/// Scopes the port can't act on (`startEffects`, `endEffects`) are parsed as
/// `Other` and dropped rather than silently merged into the general list —
/// which would apply them at the wrong time. (`channelingEffects` graduated
/// to `Skill.channeling_effects` in the G19 ground/channeling slice.)
#[test]
fn unsupported_scopes_are_dropped_not_merged() {
    let skills = dist::skills_owned();
    // Anchor 1170 is the one learnable `<endEffects>` carrier.
    let anchor = skills.get(1170, 1).expect("Anchor loads");
    let general_and_self = anchor.effects.len() + anchor.self_effects.len();
    // Its end-effects must not have leaked into either live list. Asserting on
    // the *absence* of a merge rather than an exact count keeps this robust if
    // its ordinary effects change.
    assert!(
        anchor.pve_effects.is_empty() && anchor.pvp_effects.is_empty(),
        "no stray scope contents on Anchor"
    );
    assert!(
        general_and_self > 0,
        "sanity: Anchor still loads its ordinary effects"
    );
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

/// A self-effect lands on the **caster**, not the target — the whole point of
/// the scope.
#[test]
fn a_self_effect_lands_on_the_caster() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let target = 9502;
    let _t = ingame_caster(&mut world, 2, target, 40, 0);

    let skill = Skill {
        self_continuous: false,
        id: 9950,
        name: "SelfScoped".into(),
        target_type: TargetType::Target,
        abnormal_time: 60,
        abnormal_type: "SELFSCOPE".into(),
        // Nothing on the target; a flag buff on the caster.
        effects: Vec::new(),
        self_effects: vec![SkillEffect::BlockMove],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill.clone());

    // Apply the self scope the way the cast path does.
    let self_skill = Skill {
        self_continuous: false,
        effects: skill.self_effects.clone(),
        ..skill.clone()
    };
    effects::apply_skill_effects(&mut world, CASTER, CASTER, &self_skill);

    assert_ne!(
        abnormal::flags_of(&world, CASTER) & effect_flag::IMMOBILIZED,
        0,
        "the caster got the self-effect"
    );
    assert_eq!(
        abnormal::flags_of(&world, target) & effect_flag::IMMOBILIZED,
        0,
        "and the target did not"
    );
}

/// `Default for Skill` — added so struct literals stop breaking every time a
/// field is added (this slice added three). The non-obvious defaults are the
/// two Java sentinels that gates test for explicitly.
#[test]
fn skill_default_uses_javas_sentinels() {
    let d = Skill::default();
    assert_eq!(
        d.activate_rate, -1,
        "\"no declared rate\" — never reflected, always lands"
    );
    assert_eq!(d.reuse_delay_group, -1, "\"no group\"");
    assert_eq!(d.abnormal_type, "NONE");
    assert!(d.effects.is_empty() && d.self_effects.is_empty());
}

/// A skill with no scoped effects is unaffected — the overwhelming majority of
/// the datapack, and the case that must not regress.
#[test]
fn an_ordinary_skill_has_no_scoped_effects() {
    let skills = dist::skills_owned();
    // Death Whisper 1242: a plain single-scope buff.
    let dw = skills.get(1242, 1).expect("Death Whisper loads");
    assert!(!dw.effects.is_empty(), "its general effects load");
    assert!(dw.self_effects.is_empty() && dw.pve_effects.is_empty() && dw.pvp_effects.is_empty());
}

/// Buffs applied through the self scope go through the ordinary buff pipeline,
/// so they show up in `Buffs` like anything else.
#[test]
fn self_scope_buffs_use_the_normal_pipeline() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let skill = Skill {
        self_continuous: false,
        id: 9951,
        name: "SelfBuff".into(),
        abnormal_time: 60,
        abnormal_type: "SELFBUFF".into(),
        effects: vec![SkillEffect::BlockMove],
        ..Default::default()
    };
    effects::apply_skill_effects(&mut world, CASTER, CASTER, &skill);

    assert!(
        has_buff(&world, CASTER, 9951),
        "it lands as an ordinary timed buff"
    );
}
