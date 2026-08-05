//! Antharas — the escalating, capped minion waves.

use super::*;

use crate::game_loop::antharas::{ANTHARAS, AntharasMinions};

const ANTHARAS_OID: i32 = NPC_OID + 120;
const BEHEMOTH: i32 = 29069;
const TERASQUE: i32 = 29190;

fn antharas_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [
        (ANTHARAS, "GrandBoss"),
        (BEHEMOTH, "Monster"),
        (TERASQUE, "Monster"),
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 85;
        t.base_hp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db, l)
}

fn spawned(world: &mut World) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
        if x.npc_id == BEHEMOTH || x.npc_id == TERASQUE {
            n += 1;
        }
    });
    n
}

fn state(world: &World) -> AntharasMinions {
    *world
        .objects
        .get_component::<AntharasMinions>(&ANTHARAS_OID)
        .unwrap()
}

fn set_state(world: &mut World, count: i32, multiplier: i32) {
    world
        .objects
        .add_components(&ANTHARAS_OID, AntharasMinions { count, multiplier });
}

/// The first wave is a **single pair** — the multiplier starts at 1, so
/// Antharas opens gently and escalates.
#[test]
fn the_first_wave_is_one_pair() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    crate::game_loop::antharas::begin_waves(&mut world, ANTHARAS_OID);

    world.forced_rolls.push_back(50); // > 10: the multiplier grows
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 2, "one Behemoth and one Tarask");
    assert_eq!(
        state(&world).multiplier,
        2,
        "and the next wave will be bigger"
    );
}

/// **Waves grow to a cap of 4** (eight adds), not without bound.
#[test]
fn the_multiplier_stops_at_four() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 0, 4);

    world.forced_rolls.push_back(50); // would grow, if it could
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(state(&world).multiplier, 4, "capped");
    assert_eq!(spawned(&mut world), 8, "four pairs is the largest wave");
}

/// A low roll leaves the multiplier alone — growth is ~89% per wave, not
/// guaranteed.
#[test]
fn a_low_roll_does_not_grow_the_wave() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 0, 1);

    world.forced_rolls.push_back(5); // not > 10
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(state(&world).multiplier, 1, "still one pair next time");
}

/// **Near the cap, a full wave gives way to a single pair** — the ladder's
/// second step, which keeps the population from overshooting 100.
#[test]
fn a_full_wave_gives_way_to_a_pair_near_the_cap() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    // multiplier 4 would want 8, but 100 - 8 = 92 and we are past it.
    set_state(&mut world, 95, 4);

    world.forced_rolls.push_back(5);
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 2, "one pair, not eight");
    assert_eq!(state(&world).count, 97);
}

/// **At 98 the last slot is filled by a single, randomly chosen dragon** —
/// the ladder's third step. Collapsing the ladder to "a pair if there's room
/// for two" would stall the lair at 98 and lose this.
#[test]
fn the_last_slot_takes_one_random_dragon() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 98, 1);

    world.forced_rolls.push_back(0); // picks Behemoth
    world.forced_rolls.push_back(5);
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 1, "exactly one more");
    assert_eq!(state(&world).count, 99, "filling the lair to 99");
}

/// At the cap, a wave adds nothing — but still rearms, so the fight recovers
/// as adds are killed.
#[test]
fn a_full_lair_spawns_nothing_but_keeps_ticking() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 99, 1);

    let before = world.scheduler.len();
    world.forced_rolls.push_back(5);
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 0, "the lair is full");
    assert_eq!(
        world.scheduler.len(),
        before + 1,
        "and the next wave is still armed"
    );
}

// ---------------------------------------------------------------------------
// The entry cinematic (slice 17)
// ---------------------------------------------------------------------------

fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>, opcode: u8) -> usize {
    let mut n = 0;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&opcode) {
            n += 1;
        }
    }
    n
}

/// **Antharas chains, Valakas does not.** Each beat schedules exactly the next
/// one, so at any moment only a single cinematic timer is pending — unlike
/// Valakas, which arms all ten up front. Reusing the Valakas shape here would
/// have quietly changed the timing model, so the difference is pinned.
#[test]
fn the_cinematic_is_a_chain_not_a_batch() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);

    let before = world.scheduler.len();
    crate::game_loop::antharas::begin_cinematic(&mut world, ANTHARAS_OID);
    assert_eq!(
        world.scheduler.len() - before,
        1,
        "one beat armed, not five"
    );
}

