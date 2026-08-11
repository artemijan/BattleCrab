//! GROUND casts + channeling (G19, PLAN_G19_GROUND_CHANNELING.md): the
//! ex-0x41 world-position flow, the ground-centred POINT_BLANK sweep, the
//! `SkillChannelizer` tick (MP upkeep, re-sweep, CHANNELING effects), the
//! static channeling cast time, and the reagent gate/consume.

use super::*;

use crate::model::components::{Casting, GroundSkillTarget, Vitals};
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const CASTER: i32 = 2001;
const CID: u32 = 1;

fn dist_skills() -> crate::data::skill_data::SkillData {
    dist::skills_owned()
}

/// A Volcano in miniature: 100 ms to first tick, 200 ms per tick, a short
/// channel — real geometry, test-sized clock.
fn volcano_like(id: i32) -> Skill {
    Skill {
        self_continuous: false,
        id,
        name: format!("Test Volcano {id}"),
        operate_type: OperateType::Channeling,
        target_type: TargetType::Ground,
        affect_scope: AffectScope::PointBlank,
        affect_object: AffectObject::NotFriend,
        affect_range: 200,
        cast_range: 900,
        effect_range: 1000,
        effect_point: -676,
        magic_type: 1,
        hit_time: 1500,
        mp_per_channeling: 10,
        channeling_tick_ms: 200,
        channeling_start_ms: 100,
        // Tiny power: the default test template has almost no m.def, so a
        // realistic power one-shots even a 100k-HP fixture.
        channeling_effects: vec![SkillEffect::MagicalAttack { power: 1.0 }],
        ..Default::default()
    }
}

/// The ex-0x41 body: `x, y, z, skillId, ctrl(int), shift(byte)`.
fn ground_body(x: i32, y: i32, z: i32, skill_id: i32, shift: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(skill_id);
    w.write_i32(0);
    w.write_u8(u8::from(shift));
    w.into_bytes()
}

fn learn(world: &mut World, oid: i32, skill: &Skill) {
    world.data.skill_data.insert_for_test(skill.clone());
    world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&oid)
        .unwrap()
        .0
        .insert(skill.id, 1);
}

fn hp_of(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}

fn mp_of(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_mp
}

// ---------------------------------------------------------------------------
// Dist parse
// ---------------------------------------------------------------------------

/// Volcano 1419 as the dist writes it: a CA1 GROUND cast whose damage lives in
/// `<channelingEffects>` (2 s ticks from 1 s in, 80 MP each), consuming one
/// Magic Symbol 8876. This is also the census closure for the effect-scopes
/// slice's "channelingEffects are dropped" note.
#[test]
fn volcano_parses_as_a_channeling_ground_cast() {
    let sd = dist_skills();
    let volcano = sd.get(1419, 1).expect("Volcano lvl 1");
    assert_eq!(volcano.operate_type, OperateType::Channeling);
    assert_eq!(volcano.target_type, TargetType::Ground);
    assert_eq!(volcano.affect_scope, AffectScope::PointBlank);
    assert_eq!(volcano.channeling_tick_ms, 2000);
    assert_eq!(volcano.channeling_start_ms, 1000);
    assert_eq!(volcano.mp_per_channeling, 80);
    assert_eq!(volcano.item_consume_id, 8876);
    assert_eq!(volcano.item_consume_count, 1);
    assert!(
        volcano
            .channeling_effects
            .iter()
            .any(|e| matches!(e, SkillEffect::MagicalAttack { power } if *power == 500.0)),
        "the tick payload is MagicalAttack 500: {:?}",
        volcano.channeling_effects
    );
    assert!(volcano.effects.is_empty(), "nothing lands at cast finish");
    // The `mpPerChanneling` default is `mpConsume`, not 0 (Java
    // `set.getInt("mpPerChanneling", _mpConsume)`) — a skill without the tag
    // still drains. Sonic Buster declares neither field of the channeling
    // family, so its per-tick drain equals its (zero-defaulted) mpConsume.
    let sonic_buster = sd.get(9, 1).expect("Sonic Buster lvl 1");
    assert_eq!(sonic_buster.mp_per_channeling, sonic_buster.mp_consume);
}

