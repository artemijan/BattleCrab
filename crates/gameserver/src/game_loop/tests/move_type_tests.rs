//! `StatByMoveType` + the player regen stat pipeline (G19).
//!
//! Two joined gaps: `StatByMoveType` was unparsed (Vital Force 148 and Clear
//! Mind 1297 carry *only* that effect, so both were passives that did nothing),
//! and `regen_player` never read `StatModifiers` at all — so every
//! `HpRegen`/`MpRegen`/`CpRegen` effect in the datapack was pumped and read by
//! nobody. Porting the first without the second would have been pointless: the
//! move-type term lands in the same finalizer the regen stats do.

use super::*;

use crate::model::components::space::Movement;
use crate::model::components::stats::{PlayerVitals, Speeds, StatModifiers};
use crate::model::movement::MoveData;
use crate::model::skill::effects::{SkillEffect, StatModifierEffect};
use crate::model::stats::{MoveType, Stat, StatModifierType};

use crate::game_loop::stats::regen::{move_type_of, movement_regen_multiplier, run_regen_tick};

const PLAYER: i32 = 4101;
const CID: u32 = 1;
const DIST: &str = crate::data::DIST_GAME;

/// `cast_test_world` with the **real** player templates and stat bonuses
/// loaded. Required for anything asserting on a regen *rate*: the synthetic
/// `GameData::for_test` templates have a zero `baseHpRegen`, so every rate
/// would be 0 and the multiplier assertions below would pass vacuously.
fn regen_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    world.data.player_templates = dist::player_templates_owned();
    world.data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    (world, db, l)
}

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}
fn mp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_mp
}

/// Wound the player so every regen branch has headroom to work in.
fn wound(world: &mut World, oid: i32) {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.cur_hp = 1.0;
    v.cur_mp = 1.0;
    world
        .objects
        .get_component_mut::<PlayerVitals>(&oid)
        .unwrap()
        .cur_cp = 0.0;
}

/// Put the player in a locomotion state: `Movement` present = moving, and
/// `Speeds.running` picks running vs walking (exactly what `move_type_of`
/// reads, mirroring Java's `getMoveType`).
fn set_moving(world: &mut World, oid: i32, moving: bool, running: bool) {
    if moving {
        world.objects.add_components(
            &oid,
            Movement(MoveData {
                start_x: 0,
                start_y: 0,
                start_z: 0,
                dest_x: 10_000,
                dest_y: 0,
                dest_z: 0,
                start_tick: world.tick,
                // Long enough that the mover never arrives mid-test and
                // silently drops back to Standing.
                total_ticks: 1_000_000,
                geo_path: None,
            }),
        );
    } else {
        world.objects.remove_component::<Movement>(&oid);
    }
    world
        .objects
        .get_component_mut::<Speeds>(&oid)
        .unwrap()
        .running = running;
}

/// One regen tick's HP gain, from a fixed wounded start.
fn hp_gain_per_tick(world: &mut World) -> f64 {
    wound(world, PLAYER);
    let before = hp(world, PLAYER);
    run_regen_tick(world);
    hp(world, PLAYER) - before
}

fn mp_gain_per_tick(world: &mut World) -> f64 {
    wound(world, PLAYER);
    let before = mp(world, PLAYER);
    run_regen_tick(world);
    mp(world, PLAYER) - before
}

// ---------------------------------------------------------------------------
// The movement multiplier
// ---------------------------------------------------------------------------

/// The "Calculate Movement bonus" block, identical in all three regen
/// finalizers. The load-bearing oddity: **walking falls through every branch**
/// and gets no multiplier, so walking regen (×1.0) is *worse* than standing
/// still (×1.1). That is Java as written.
#[test]
fn movement_regen_multipliers_match_java() {
    assert_eq!(movement_regen_multiplier(MoveType::Sitting), 1.5);
    assert_eq!(movement_regen_multiplier(MoveType::Standing), 1.1);
    assert_eq!(movement_regen_multiplier(MoveType::Running), 0.7);
    assert_eq!(
        movement_regen_multiplier(MoveType::Walking),
        1.0,
        "walking gets no multiplier at all"
    );
    assert!(
        movement_regen_multiplier(MoveType::Walking)
            < movement_regen_multiplier(MoveType::Standing),
        "walking regenerates slower than standing still — Java's fall-through, not a bug"
    );
}