/// Each beat sends its camera shot and arms the next.
#[test]
fn each_beat_sends_a_shot_and_arms_the_next() {
    let (mut world, _db, _l) = antharas_world();
    let mut rx = ingame_caster(&mut world, 1, 9960, 0, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    while rx.try_recv().is_ok() {}

    let before = world.scheduler.len();
    crate::game_loop::antharas::handle_cinematic_step(&mut world, ANTHARAS_OID, 0);
    assert_eq!(drain(&mut rx, 0xD6), 1, "one camera shot");
    assert_eq!(world.scheduler.len(), before + 1, "and the next beat armed");
}

/// **Beat 3 forks**: it roars *and* schedules a second social action 5.2 s
/// later, independent of the camera chain — the only beat that arms two
/// timers, which a uniform "each beat arms one" port would lose.
#[test]
fn the_third_beat_forks_a_second_social() {
    let (mut world, _db, _l) = antharas_world();
    let mut rx = ingame_caster(&mut world, 1, 9960, 0, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    while rx.try_recv().is_ok() {}

    let before = world.scheduler.len();
    crate::game_loop::antharas::handle_cinematic_step(&mut world, ANTHARAS_OID, 2);
    assert_eq!(
        world.scheduler.len(),
        before + 2,
        "the next beat *and* the forked social"
    );
    // `SocialAction` is 0x27 — the roar goes out with the shot, not only the
    // deferred one 5.2 s later.
    assert_eq!(
        drain(&mut rx, 0x27),
        1,
        "the roar accompanied the camera shot"
    );
}

/// The forked social fires on its own, after the chain has moved on.
#[test]
fn the_forked_social_fires_independently() {
    let (mut world, _db, _l) = antharas_world();
    let mut rx = ingame_caster(&mut world, 1, 9960, 0, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    while rx.try_recv().is_ok() {}

    crate::game_loop::antharas::handle_social(&mut world, ANTHARAS_OID);
    assert_eq!(
        drain(&mut rx, 0x27),
        1,
        "the second social went out by itself"
    );
}

/// The tail hands Antharas his AI back and **starts the minion waves** — so a
/// boss standing in its lair un-engaged is not already spawning adds.
#[test]
fn the_cinematic_tail_starts_the_waves() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);

    // One past the last camera beat is `START_MOVE`.
    crate::game_loop::antharas::handle_cinematic_step(&mut world, ANTHARAS_OID, 5);
    assert!(
        world
            .objects
            .get_component::<AntharasMinions>(&ANTHARAS_OID)
            .is_some(),
        "the wave state exists, so the waves are running"
    );
}

/// Spawning Antharas runs the cinematic rather than going straight to waves —
/// the ordering that keeps an un-engaged boss quiet.
#[test]
fn spawning_starts_the_cinematic_not_the_waves() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    crate::game_loop::antharas::begin_cinematic(&mut world, ANTHARAS_OID);

    assert!(
        world
            .objects
            .get_component::<AntharasMinions>(&ANTHARAS_OID)
            .is_none(),
        "no adds before the fight begins"
    );
}

// ---------------------------------------------------------------------------
// The entry gate (slice 18)
// ---------------------------------------------------------------------------

use crate::game_loop::antharas::{EntryVerdict, PORTAL_STONE};

const LEADER: i32 = 9940;
const MEMBER: i32 = 9941;

fn gate_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = antharas_world();
    world.grand_bosses.insert(
        ANTHARAS,
        crate::model::grand_boss::GrandBoss {
            boss_id: ANTHARAS,
            loc_x: 0,
            loc_y: 0,
            loc_z: 0,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: 0, // ALIVE — entry open
        },
    );
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = PORTAL_STONE;
    t.name = "Portal Stone".into();
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
    (world, db, l)
}

fn give_stone(world: &mut World, oid: i32) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&oid)
        .unwrap()
        .add_item(&data.item_data, 7_900_000 + oid, PORTAL_STONE, 1);
}

/// A solo player with a Portal Stone gets in.
#[test]
fn a_solo_player_with_a_stone_is_admitted() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    give_stone(&mut world, LEADER);

    assert_eq!(
        crate::game_loop::antharas::try_enter(&mut world, LEADER),
        EntryVerdict::Admitted(vec![LEADER])
    );
}

/// Without the stone, nobody enters.
#[test]
fn no_stone_no_entry() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);

    assert_eq!(
        crate::game_loop::antharas::try_enter(&mut world, LEADER),
        EntryVerdict::NoStone
    );
}

/// A dead or already-fighting Antharas refuses everyone, stone or not — and
/// **before** the stone is even checked, so the player is told the real reason.
#[test]
fn the_boss_state_is_checked_before_the_ticket() {
    for (status, expected) in [
        (3, EntryVerdict::BossDead),
        (2, EntryVerdict::AlreadyFighting),
    ] {
        let (mut world, _db, _l) = gate_world();
        let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
        // No stone: if the ladder were reordered this would report NoStone.
        world.grand_bosses.get_mut(&ANTHARAS).unwrap().status = status;

        assert_eq!(
            crate::game_loop::antharas::try_enter(&mut world, LEADER),
            expected
        );
    }
}

/// **Only the leader may bring a group in.** A member who talks to the Heart
/// is refused rather than entering alone.
#[test]
fn a_party_member_cannot_let_the_group_in() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    let _rx2 = ingame_caster(&mut world, 2, MEMBER, 20, 0);
    make_party(&mut world, &[LEADER, MEMBER], LootRule::FindersKeepers);
    give_stone(&mut world, MEMBER);

    assert_eq!(
        crate::game_loop::antharas::try_enter(&mut world, MEMBER),
        EntryVerdict::NotLeader
    );
}