// ---------------------------------------------------------------------------
// The happy path
// ---------------------------------------------------------------------------

/// The full flow: ex 0x41 → cast starts → ticks burn the mob standing at the
/// ground point, drain the caster's MP per tick, and never touch the caster
/// or a mob outside the circle.
#[test]
fn ground_channel_burns_the_point_not_the_caster() {
    let (mut world, _db, _l) = cast_test_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let skill = volcano_like(9200);
    learn(&mut world, CASTER, &skill);
    let (in_fire, outside) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, in_fire, 20001, "Monster", 5, 500, 0, 0);
    add_test_npc(&mut world, outside, 20001, "Monster", 5, 900, 0, 0);
    // The fixture caster has 50 MP; give the channel room to breathe. The mob
    // gets a deep HP pool so it survives (a dead mob despawns mid-test).
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = 10_000.0;
    {
        let v = world.objects.get_component_mut::<Vitals>(&in_fire).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }
    drain(&mut out);

    let mp_before = mp_of(&world, CASTER);
    let hp_before = hp_of(&world, in_fire);
    let outside_before = hp_of(&world, outside);
    let caster_hp_before = hp_of(&world, CASTER);
    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, 9200, false),
    );
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "the ground cast starts"
    );
    assert_eq!(
        world
            .objects
            .get_component::<GroundSkillTarget>(&CASTER)
            .map(|g| (g.x, g.y)),
        Some((500, 0)),
        "the world position is stored"
    );

    // 10 ticks = 1 s: first channel tick at 100 ms, then every 200 ms.
    advance_ticks(&mut world, 10);
    assert!(
        hp_of(&world, in_fire) < hp_before,
        "the mob at the ground point burns"
    );
    assert_eq!(
        hp_of(&world, outside),
        outside_before,
        "400 units outside the circle: untouched"
    );
    assert!(
        mp_of(&world, CASTER) < mp_before,
        "each tick drains mpPerChanneling"
    );
    // (No caster-HP assertion here: the burned mob legitimately fights back,
    // so "the volcano doesn't self-hit" is pinned at the sweep level below.)
    let _ = caster_hp_before;
}

/// The ground sweep never contains the caster — Java's world sweep skips its
/// origin object and nothing re-adds it — even with an `ALL` affect object,
/// where the friend/foe filter would not save them.
#[test]
fn the_ground_sweep_excludes_the_caster() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 500, 0);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 5, 520, 0, 0);
    // The caster stands INSIDE their own blast circle.
    world
        .objects
        .add_components(&CASTER, GroundSkillTarget { x: 500, y: 0, z: 0 });
    let mut skill = volcano_like(9210);
    skill.affect_object = AffectObject::All;

    let hit =
        crate::game_loop::skills::affect::targets_affected(&mut world, CASTER, CASTER, &skill);
    assert!(hit.contains(&mob), "the mob in the circle is swept");
    assert!(
        !hit.contains(&CASTER),
        "the caster never is, even under ALL"
    );
}

/// The ticks die with the cast: once the channel has run its course, further
/// world ticks deal no more damage.
#[test]
fn ticks_stop_when_the_cast_ends() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let skill = volcano_like(9201);
    learn(&mut world, CASTER, &skill);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 5, 500, 0, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = 10_000.0;
    {
        let v = world.objects.get_component_mut::<Vitals>(&mob).unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, 9201, false),
    );
    // Run far past the full cast (hit ≈ 1 s + cancel 2866 ms ≈ 39 ticks).
    advance_ticks(&mut world, 60);
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "the cast is long over"
    );
    let hp_after_cast = hp_of(&world, mob);
    assert!(hp_after_cast < 100_000.0, "sanity: the channel did damage");

    advance_ticks(&mut world, 20);
    assert_eq!(
        hp_of(&world, mob),
        hp_after_cast,
        "no tick survives the cast that owned it"
    );
}