/// `Creature.getMoveType`: moving+running → Running, moving alone → Walking,
/// otherwise Standing. (Sitting has no source on this port.)
#[test]
fn move_type_follows_movement_and_run_flag() {
    let (mut world, _db, _l) = cast_test_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    set_moving(&mut world, PLAYER, false, true);
    assert_eq!(
        move_type_of(&world, PLAYER),
        MoveType::Standing,
        "not moving → Standing, run flag irrelevant"
    );

    set_moving(&mut world, PLAYER, true, true);
    assert_eq!(move_type_of(&world, PLAYER), MoveType::Running);

    set_moving(&mut world, PLAYER, true, false);
    assert_eq!(move_type_of(&world, PLAYER), MoveType::Walking);
}

/// The regen rate actually tracks the move type. Before this slice the port
/// hard-coded the standing 1.1 for every state, so a running player
/// regenerated ~57% faster than they should have.
#[test]
fn regen_rate_tracks_the_move_type() {
    let (mut world, _db, _l) = regen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    set_moving(&mut world, PLAYER, false, false);
    let standing = hp_gain_per_tick(&mut world);
    set_moving(&mut world, PLAYER, true, false);
    let walking = hp_gain_per_tick(&mut world);
    set_moving(&mut world, PLAYER, true, true);
    let running = hp_gain_per_tick(&mut world);

    assert!(
        standing > walking,
        "standing ({standing}) beats walking ({walking})"
    );
    assert!(
        walking > running,
        "walking ({walking}) beats running ({running})"
    );
    // The ratios are the raw multipliers: 1.1 / 1.0 / 0.7.
    assert!(
        (standing / walking - 1.1).abs() < 1e-6,
        "standing/walking == 1.1, got {}",
        standing / walking
    );
    assert!(
        (running / walking - 0.7).abs() < 1e-6,
        "running/walking == 0.7, got {}",
        running / walking
    );
}

// ---------------------------------------------------------------------------
// The regen stat pipeline (`HpRegen`/`MpRegen`/`CpRegen`)
// ---------------------------------------------------------------------------

/// The gap this slice closes on the way past: `regen_player` never consulted
/// `StatModifiers`, so a flat `HpRegen` buff changed nothing. Java ends every
/// regen finalizer in `Stat.defaultValue` = `mul*base + add + moveTypeValue`.
#[test]
fn hp_regen_stat_modifiers_now_reach_the_regen_tick() {
    let (mut world, _db, _l) = regen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    set_moving(&mut world, PLAYER, false, false);

    let bare = hp_gain_per_tick(&mut world);

    // A flat +10 `HpRegen` (Java `Diff` mode), as Regeneration 1044 grants.
    world
        .objects
        .get_component_mut::<StatModifiers>(&PLAYER)
        .unwrap()
        .add
        .insert(Stat::RegenerateHpRate, 10.0);
    let buffed = hp_gain_per_tick(&mut world);
    assert!(
        (buffed - bare - 10.0).abs() < 1e-6,
        "a flat +10 adds exactly 10: {bare} -> {buffed}"
    );

    // And a `Per` modifier multiplies the base, not the already-added flat.
    world
        .objects
        .get_component_mut::<StatModifiers>(&PLAYER)
        .unwrap()
        .add
        .clear();
    world
        .objects
        .get_component_mut::<StatModifiers>(&PLAYER)
        .unwrap()
        .mul
        .insert(Stat::RegenerateHpRate, 2.0);
    let doubled = hp_gain_per_tick(&mut world);
    assert!(
        (doubled - bare * 2.0).abs() < 1e-6,
        "×2 doubles the base: {bare} -> {doubled}"
    );
}