/// The leader brings the party — but **only members gathered at the Heart**.
#[test]
fn the_leader_brings_only_nearby_members() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    let _rx2 = ingame_caster(&mut world, 2, MEMBER, 20, 0);
    let straggler = MEMBER + 1;
    let _rx3 = ingame_caster(&mut world, 3, straggler, 0, 0);
    world
        .objects
        .get_component_mut::<Position>(&straggler)
        .unwrap()
        .x = 500_000;
    make_party(
        &mut world,
        &[LEADER, MEMBER, straggler],
        LootRule::FindersKeepers,
    );
    give_stone(&mut world, LEADER);

    match crate::game_loop::antharas::try_enter(&mut world, LEADER) {
        EntryVerdict::Admitted(v) => {
            assert!(
                v.contains(&LEADER) && v.contains(&MEMBER),
                "the gathered two came"
            );
            assert!(!v.contains(&straggler), "the one who wandered off did not");
        }
        other => panic!("expected admission, got {other:?}"),
    }
}

/// **A group that would overfill the lair is refused outright**, not admitted
/// up to the limit — a raid is never split in half by the doorway.
///
/// Reached via `try_enter_with_occupancy`, which exists precisely so this rung
/// is testable: filling a 200-player lair for real is impractical, and a branch
/// no test can reach is a branch nothing checks.
#[test]
fn a_group_too_large_for_the_remaining_room_is_refused() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    let _rx2 = ingame_caster(&mut world, 2, MEMBER, 20, 0);
    make_party(&mut world, &[LEADER, MEMBER], LootRule::FindersKeepers);
    give_stone(&mut world, LEADER);

    // 199 already inside: room for one, and the party is two.
    assert_eq!(
        crate::game_loop::antharas::try_enter_with_occupancy(&mut world, LEADER, 199),
        EntryVerdict::LairFull,
        "refused outright rather than admitting only the leader"
    );
    // Room for both, and they get in.
    assert!(matches!(
        crate::game_loop::antharas::try_enter_with_occupancy(&mut world, LEADER, 198),
        EntryVerdict::Admitted(_)
    ));
}

/// A full lair refuses before anything else is considered.
#[test]
fn a_full_lair_refuses_immediately() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    give_stone(&mut world, LEADER);

    assert_eq!(
        crate::game_loop::antharas::try_enter_with_occupancy(&mut world, LEADER, 200),
        EntryVerdict::LairFull
    );
}

// ---------------------------------------------------------------------------
// Skill selection (slice 19)
// ---------------------------------------------------------------------------

use crate::game_loop::antharas::Choice;

const ANTH_JUMP: i32 = 4106;
const ANTH_TAIL: i32 = 4107;
const ANTH_DEBUFF: i32 = 4109;
const ANTH_MOUTH: i32 = 4110;
const ANTH_NORM_ATTACK: i32 = 4112;
const ATTACKER: i32 = 9950;

/// Antharas at the origin, and a target placed by hand so the arcs are exact.
fn skill_world(
    target_at: (i32, i32),
) -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    add_test_npc(
        &mut world,
        ATTACKER,
        BEHEMOTH,
        "Monster",
        80,
        target_at.0,
        target_at.1,
        0,
    );
    (world, db, l)
}

fn wound_to(world: &mut World, fraction: f64) {
    let v = world
        .objects
        .get_component_mut::<Vitals>(&ANTHARAS_OID)
        .unwrap();
    v.cur_hp = v.max_hp as f64 * fraction;
}

/// Force a run of rolls, then choose.
fn choose(world: &mut World, rolls: &[i32]) -> Option<Choice> {
    for r in rolls {
        world.forced_rolls.push_back(*r);
    }
    crate::game_loop::antharas::choose_skill(world, ANTHARAS_OID, ATTACKER)
}

/// **The tail sweep is gated on an absolute world angle, not on Antharas's
/// facing.** Java compares `calculateDirectionTo(target)` — plain `atan2` —
/// against a window around 180°, and never subtracts his heading. So the sweep
/// lands on a target due **west** of him whichever way he is turned.
///
/// This reads like a rear-arc check that lost its heading term, but the
/// datapack is the specification and "fixing" it would change how often the
/// tail lands. Pinned here so a later reader can see it is deliberate.
#[test]
fn the_tail_sweep_is_gated_on_world_west_not_on_facing() {
    // Due west, well inside 1423 units.
    let (mut world, _db, _l) = skill_world((-800, 0));
    world
        .objects
        .get_component_mut::<Position>(&ANTHARAS_OID)
        .unwrap()
        .heading = 0; // facing east
    let c = choose(&mut world, &[0]).unwrap();
    assert_eq!(
        c.skill_id, ANTH_TAIL,
        "west of him: the tail lands even facing away"
    );

    // Same distance, due east — the mirror position, and no tail.
    let (mut world, _db, _l) = skill_world((800, 0));
    world
        .objects
        .get_component_mut::<Position>(&ANTHARAS_OID)
        .unwrap()
        .heading = 32_768; // facing west
    let c = choose(&mut world, &[0, 99, 99, 99, 99, 1, 99]).unwrap();
    assert_ne!(
        c.skill_id, ANTH_TAIL,
        "east of him: no tail, however he faces"
    );
}