/// A mob that walks into the fire mid-channel starts burning: the tick
/// re-sweeps the scope every time instead of freezing the target list.
#[test]
fn a_mob_walking_in_mid_channel_burns() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let skill = volcano_like(9202);
    learn(&mut world, CASTER, &skill);
    let latecomer = NPC_OID;
    // Starts outside the 200 circle around (500, 0)…
    add_test_npc(&mut world, latecomer, 20001, "Monster", 5, 850, 0, 0);
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&latecomer)
            .unwrap();
        v.max_hp = 100_000;
        v.cur_hp = 100_000.0;
    }
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = 10_000.0;

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, 9202, false),
    );
    advance_ticks(&mut world, 5);
    assert_eq!(
        hp_of(&world, latecomer),
        100_000.0,
        "outside the circle: safe so far"
    );

    // …then steps onto the point while the channel is still running.
    world
        .objects
        .get_component_mut::<Position>(&latecomer)
        .unwrap()
        .x = 500;
    advance_ticks(&mut world, 10);
    assert!(
        hp_of(&world, latecomer) < 100_000.0,
        "the re-sweep catches the latecomer"
    );
}

/// MP starvation mid-channel aborts the cast — SM 140's branch — instead of
/// ticking for free.
#[test]
fn mp_starvation_aborts_the_channel() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut skill = volcano_like(9203);
    skill.mp_per_channeling = 20;
    learn(&mut world, CASTER, &skill);
    let mob = NPC_OID;
    add_test_npc(&mut world, mob, 20001, "Monster", 5, 500, 0, 0);
    // Enough MP for two ticks, nowhere near the full channel.
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_mp = 45.0;

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, 9203, false),
    );
    assert!(world.objects.has_component::<Casting>(&CASTER));
    advance_ticks(&mut world, 15);
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "the third tick found the tank empty and aborted the cast"
    );
}

// ---------------------------------------------------------------------------
// Ground validation
// ---------------------------------------------------------------------------

/// A GROUND cast with no stored position (no ex 0x41 first) is refused
/// outright — Java's `useMagic` null check.
#[test]
fn ground_cast_without_a_position_is_refused() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let skill = volcano_like(9204);
    learn(&mut world, CASTER, &skill);

    crate::game_loop::skills::cast::use_magic(&mut world, CID, CASTER, 9204, false, false);
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "no stored world position → ActionFailed, no cast"
    );
}

/// Shift (Java `dontMove`) refuses a point beyond `castRange` instead of
/// walking; the same click without shift casts.
#[test]
fn shift_refuses_a_far_ground_point() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let skill = volcano_like(9205);
    learn(&mut world, CASTER, &skill);

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(1500, 0, 0, 9205, true),
    );
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "1500 > castRange 900 with dontMove → refused"
    );

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(1500, 0, 0, 9205, false),
    );
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "without shift the Ground handler applies no range gate (Java's client \
         constrains the cursor; the server casts as told)"
    );
}

// ---------------------------------------------------------------------------
// Static cast time
// ---------------------------------------------------------------------------

/// Channeling cast time ignores casting speed: doubling `mAtkSpd` halves an
/// Active cast's hit phase but leaves the CA1 channel at its full length —
/// Java skips the time factor entirely for channeling.
#[test]
fn channeling_cast_time_is_static() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let channel = volcano_like(9206);
    let active = Skill {
        self_continuous: false,
        operate_type: OperateType::Active,
        ..volcano_like(9207)
    };

    let hit_of = |world: &World, skill: &Skill| {
        let p = world.objects.get_component::<Player>(&CASTER).unwrap();
        let base = world
            .objects
            .get_component::<crate::model::components::BaseStats>(&CASTER)
            .unwrap();
        let mods = world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&CASTER)
            .unwrap();
        let combat = world
            .objects
            .get_component::<crate::model::components::CombatStats>(&CASTER)
            .unwrap();
        crate::model::formulas::calc_cast_times(p, base, mods, combat, &world.data, skill)
    };

    let (channel_slow, cancel, _) = hit_of(&world, &channel);
    let (active_slow, _, _) = hit_of(&world, &active);
    assert_eq!(
        cancel, 2866,
        "channeling pins the launch→finish phase at 2866 ms"
    );

    // Double the casting speed (the time factor reads
    // `StatModifiers.mul[MagicAttackSpeed]`).
    world
        .objects
        .get_component_mut::<crate::model::components::StatModifiers>(&CASTER)
        .unwrap()
        .mul
        .insert(crate::model::stats::Stat::MagicAttackSpeed, 2.0);
    let (channel_fast, _, _) = hit_of(&world, &channel);
    let (active_fast, _, _) = hit_of(&world, &active);

    assert_eq!(channel_fast, channel_slow, "the channel never shortens");
    assert!(
        active_fast < active_slow,
        "sanity: the same shape as Active does"
    );
}