/// The same for MP — `MpRegen` is the bigger of the two families (12 learnable
/// skills: Focus Mind 191, Mana Recovery 214, Armor Mastery 142, …).
#[test]
fn mp_regen_stat_modifiers_now_reach_the_regen_tick() {
    let (mut world, _db, _l) = regen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    set_moving(&mut world, PLAYER, false, false);

    let bare = mp_gain_per_tick(&mut world);
    world
        .objects
        .get_component_mut::<StatModifiers>(&PLAYER)
        .unwrap()
        .add
        .insert(Stat::RegenerateMpRate, 7.0);
    let buffed = mp_gain_per_tick(&mut world);
    assert!(
        (buffed - bare - 7.0).abs() < 1e-6,
        "a flat +7 MP regen lands: {bare} -> {buffed}"
    );
}

// ---------------------------------------------------------------------------
// `StatByMoveType` routing and effect
// ---------------------------------------------------------------------------

/// A move-type-qualified effect must land in `by_move_type`, **not** `add` —
/// folding it into `add` would apply it in every locomotion state instead of
/// the one it names, which is the whole point of the effect.
#[test]
fn move_type_effects_route_to_their_own_map() {
    let (mut world, _db, _l) = cast_test_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    let mut mods = StatModifiers::default();
    model::stat_finalize::apply_modifier(
        &mut mods,
        &StatModifierEffect {
            stat: Stat::RegenerateHpRate,
            mode: StatModifierType::Diff,
            amount: 1.9,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: Some(model::stats::StatQualifier::MoveType(MoveType::Running)),
            two_handed: false,
            hp_percent: 0,
        },
    );
    assert!(
        mods.add.is_empty(),
        "not folded into the unconditional add map"
    );
    assert_eq!(
        mods.move_type_value(Stat::RegenerateHpRate, MoveType::Running),
        1.9
    );
    assert_eq!(
        mods.move_type_value(Stat::RegenerateHpRate, MoveType::Standing),
        0.0,
        "and only in its own state"
    );
}

/// End to end: a `RUNNING`-qualified HP-regen bonus shows up while running and
/// vanishes when standing — the reverse of the movement multiplier's slope, so
/// this can't be satisfied by the multiplier alone.
#[test]
fn stat_by_move_type_applies_only_in_its_own_state() {
    let (mut world, _db, _l) = regen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    set_moving(&mut world, PLAYER, true, true);
    let running_bare = hp_gain_per_tick(&mut world);
    set_moving(&mut world, PLAYER, false, false);
    let standing_bare = hp_gain_per_tick(&mut world);

    world
        .objects
        .get_component_mut::<StatModifiers>(&PLAYER)
        .unwrap()
        .by_move_type
        .insert((Stat::RegenerateHpRate, MoveType::Running), 50.0);

    set_moving(&mut world, PLAYER, true, true);
    let running_buffed = hp_gain_per_tick(&mut world);
    set_moving(&mut world, PLAYER, false, false);
    let standing_buffed = hp_gain_per_tick(&mut world);

    assert!(
        (running_buffed - running_bare - 50.0).abs() < 1e-6,
        "the RUNNING bonus applies while running"
    );
    assert!(
        (standing_buffed - standing_bare).abs() < 1e-6,
        "and not at all while standing"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The four learnable `StatByMoveType` skills parse to move-type-qualified
/// modifiers with the real values. Vital Force and Clear Mind carry *only*
/// this effect, so before this slice they parsed to an empty effect list and
/// were dropped whole — passives that did precisely nothing.
#[test]
fn real_dist_stat_by_move_type_skills_parse() {
    let skills = dist::skills();

    let qualified = |id: i32, level: i32| -> Vec<(Stat, MoveType, f64)> {
        skills
            .get(id, level)
            .unwrap_or_else(|| panic!("skill {id} loads"))
            .effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) => match m.qualifier {
                    Some(model::stats::StatQualifier::MoveType(mt)) => Some((m.stat, mt, m.amount)),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    };

    // Vital Force 148 lvl 1: +1.9 HP *and* MP regen, while RUNNING.
    assert_eq!(
        qualified(148, 1),
        vec![
            (Stat::RegenerateHpRate, MoveType::Running, 1.9),
            (Stat::RegenerateMpRate, MoveType::Running, 1.9),
        ]
    );
    // Clear Mind 1297 lvl 1: MP regen, split across two *different* states.
    assert_eq!(
        qualified(1297, 1),
        vec![
            (Stat::RegenerateMpRate, MoveType::Walking, 3.2),
            (Stat::RegenerateMpRate, MoveType::Standing, 2.6),
        ]
    );
    // Acrobatic Move 225: the one non-regen use — evasion while RUNNING.
    assert_eq!(
        qualified(225, 1),
        vec![(Stat::EvasionRate, MoveType::Running, 4.0)]
    );
    // Esprit 171 — the same shape as Vital Force but its own values (2.5/1.8,
    // not 1.9/1.9), alongside a `DefenceTrait` that must survive the parse too.
    assert_eq!(
        qualified(171, 1),
        vec![
            (Stat::RegenerateHpRate, MoveType::Running, 2.5),
            (Stat::RegenerateMpRate, MoveType::Running, 1.8),
        ]
    );
    assert!(
        skills
            .get(171, 1)
            .unwrap()
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::DefenceTrait { .. })),
        "Esprit keeps its DefenceTrait"
    );
}

