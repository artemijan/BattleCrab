//! `ReflectSkill` + `BlockMove` — two defensive-stance effects (G19).
//!
//! Physical Mirror 350 and Magical Mirror 351 carry **only** `ReflectSkill`, so
//! both were dropped whole. `BlockMove` is the `_isImmobilized` source that
//! `game_loop::abnormal`'s own module docs listed as missing.

use super::*;

use crate::model::components::{Buffs, StatModifiers};
use crate::model::skill::{SkillEffect, effect_flag};
use crate::model::stats::Stat;

const PLAYER: i32 = 9001;
const CID: u32 = 1;
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn dist_skills() -> crate::data::skill_data::SkillData {
    crate::data::skill_data::SkillData::load_from(DIST)
}

/// Stamp a buff carrying `flags` onto the player.
fn give_flag_buff(world: &mut World, oid: i32, skill_id: i32, flags: u32) {
    world
        .objects
        .get_component_mut::<Buffs>(&oid)
        .unwrap()
        .0
        .push(crate::model::skill::ActiveBuff {
            displayed: true,
            skill_id,
            skill_level: 1,
            abnormal_type_client_id: 0,
            abnormal_type: "NONE".to_string(),
            abnormal_level: 0,
            slot: crate::model::skill::BuffSlot::Buff,
            expires_at_tick: u64::MAX,
            passive: false,
            effect_flags: flags,
            blocked_abnormals: Vec::new(),
            abnormal_visuals: Vec::new(),
            effects: Vec::new(),
        });
}

// ---------------------------------------------------------------------------
// BlockMove
// ---------------------------------------------------------------------------

/// `setImmobilized(true)` — the creature can no longer move. This is the
/// `_isImmobilized` term Java's `isMovementDisabled()` ORs and the port had no
/// source for.
#[test]
fn block_move_disables_movement() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    assert!(
        !crate::game_loop::abnormal::is_movement_disabled(&world, PLAYER),
        "mobile to begin with"
    );

    give_flag_buff(&mut world, PLAYER, 110, effect_flag::IMMOBILIZED);
    assert!(
        crate::game_loop::abnormal::is_movement_disabled(&world, PLAYER),
        "BlockMove pins them in place"
    );
}

/// Unlike a stun, an immobilised creature can still **act** — that is the whole
/// point of Ultimate Defense and Snipe. Only the movement gate closes.
#[test]
fn an_immobilised_creature_can_still_act() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    give_flag_buff(&mut world, PLAYER, 110, effect_flag::IMMOBILIZED);

    assert!(crate::game_loop::abnormal::is_movement_disabled(
        &world, PLAYER
    ));
    assert!(
        !crate::game_loop::abnormal::is_blocked_from_actions(&world, PLAYER),
        "immobilise is not a stun — attacking and casting stay available"
    );
}

/// The carriers whose `BlockMove` sits in the ordinary `<effects>` block parse
/// it, and keep the effects they already had.
///
/// **Vengeance 368 is deliberately absent.** Its `BlockMove` lives in
/// `<selfEffects>` and so lands on the caster rather than the target — see
/// `vengeance_block_move_loads_from_its_self_effect_scope` below.
#[test]
fn real_dist_block_move_skills_parse() {
    let skills = dist_skills();
    // Ultimate Defense 110, Snipe 313.
    for id in [110, 313] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::BlockMove)),
            "skill {id} carries BlockMove: {:?}",
            skill.effects
        );
        assert_ne!(
            skill.effect_flags() & effect_flag::IMMOBILIZED,
            0,
            "skill {id} contributes the flag"
        );
        assert!(
            skill.effects.len() > 1,
            "skill {id} keeps its other effects"
        );
    }
}

/// **Gap closed.** The `BlockMove` slice found that Vengeance 368's immobilise
/// sat in `<selfEffects>`, a scope the parser did not read; the effect-scopes
/// slice ported it. This test flipped from "still unread" to asserting the
/// effect loads — into `self_effects`, not the general list, because it buffs
/// the caster rather than the target.
#[test]
fn vengeance_block_move_loads_from_its_self_effect_scope() {
    let skills = dist_skills();
    let vengeance = skills.get(368, 1).expect("Vengeance loads");
    assert!(
        vengeance
            .self_effects
            .iter()
            .any(|e| matches!(e, SkillEffect::BlockMove)),
        "now read from <selfEffects>: {:?}",
        vengeance.self_effects
    );
    assert!(
        !vengeance
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::BlockMove)),
        "and not merged into the general list, which would apply it to the target"
    );
    assert!(
        vengeance.effects.len() >= 3,
        "its ordinary <effects> still load"
    );
}