// ---------------------------------------------------------------------------
// Reagent gate + consume
// ---------------------------------------------------------------------------

/// Volcano's Magic Symbol: no reagent → SM 2156 refusal; with one, the cast
/// starts and the symbol is consumed **at cast start** (Java pays reagents in
/// `startCasting`, not at finish).
#[test]
fn reagent_is_required_and_consumed_at_cast_start() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut skill = volcano_like(9208);
    skill.item_consume_id = 8876;
    skill.item_consume_count = 1;
    learn(&mut world, CASTER, &skill);

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, 9208, false),
    );
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "no Magic Symbol → the reagent gate refuses"
    );

    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<crate::model::inventory::Inventory>(&CASTER)
            .unwrap();
        inv.add_item(&data.item_data, 990_001, 8876, 2);
    }
    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, 9208, false),
    );
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "with a symbol it casts"
    );
    let left = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&CASTER)
        .unwrap()
        .count_of(8876);
    assert_eq!(left, 1, "one symbol paid at cast start");
}

// ---------------------------------------------------------------------------
// `channelingSkillId` — the stacking channeled buff (Battle/Spell Stance)
// ---------------------------------------------------------------------------

/// The channeled skill: three levels, a stat pump per level, so the *level*
/// that lands is observable.
const CHANNELED: i32 = 9310;
/// The stance that channels it — no `<channelingEffects>` at all, which is the
/// shape that used to bail out of the tick entirely.
const STANCE: i32 = 9311;
const ALLY: i32 = 2002;
const ALLY2: i32 = 2003;

fn channeled_skill(level: i32) -> Skill {
    Skill {
        self_continuous: false,
        id: CHANNELED,
        level,
        name: format!("Battle Force {level}"),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        is_continuous: true,
        abnormal_time: 15,
        abnormal_level: level,
        abnormal_type: "PHYSICAL_STANCE".into(),
        magic_type: 2,
        effects: vec![SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::PhysicalAttack,
                mode: crate::model::stats::StatModifierType::Diff,
                amount: (5 * level) as f64,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
            },
        )],
        ..Default::default()
    }
}

fn stance_skill() -> Skill {
    Skill {
        self_continuous: false,
        id: STANCE,
        level: 1,
        name: "Battle Stance".into(),
        operate_type: OperateType::Channeling,
        target_type: TargetType::Target,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        cast_range: 400,
        effect_range: 600,
        magic_type: 2,
        hit_time: 15000,
        mp_per_channeling: 1,
        channeling_tick_ms: 200,
        channeling_start_ms: 100,
        channeling_skill_id: CHANNELED,
        // Deliberately empty: a `channelingSkillId` skill applies a *named
        // skill*, never channeling effects.
        channeling_effects: Vec::new(),
        ..Default::default()
    }
}

fn stance_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cast_test_world();
    for lvl in 1..=3 {
        world.data.skill_data.insert_for_test(channeled_skill(lvl));
    }
    (world, db, l)
}

fn start_stance(world: &mut World, client: u32, caster: i32, target: i32) {
    world
        .objects
        .add_components(&caster, crate::model::components::TargetRef(Some(target)));
    world
        .objects
        .get_component_mut::<Vitals>(&caster)
        .unwrap()
        .cur_mp = 10_000.0;
    crate::game_loop::skills::cast::handle_request_magic_skill_use(
        world,
        client,
        &magic_skill_use_body(STANCE, false),
    );
}

/// One channeler lands the channeled skill at **level 1**.
///
/// The stance carries no `<channelingEffects>`, which is precisely the shape
/// that used to return early from the tick — so before this it channeled its MP
/// upkeep and applied nothing at all.
#[test]
fn a_single_channeler_applies_the_channeled_skill_at_level_one() {
    let (mut world, _db, _l) = stance_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _ally = ingame_caster(&mut world, 2, ALLY, 50, 0);
    let stance = stance_skill();
    learn(&mut world, CASTER, &stance);

    start_stance(&mut world, CID, CASTER, ALLY);
    advance_ticks(&mut world, 10);

    assert_eq!(
        buff_level(&world, ALLY, CHANNELED),
        Some(1),
        "one channeler → level 1"
    );
}