/// Learning Vital Force as a passive folds its move-type entries through the
/// ordinary `Player::from_char` passive path — no separate plumbing — and they
/// land in `by_move_type` rather than `add`.
#[test]
fn vital_force_passive_folds_into_by_move_type() {
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let bare = Player::from_char(&world.data, &dummy_char(4201, "Bare"));
    assert_eq!(
        bare.stat_modifiers
            .move_type_value(Stat::RegenerateHpRate, MoveType::Running),
        0.0
    );

    let mut chr = dummy_char(4202, "Vital");
    chr.skills = vec![(148, 1, 0)]; // Vital Force
    let bundle = Player::from_char(&world.data, &chr);
    assert_eq!(
        bundle
            .stat_modifiers
            .move_type_value(Stat::RegenerateHpRate, MoveType::Running),
        1.9,
        "the passive's RUNNING entry folded in"
    );
    assert!(
        !bundle
            .stat_modifiers
            .add
            .contains_key(&Stat::RegenerateHpRate),
        "and did not leak into the unconditional add map"
    );
}

/// `Config.HP_REGEN_MULTIPLIER` and its MP/CP siblings scale player regen.
///
/// Java applies these in `RegenHPFinalizer` at `baseValue *= isRaid ?
/// RAID_HP_REGEN_MULTIPLIER : HP_REGEN_MULTIPLIER` — **above** the
/// `isPlayer()` branch, so every creature gets them. This port applied them to
/// NPCs and pets and skipped players, and `CpRegenMultiplier` was never parsed
/// at all.
///
/// All three ship at 100 (×1.0), so the omission changed nothing on this dist
/// and nothing failed — which is precisely why it went unnoticed. The test
/// therefore retunes them, the way the only server that would ever notice does.
#[test]
fn the_config_regen_multipliers_scale_player_regen() {
    let (mut world, _db, _l) = regen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    set_moving(&mut world, PLAYER, false, false);

    let base_hp = hp_gain_per_tick(&mut world);
    let base_mp = mp_gain_per_tick(&mut world);
    assert!(base_hp > 0.0 && base_mp > 0.0, "sanity: regen happens");

    world.cfg.npc.hp_regen_multiplier = 3.0;
    world.cfg.npc.mp_regen_multiplier = 0.5;

    let scaled_hp = hp_gain_per_tick(&mut world);
    let scaled_mp = mp_gain_per_tick(&mut world);
    assert!(
        (scaled_hp - base_hp * 3.0).abs() < 1e-6,
        "HP regen scales by HpRegenMultiplier ({base_hp} → {scaled_hp})"
    );
    assert!(
        (scaled_mp - base_mp * 0.5).abs() < 1e-6,
        "MP regen scales by MpRegenMultiplier ({base_mp} → {scaled_mp})"
    );
}