// ---------------------------------------------------------------------------
// ReflectSkill
// ---------------------------------------------------------------------------

/// The stat is additive and lands on whoever holds the buff — `pump` →
/// `mergeAdd`. Expressed through the ordinary `StatModifier` pipeline.
#[test]
fn reflect_skill_folds_into_an_additive_stat() {
    let skills = dist_skills();
    // Physical Mirror 350 → the physical stat; Magical Mirror 351 → the magic one.
    let mods_of = |id: i32| {
        skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"))
            .stat_modifier_effects()
            .iter()
            .map(|m| (m.stat, m.amount))
            .collect::<Vec<_>>()
    };
    let physical = mods_of(350);
    assert!(
        physical
            .iter()
            .any(|(s, a)| *s == Stat::ReflectSkillPhysic && *a > 0.0),
        "Physical Mirror pumps the physical reflect chance: {physical:?}"
    );
    let magical = mods_of(351);
    assert!(
        magical
            .iter()
            .any(|(s, a)| *s == Stat::ReflectSkillMagic && *a > 0.0),
        "Magical Mirror pumps the magic one: {magical:?}"
    );
    // Both carry both stats — the `type` value is `MAGIC`, not `MAGICAL`;
    // guessing the latter would route every magic reflect into the physical
    // stat and this assertion would fail.
    assert!(
        physical.iter().any(|(s, _)| *s == Stat::ReflectSkillMagic),
        "Physical Mirror also has a magic share"
    );
}

/// Both Mirrors carry *nothing but* `ReflectSkill` — the reason both were
/// dropped whole. Each carries **two** of them: a physical and a magic chance,
/// weighted 30/10 one way and 10/30 the other, so the two Mirrors differ by
/// emphasis rather than by kind.
#[test]
fn the_mirrors_carry_only_reflect_effects() {
    let skills = dist_skills();
    for (id, physical, magic) in [(350, 30.0, 10.0), (351, 10.0, 30.0)] {
        let skill = skills.get(id, 1).unwrap();
        assert!(
            skill
                .effects
                .iter()
                .all(|e| matches!(e, SkillEffect::ReflectSkill { .. })),
            "skill {id} carries only ReflectSkill: {:?}",
            skill.effects
        );
        let got: Vec<(bool, f64)> = skill
            .effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::ReflectSkill { magic, amount } => Some((*magic, *amount)),
                _ => None,
            })
            .collect();
        assert!(
            got.contains(&(false, physical)),
            "skill {id} physical {physical}: {got:?}"
        );
        assert!(
            got.contains(&(true, magic)),
            "skill {id} magic {magic}: {got:?}"
        );
    }
}

/// Riposte Stance 340 pairs it with several already-ported effects, all of
/// which must survive.
#[test]
fn riposte_stance_keeps_its_other_effects() {
    let skills = dist_skills();
    let riposte = skills.get(340, 1).expect("Riposte Stance loads");
    assert!(
        riposte
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::ReflectSkill { magic: false, .. }))
    );
    assert!(
        riposte
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::DamageShield { .. })),
        "its DamageShield grant survives"
    );
    assert!(
        riposte
            .stat_modifier_effects()
            .iter()
            .any(|m| m.stat == Stat::AccuracyCombat),
        "and its Accuracy bonus"
    );
}

/// The reflect stat is read off the **defender**, but which of the two applies
/// is decided by the *incoming skill's* magic flag — a defender holding only
/// the physical chance is not protected from a magic debuff.
#[test]
fn the_incoming_skill_picks_which_reflect_stat_is_read() {
    let (mut world, _db, _l) = cast_test_world();
    let _c = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    world
        .objects
        .get_component_mut::<StatModifiers>(&PLAYER)
        .unwrap()
        .add
        .insert(Stat::ReflectSkillPhysic, 100.0);

    let mods = world
        .objects
        .get_component::<StatModifiers>(&PLAYER)
        .unwrap();
    assert_eq!(
        mods.add.get(&Stat::ReflectSkillPhysic).copied(),
        Some(100.0)
    );
    assert_eq!(
        mods.add.get(&Stat::ReflectSkillMagic).copied(),
        None,
        "a physical-only defender has no magic reflect chance at all"
    );
}