/// The arc gate is a **distance-and-angle pair**: the wide window is short
/// ranged and the narrow one reaches further. A target 1000 units west is
/// inside the far window; the same target at 1500 is outside both.
#[test]
fn the_tail_arc_has_two_windows() {
    for (x, expect_tail) in [(-1000, true), (-1500, false)] {
        let (mut world, _db, _l) = skill_world((x, 0));
        let c = choose(&mut world, &[0, 99, 99, 99, 99, 1, 99]).unwrap();
        assert_eq!(c.skill_id == ANTH_TAIL, expect_tail, "target at x={x}");
    }
}

/// **The curse only exists below half health.** A party that has seen Curse of
/// Antharas has burnt him past 50% — above it the branch is not even rolled.
#[test]
fn the_curse_is_a_below_half_health_skill() {
    // Inside the debuff arc (west, 400 units) but outside the tail's far
    // window, so the tail cannot pre-empt it.
    let angles = |w: &mut World| choose(w, &[99, 0]);

    let (mut world, _db, _l) = skill_world((-400, -100));
    wound_to(&mut world, 0.4);
    assert_eq!(
        angles(&mut world).unwrap().skill_id,
        ANTH_DEBUFF,
        "below half: cursed"
    );

    let (mut world, _db, _l) = skill_world((-400, -100));
    wound_to(&mut world, 0.6);
    assert_ne!(
        angles(&mut world).unwrap().skill_id,
        ANTH_DEBUFF,
        "above half: never"
    );
}

/// **Below 25% he leads with the Breath Attack**, before distance or angle is
/// consulted at all — the one skill that opens a band rather than closing it.
#[test]
fn the_breath_attack_opens_the_final_band() {
    let (mut world, _db, _l) = skill_world((5000, 5000)); // far away, out of every arc
    wound_to(&mut world, 0.1);
    assert_eq!(choose(&mut world, &[0]).unwrap().skill_id, ANTH_MOUTH);

    // At 26% the same roll gets nothing of the sort.
    let (mut world, _db, _l) = skill_world((5000, 5000));
    wound_to(&mut world, 0.26);
    assert_ne!(choose(&mut world, &[0]).unwrap().skill_id, ANTH_MOUTH);
}

/// Every roll missing falls through to the ordinary attack, in **all four
/// bands** — the ladder always yields something.
#[test]
fn every_band_falls_back_to_the_ordinary_attack() {
    for fraction in [1.0, 0.6, 0.4, 0.1] {
        let (mut world, _db, _l) = skill_world((5000, 5000));
        wound_to(&mut world, fraction);
        let c = choose(&mut world, &[99; 12]).unwrap();
        assert_eq!(c.skill_id, ANTH_NORM_ATTACK, "at {fraction} health");
        assert!(!c.on_self, "the ordinary attack is aimed at the target");
    }
}

/// **The areas are cast on Antharas himself.** Java's `castOnTarget == false`
/// means the tail sweep, the curse and the stomp take *him* as their target —
/// they are centred on the boss, not on the player who drew them. Dropping
/// that distinction would make each of them a single-target hit.
#[test]
fn the_area_skills_are_cast_on_antharas_himself() {
    let (mut world, _db, _l) = skill_world((-800, 0));
    assert!(choose(&mut world, &[0]).unwrap().on_self, "tail");

    let (mut world, _db, _l) = skill_world((-400, -100));
    wound_to(&mut world, 0.4);
    assert!(choose(&mut world, &[99, 0]).unwrap().on_self, "curse");

    // The stomp: inside 1100 units but out of both arcs (due east).
    let (mut world, _db, _l) = skill_world((900, 0));
    wound_to(&mut world, 0.4);
    let c = choose(&mut world, &[99, 99, 0]).unwrap();
    assert_eq!(c.skill_id, ANTH_JUMP);
    assert!(c.on_self, "stomp");
}

/// **The end of the chain: a hit actually makes him cast.**
///
/// `manage_skills` and its Baium twin choose a skill; nothing called either
/// until this slice, so both bosses decided into the void and only ever swung.
/// This test goes through the real damage hook and asserts a `MagicSkillUse`
/// leaves the server — it fails against the previous commit.
#[test]
fn a_hit_makes_antharas_cast() {
    let (mut world, _db, _l) = antharas_world();
    fighting_antharas(&mut world); // the damage hook only runs mid-fight
    let mut rx = ingame_caster(&mut world, 1, ATTACKER, -800, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&ANTHARAS_OID)
            .unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: ANTH_TAIL,
            level: 1,
            ..Default::default()
        });
    while rx.try_recv().is_ok() {}

    // No jitter, then the tail's opening roll.
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(0);
    crate::game_loop::antharas::on_antharas_damage(&mut world, ANTHARAS_OID, ATTACKER, 500, true);

    let casts = std::iter::from_fn(|| rx.try_recv().ok())
        .filter(|p| p.first() == Some(&0x48))
        .count();
    assert_eq!(casts, 1, "the damage hook chose a skill and cast it");
}