/// **Two channelers stack it to level 2.** The registry's size *is* the level
/// (`min(channelizers, maxLevel)`), which is the whole mechanic — a port that
/// applied a fixed level 1 would pass the test above and fail this one.
#[test]
fn two_channelers_stack_the_channeled_skill_to_level_two() {
    let (mut world, _db, _l) = stance_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _out2 = ingame_caster(&mut world, 3, ALLY2, 10, 0);
    let _ally = ingame_caster(&mut world, 2, ALLY, 50, 0);
    let stance = stance_skill();
    learn(&mut world, CASTER, &stance);
    learn(&mut world, ALLY2, &stance);

    start_stance(&mut world, CID, CASTER, ALLY);
    start_stance(&mut world, 3, ALLY2, ALLY);
    advance_ticks(&mut world, 10);

    assert_eq!(
        buff_level(&world, ALLY, CHANNELED),
        Some(2),
        "two channelers → level 2"
    );
}

/// The level is capped at the channeled skill's max level, so a fourth
/// channeler cannot push Battle Force past 3.
#[test]
fn the_channeled_level_is_capped_at_the_skills_max() {
    let (mut world, _db, _l) = stance_world();
    let _ally = ingame_caster(&mut world, 9, ALLY, 50, 0);
    // Four channelers on one ally.
    let casters = [(CID, CASTER), (3, ALLY2), (4, 2004), (5, 2005)];
    let stance = stance_skill();
    for (cid, oid) in casters {
        let _o = ingame_caster(&mut world, cid, oid, 10, 0);
        learn(&mut world, oid, &stance);
        start_stance(&mut world, cid, oid, ALLY);
    }
    advance_ticks(&mut world, 10);

    assert_eq!(
        buff_level(&world, ALLY, CHANNELED),
        Some(3),
        "four channelers, but the skill only has three levels"
    );
}

/// A channeler that stops is dropped from the registry, so the stack it was
/// contributing to shrinks. Pinned because the removal hangs off `stop_casting`
/// — a path that is easy to leave unhooked, and would then let a logged-off
/// channeler prop up a stack forever.
#[test]
fn stopping_a_channel_drops_that_channeler_from_the_stack() {
    let (mut world, _db, _l) = stance_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _out2 = ingame_caster(&mut world, 3, ALLY2, 10, 0);
    let _ally = ingame_caster(&mut world, 2, ALLY, 50, 0);
    let stance = stance_skill();
    learn(&mut world, CASTER, &stance);
    learn(&mut world, ALLY2, &stance);
    start_stance(&mut world, CID, CASTER, ALLY);
    start_stance(&mut world, 3, ALLY2, ALLY);
    advance_ticks(&mut world, 10);
    assert_eq!(buff_level(&world, ALLY, CHANNELED), Some(2), "baseline");

    crate::game_loop::skills::cast::stop_casting(&mut world, ALLY2);

    assert_eq!(
        world
            .channelized
            .get(&ALLY)
            .and_then(|m| m.get(&CHANNELED))
            .map_or(0, |s| s.len()),
        1,
        "only the still-channeling caster remains registered"
    );
}

/// The dist's real stances carry the ids this feature exists for. Pinned
/// because a comment in this port once claimed no reachable channeler used
/// `channelingSkillId` — 426 and 427 are learnable at 77.
#[test]
fn the_real_stances_parse_their_channeling_skill_id() {
    let skills = dist_skills();
    for (stance, channeled) in [(426, 5104), (427, 5105)] {
        let s = skills
            .get(stance, 1)
            .unwrap_or_else(|| panic!("skill {stance}"));
        assert_eq!(
            s.channeling_skill_id, channeled,
            "skill {stance} channels {channeled}"
        );
        assert!(
            s.channeling_effects.is_empty(),
            "and carries no channeling effects — it applies a named skill"
        );
        assert_eq!(
            skills.max_level(channeled),
            3,
            "the channeled skill has three levels to stack into"
        );
    }
}