/// He does not start a second cast while one is running — Java's
/// `if (npc.isCastingNow()) return`, which is what makes calling `manageSkills`
/// on *every* hit safe.
#[test]
fn a_second_hit_mid_cast_starts_nothing() {
    let (mut world, _db, _l) = antharas_world();
    fighting_antharas(&mut world); // the damage hook only runs mid-fight
    let mut rx = ingame_caster(&mut world, 1, ATTACKER, -800, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&ANTHARAS_OID)
            .unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: ANTH_TAIL,
            level: 1,
            hit_time: 5_000,
            ..Default::default()
        });
    for _ in 0..3 {
        world.forced_rolls.push_back(0);
    }
    crate::game_loop::antharas::on_antharas_damage(&mut world, ANTHARAS_OID, ATTACKER, 500, true);
    while rx.try_recv().is_ok() {}

    for _ in 0..3 {
        world.forced_rolls.push_back(0);
    }
    crate::game_loop::antharas::on_antharas_damage(&mut world, ANTHARAS_OID, ATTACKER, 500, true);
    let casts = std::iter::from_fn(|| rx.try_recv().ok())
        .filter(|p| p.first() == Some(&0x48))
        .count();
    assert_eq!(casts, 0, "still casting the first");
}

// ---------------------------------------------------------------------------
// Slice 20: the entry flow wired — Heart of Warding → WAITING → SPAWN.
// ---------------------------------------------------------------------------

const DIST_GAME: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

/// The full arc: an admitted player is teleported in, WAITING is set, and the
/// clock is NOT restarted by a second entrant — the boss takes the platform
/// exactly one window after the FIRST entry, crossing regions, flipping
/// IN_FIGHT, sounding the lair and arming the camera chain.
#[test]
fn the_heart_admits_waits_and_spawns_antharas() {
    let (mut world, _db, _l) = gate_world();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST_GAME);
    let mut rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    let mut rx2 = ingame_caster(&mut world, 2, MEMBER, 10, 0);
    give_stone(&mut world, LEADER);
    give_stone(&mut world, MEMBER);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    while rx.try_recv().is_ok() {}
    while rx2.try_recv().is_ok() {}

    assert_eq!(
        crate::game_loop::antharas::heart_enter(&mut world, LEADER),
        None,
        "admitted"
    );
    let pos = world
        .objects
        .get_component::<Position>(&LEADER)
        .copied()
        .unwrap();
    assert!(
        (179700..=180400).contains(&pos.x)
            && (113800..=115900).contains(&pos.y)
            && (pos.z - -7709).abs() < 100,
        "teleported into the nest: {pos:?}"
    );
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, ANTHARAS),
        Some(1),
        "WAITING"
    );

    // Half the 20-minute window passes; a second player enters. The clock
    // must NOT restart.
    advance_ticks(&mut world, 10 * 60 * 10);
    assert_eq!(
        crate::game_loop::antharas::heart_enter(&mut world, MEMBER),
        None,
        "second entrant admitted"
    );
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, ANTHARAS),
        Some(1),
        "still WAITING"
    );

    // The remaining half elapses → SPAWN_ANTHARAS fires off the FIRST clock.
    advance_ticks(&mut world, 10 * 60 * 10 + 5);
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, ANTHARAS),
        Some(2),
        "IN_FIGHT"
    );
    let boss_pos = world
        .objects
        .get_component::<Position>(&ANTHARAS_OID)
        .copied()
        .unwrap();
    assert_eq!(
        (boss_pos.x, boss_pos.y, boss_pos.z),
        (181323, 114850, -7623),
        "on the platform"
    );
    // The region index followed the cross-region teleport.
    let new_region = crate::world::region_of(181323, 114850);
    assert!(
        world
            .npc_regions
            .get(&new_region)
            .is_some_and(|ids| ids.contains(&ANTHARAS_OID)),
        "region index moved with him"
    );
    let old_region = crate::world::region_of(0, 0);
    assert!(
        !world
            .npc_regions
            .get(&old_region)
            .is_some_and(|ids| ids.contains(&ANTHARAS_OID)),
        "and left the old cell"
    );
    // The lair heard BS02_A — the entrants stand inside the lair zone, and
    // PlaySound carries the name as UTF-16.
    let mut heard = Vec::new();
    while let Ok(p) = rx.try_recv() {
        heard.push(p);
    }
    let sound: Vec<u8> = "BS02_A"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    assert!(
        heard
            .iter()
            .any(|p| p.first() == Some(&0x9E) && p.windows(sound.len()).any(|w| w == sound)),
        "the lair player heard BS02_A"
    );
}

/// The refusal htmls: a dead Antharas answers 13001-01, a stoneless visitor
/// 13001-03 — the ladder's verdicts drive the window the player reads.
#[test]
fn the_heart_serves_the_refusal_htmls() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);

    assert_eq!(
        crate::game_loop::antharas::heart_enter(&mut world, LEADER),
        Some("13001-03.html"),
        "no stone"
    );
    world.grand_bosses.get_mut(&ANTHARAS).unwrap().status = 3;
    assert_eq!(
        crate::game_loop::antharas::heart_enter(&mut world, LEADER),
        Some("13001-01.html"),
        "dead boss wins over the missing stone"
    );
}

/// The Teleportation Cubic sends a player to the Giran side.
#[test]
fn the_cubic_teleports_out() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);

    crate::game_loop::antharas::teleport_out(&mut world, LEADER);
    let pos = world
        .objects
        .get_component::<Position>(&LEADER)
        .copied()
        .unwrap();
    assert!(
        (79800..=80400).contains(&pos.x)
            && (151200..=152300).contains(&pos.y)
            && (pos.z - -3534).abs() < 100,
        "out to Giran: {pos:?}"
    );
}

/// The status-model fix: a killed Antharas stores DEAD as **3** (not the
/// two-state 1, which the four-state ladder reads as WAITING), entry refuses
/// it as BossDead, and the boot branch still recognises 3 as dead — an
/// elapsed window respawns him.
#[test]
fn a_dead_antharas_stores_three_and_still_respawns() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    give_stone(&mut world, LEADER);

    crate::game_loop::grand_boss::on_grand_boss_killed(&mut world, ANTHARAS);
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, ANTHARAS),
        Some(3),
        "four-state DEAD"
    );
    assert_eq!(
        crate::game_loop::antharas::try_enter(&mut world, LEADER),
        EntryVerdict::BossDead,
        "a dead boss refuses entry even with the stone"
    );

    // The window elapsed while the server was down: boot must respawn him.
    world.grand_bosses.get_mut(&ANTHARAS).unwrap().respawn_time = 1;
    crate::game_loop::grand_boss::resolve_at_boot(&mut world);
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, ANTHARAS),
        Some(0),
        "back to DORMANT"
    );

    // The simple bosses keep the two-state pair.
    world.grand_bosses.insert(
        crate::game_loop::core_boss::CORE,
        crate::model::grand_boss::GrandBoss {
            boss_id: crate::game_loop::core_boss::CORE,
            loc_x: 0,
            loc_y: 0,
            loc_z: 0,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: 0,
        },
    );
    crate::game_loop::grand_boss::on_grand_boss_killed(
        &mut world,
        crate::game_loop::core_boss::CORE,
    );
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, crate::game_loop::core_boss::CORE),
        Some(1),
        "Core's DEAD stays 1"
    );
}

/// The wiring itself — the whole point of this slice: the dist html's
/// `Quest Antharas enter` bypass reaches `heart_enter` through the real
/// bypass router and registered script, not through a direct call. (Slices
/// 12 and 18 both shipped complete, tested, *uncalled* functions; this test
/// is what would have caught them.)
#[test]
fn the_enter_bypass_reaches_the_ladder_through_the_router() {
    let (mut world, _db, _l) = gate_world();
    let _rx = ingame_caster(&mut world, 1, LEADER, 0, 0);
    give_stone(&mut world, LEADER);
    let heart_oid = NPC_OID + 130;
    world.data.npc_data.insert_for_test({
        let mut t = crate::data::npc_data::default_template(13001);
        t.type_name = "Folk".into();
        t
    });
    add_test_npc(&mut world, heart_oid, 13001, "Folk", 80, 20, 0, 0);
    world
        .objects
        .add_components(&LEADER, crate::model::components::LastFolkNpc(heart_oid));

    crate::game_loop::bypass::handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("Quest Antharas enter"),
    );

    let pos = world
        .objects
        .get_component::<Position>(&LEADER)
        .copied()
        .unwrap();
    assert!(
        (179700..=180400).contains(&pos.x),
        "the bypass admitted and teleported through the real router: {pos:?}"
    );
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, ANTHARAS),
        Some(1),
        "WAITING set"
    );

    // And the cubic's bypass sends them back out — the cubic stands inside
    // the nest beside the teleported player (the Heart is 180k units away
    // now, and the bypass distance guard would rightly refuse it).
    let cube_oid = NPC_OID + 131;
    world.data.npc_data.insert_for_test({
        let mut t = crate::data::npc_data::default_template(31859);
        t.type_name = "Folk".into();
        t
    });
    add_test_npc(
        &mut world,
        cube_oid,
        31859,
        "Folk",
        80,
        pos.x + 20,
        pos.y,
        pos.z,
    );
    world
        .objects
        .add_components(&LEADER, crate::model::components::LastFolkNpc(cube_oid));
    crate::game_loop::bypass::handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("Quest Antharas teleportOut"),
    );
    let pos = world
        .objects
        .get_component::<Position>(&LEADER)
        .copied()
        .unwrap();
    assert!(
        (79800..=80400).contains(&pos.x),
        "teleportOut through the router: {pos:?}"
    );
}

// ---------------------------------------------------------------------------
// The death tail — exit cube + zone clear (`onKill` / `CLEAR_ZONE`).
// ---------------------------------------------------------------------------

const CUBE: i32 = 31859;
/// A point inside the lair zone (12016) — Java's death-cube location.
const LAIR_POINT: (i32, i32, i32) = (177615, 114941, -7709);
const MINION_A: i32 = NPC_OID + 140;
const MINION_B: i32 = NPC_OID + 141;
const KILLER: i32 = 9960;

fn register_cube(world: &mut World) {
    let mut t = crate::data::npc_data::default_template(CUBE);
    t.type_name = "Folk".into();
    world.data.npc_data.insert_for_test(t);
}

fn cube_in_lair(world: &World) -> Option<i32> {
    let zone = world.data.zone_data.by_id(70050)?;
    world.npc_regions.values().flatten().copied().find(|oid| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(oid)
            .is_some_and(|n| n.npc_id == CUBE)
            && world
                .objects
                .get_component::<Position>(oid)
                .is_some_and(|p| zone.contains(p.x, p.y, p.z))
    })
}

/// Killing Antharas (through the real `npc_do_die` death path) despawns the
/// adds, drops the exit cube in the lair, and arms the 15-minute zone clear.
#[test]
fn killing_antharas_spawns_the_exit_cube_and_clears_minions() {
    let (mut world, _db, _l) = gate_world();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST_GAME);
    register_cube(&mut world);
    let _rx = ingame_caster(&mut world, 1, KILLER, LAIR_POINT.0, LAIR_POINT.1);
    add_test_npc(
        &mut world,
        ANTHARAS_OID,
        ANTHARAS,
        "GrandBoss",
        85,
        LAIR_POINT.0,
        LAIR_POINT.1,
        LAIR_POINT.2,
    );
    add_test_npc(
        &mut world,
        MINION_A,
        BEHEMOTH,
        "Monster",
        85,
        LAIR_POINT.0,
        LAIR_POINT.1,
        LAIR_POINT.2,
    );
    add_test_npc(
        &mut world,
        MINION_B,
        TERASQUE,
        "Monster",
        85,
        LAIR_POINT.0,
        LAIR_POINT.1,
        LAIR_POINT.2,
    );
    assert_eq!(spawned(&mut world), 2, "two adds before the kill");

    crate::game_loop::death::npc_do_die(&mut world, ANTHARAS_OID, KILLER);

    assert_eq!(spawned(&mut world), 0, "DESPAWN_MINIONS cleared the adds");
    assert!(
        cube_in_lair(&world).is_some(),
        "the exit cube stands in the lair"
    );
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(t, ScheduledTask::AntharasClearZone)),
        "CLEAR_ZONE armed"
    );
}

/// The auto-spawned death cube is talkable: `Quest Antharas teleportOut`
/// through the real bypass router sends the player to the Giran-side exit.
#[test]
fn the_death_cube_teleports_out_through_the_router() {
    let (mut world, _db, _l) = gate_world();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST_GAME);
    register_cube(&mut world);
    let _rx = ingame_caster(&mut world, 1, KILLER, LAIR_POINT.0, LAIR_POINT.1);
    world
        .objects
        .get_component_mut::<Position>(&KILLER)
        .unwrap()
        .z = LAIR_POINT.2; // beside the cube, deep underground
    add_test_npc(
        &mut world,
        ANTHARAS_OID,
        ANTHARAS,
        "GrandBoss",
        85,
        LAIR_POINT.0,
        LAIR_POINT.1,
        LAIR_POINT.2,
    );

    crate::game_loop::death::npc_do_die(&mut world, ANTHARAS_OID, KILLER);
    let cube_oid = cube_in_lair(&world).expect("cube spawned on death");

    // The killer stands beside the cube; the named bypass reaches teleportOut.
    world
        .objects
        .add_components(&KILLER, crate::model::components::LastFolkNpc(cube_oid));
    crate::game_loop::bypass::handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("Quest Antharas teleportOut"),
    );

    let pos = world
        .objects
        .get_component::<Position>(&KILLER)
        .copied()
        .unwrap();
    assert!(
        (79800..=80400).contains(&pos.x) && (151200..=152300).contains(&pos.y),
        "the death cube teleported the player out: {pos:?}"
    );
}

/// The scheduled `CLEAR_ZONE` task, fired through the loop dispatch, ousts a
/// lingering player to the exit and despawns the cube (and any straggler).
#[test]
fn clear_zone_ousts_players_and_despawns_the_cube_through_the_loop() {
    let (mut world, _db, _l) = gate_world();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST_GAME);
    register_cube(&mut world);
    let _rx = ingame_caster(&mut world, 1, KILLER, LAIR_POINT.0, LAIR_POINT.1);
    world
        .objects
        .get_component_mut::<Position>(&KILLER)
        .unwrap()
        .z = LAIR_POINT.2; // the lair sits deep underground
    add_test_npc(
        &mut world,
        ANTHARAS_OID,
        ANTHARAS,
        "GrandBoss",
        85,
        LAIR_POINT.0,
        LAIR_POINT.1,
        LAIR_POINT.2,
    );
    crate::game_loop::death::npc_do_die(&mut world, ANTHARAS_OID, KILLER);
    assert!(
        cube_in_lair(&world).is_some(),
        "cube present before the clear"
    );

    // Fire the armed clear immediately through the real dispatch.
    world
        .scheduler
        .schedule(world.tick, ScheduledTask::AntharasClearZone);
    advance_ticks(&mut world, 1);

    let pos = world
        .objects
        .get_component::<Position>(&KILLER)
        .copied()
        .unwrap();
    assert!(
        (79800..=80400).contains(&pos.x),
        "the lingering player was ousted to the exit: {pos:?}"
    );
    assert!(
        cube_in_lair(&world).is_none(),
        "the cube was despawned with the zone"
    );
}

// ---------------------------------------------------------------------------
// SET_REGEN + CHECK_ATTACK + the strider leg (the lifecycle/onAttack gaps)
// ---------------------------------------------------------------------------

use crate::game_loop::antharas::AntharasCombat;

const AP_PLAYER: i32 = 9700;

fn fighting_antharas(world: &mut World) {
    world.grand_bosses.insert(
        ANTHARAS,
        crate::model::grand_boss::GrandBoss {
            boss_id: ANTHARAS,
            loc_x: 0,
            loc_y: 0,
            loc_z: 0,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: 2, // IN_FIGHT
        },
    );
}

fn casting_skill(world: &World, oid: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::components::Casting>(&oid)
        .map(|c| c.0.skill_id)
}

fn give_mp(world: &mut World, oid: i32) {
    let v = world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&oid)
        .unwrap();
    v.max_mp = 100_000;
    v.cur_mp = 100_000.0;
}

/// **Antharas heals harder the lower his health.** At 40% HP he is in the
/// third band, so he self-casts regen skill 4240 — not the weakest (4125).
#[test]
fn antharas_heals_for_his_current_hp_band() {
    let (mut world, _db, _l) = antharas_world();
    fighting_antharas(&mut world);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    give_mp(&mut world, ANTHARAS_OID);
    wound_to(&mut world, 0.4); // < 50%, ≥ 25% -> band 2 -> 4240
    world.objects.add_components(
        &ANTHARAS_OID,
        AntharasCombat {
            last_attack_tick: world.tick,
        },
    );
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: 4240,
            level: 1,
            ..Default::default()
        });
    let before = world.scheduler.len();

    crate::game_loop::antharas::handle_set_regen(&mut world, ANTHARAS_OID);

    assert_eq!(
        casting_skill(&world, ANTHARAS_OID),
        Some(4240),
        "cast the band-2 regeneration"
    );
    assert!(world.scheduler.len() > before, "the regen beat re-armed");
}

/// The regen beat stops once the fight is over.
#[test]
fn the_regen_beat_stops_when_the_fight_is_over() {
    let (mut world, _db, _l) = antharas_world();
    fighting_antharas(&mut world);
    world.grand_bosses.get_mut(&ANTHARAS).unwrap().status = 3; // DEAD
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    let before = world.scheduler.len();

    crate::game_loop::antharas::handle_set_regen(&mut world, ANTHARAS_OID);

    assert_eq!(world.scheduler.len(), before, "no re-arm");
    assert_eq!(casting_skill(&world, ANTHARAS_OID), None, "no cast");
}

/// **Fifteen idle minutes abandon the fight:** Antharas is parked at his resting
/// spot and reverts to the resting (re-enterable) status.
#[test]
fn a_fifteen_minute_idle_resets_antharas() {
    let (mut world, _db, _l) = antharas_world();
    fighting_antharas(&mut world);
    add_test_npc(
        &mut world,
        ANTHARAS_OID,
        ANTHARAS,
        "GrandBoss",
        85,
        179_011,
        114_871,
        -7_704,
    );
    world.objects.add_components(
        &ANTHARAS_OID,
        AntharasCombat {
            last_attack_tick: 0,
        },
    );
    world.tick = 9_001; // > 15 min since last_attack 0

    crate::game_loop::antharas::handle_check_attack(&mut world, ANTHARAS_OID);

    assert_eq!(
        world.grand_bosses.get(&ANTHARAS).unwrap().status,
        0,
        "reverted to ALIVE"
    );
    let p = world
        .objects
        .get_component::<crate::model::components::Position>(&ANTHARAS_OID)
        .unwrap();
    assert_eq!(
        (p.x, p.y, p.z),
        (185_708, 114_298, -8_221),
        "parked at his resting spot"
    );
}

/// A recent hit keeps Antharas fighting and re-arms the beat.
#[test]
fn a_recently_hit_antharas_keeps_fighting() {
    let (mut world, _db, _l) = antharas_world();
    fighting_antharas(&mut world);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    world.tick = 10_000;
    world.objects.add_components(
        &ANTHARAS_OID,
        AntharasCombat {
            last_attack_tick: world.tick,
        },
    );
    let before = world.scheduler.len();

    crate::game_loop::antharas::handle_check_attack(&mut world, ANTHARAS_OID);

    assert_eq!(
        world.grand_bosses.get(&ANTHARAS).unwrap().status,
        2,
        "still IN_FIGHT"
    );
    assert!(world.scheduler.len() > before, "the beat re-armed");
}

/// A strider-mounted attacker is hindered (skill 4258) — the `onAttack` leg
/// that was missing.
#[test]
fn a_strider_rider_is_hindered_by_antharas() {
    let (mut world, _db, _l) = antharas_world();
    fighting_antharas(&mut world);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    give_mp(&mut world, ANTHARAS_OID);
    world.objects.add_components(
        &ANTHARAS_OID,
        AntharasCombat {
            last_attack_tick: 0,
        },
    );
    let _rx = ingame_player(&mut world, 3, AP_PLAYER, 20, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&AP_PLAYER)
        .unwrap()
        .mount_type = 1; // STRIDER
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: 4258,
            level: 1,
            ..Default::default()
        });

    crate::game_loop::antharas::on_antharas_damage(&mut world, ANTHARAS_OID, AP_PLAYER, 100, true);

    assert_eq!(
        casting_skill(&world, ANTHARAS_OID),
        Some(4258),
        "the rider was hindered"
    );
}
