//! Orfen — the drag, and the half-HP relocation.

use super::*;

use crate::game_loop::orfen::{ORFEN, RIBA_IREN};

const ORFEN_OID: i32 = NPC_OID + 90;
const PLAYER: i32 = 9970;
const CID: u32 = 1;
const PARALYSIS: i32 = 4064;
const ORFEN_HEAL: i32 = 4516;

fn orfen_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    for id in [ORFEN, RIBA_IREN] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = if id == ORFEN {
            "RaidBoss".into()
        } else {
            "Monster".into()
        };
        t.level = 50;
        t.base_hp_max = 10_000.0;
        t.base_mp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    // The two skills do different things, and giving them the same effect list
    // is how the first draft of `riba_iren_heals_on_its_own_wounds` passed
    // while measuring nothing.
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: PARALYSIS,
            level: 1,
            abnormal_time: 60,
            effects: vec![crate::model::skill::SkillEffect::BlockActions { conditional: false }],
            ..Default::default()
        });
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: ORFEN_HEAL,
            level: 1,
            magic_type: 1,
            effects: vec![crate::model::skill::SkillEffect::Heal { power: 1000.0 }],
            ..Default::default()
        });
    (world, db, l)
}

/// Put the player at a given 2D distance from Orfen.
fn place_player_at(world: &mut World, dist: i32) {
    world
        .objects
        .get_component_mut::<Position>(&PLAYER)
        .unwrap()
        .x = dist;
}

fn player_pos(world: &World) -> (i32, i32) {
    let p = world.objects.get_component::<Position>(&PLAYER).unwrap();
    (p.x, p.y)
}

/// **The drag**: a mid-range attacker is yanked onto Orfen. The roll is forced
/// so the mechanic, not the RNG, is what is under test.
#[test]
fn a_mid_range_attacker_is_dragged_onto_orfen() {
    let (mut world, _db, _l) = orfen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    place_player_at(&mut world, 600); // inside the 300..1000 band
    world.forced_rolls.push_back(0); // getRandom(10) == 0

    crate::game_loop::orfen::on_orfen_attacked(&mut world, ORFEN_OID, PLAYER);

    assert_eq!(player_pos(&world), (0, 0), "dragged to Orfen's position");
}

/// **Melee is never dragged.** Inside 300 units Orfen ignores you — the band
/// is what makes this an anti-ranged mechanic rather than a random yank.
#[test]
fn a_melee_attacker_is_not_dragged() {
    let (mut world, _db, _l) = orfen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    place_player_at(&mut world, 150);
    world.forced_rolls.push_back(0);

    crate::game_loop::orfen::on_orfen_attacked(&mut world, ORFEN_OID, PLAYER);
    assert_eq!(player_pos(&world), (150, 0), "melee range, left alone");
}

/// Beyond 1000 units is out of reach entirely.
#[test]
fn a_distant_attacker_is_not_dragged() {
    let (mut world, _db, _l) = orfen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    place_player_at(&mut world, 5000);
    world.forced_rolls.push_back(0);

    crate::game_loop::orfen::on_orfen_attacked(&mut world, ORFEN_OID, PLAYER);
    assert_eq!(player_pos(&world), (5000, 0), "out of reach");
}

/// The drag is a 1-in-10 chance, not every hit.
#[test]
fn the_drag_is_a_one_in_ten_chance() {
    let (mut world, _db, _l) = orfen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    place_player_at(&mut world, 600);
    world.forced_rolls.push_back(3); // any non-zero

    crate::game_loop::orfen::on_orfen_attacked(&mut world, ORFEN_OID, PLAYER);
    assert_eq!(player_pos(&world), (600, 0), "the roll failed, no drag");
}

/// Orfen relocates the first time it drops below half — and **only** the first
/// time, or it would teleport on every subsequent hit.
#[test]
fn orfen_relocates_once_at_half_health() {
    let (mut world, _db, _l) = orfen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&ORFEN_OID)
            .unwrap();
        v.cur_hp = v.max_hp as f64 * 0.4;
    }

    crate::game_loop::orfen::on_orfen_attacked(&mut world, ORFEN_OID, PLAYER);
    let after_first = *world.objects.get_component::<Position>(&ORFEN_OID).unwrap();
    assert_eq!(
        (after_first.x, after_first.y),
        (43728, 17220),
        "moved to its home point"
    );

    // Shove it elsewhere and hit it again: it must not relocate a second time.
    world
        .objects
        .get_component_mut::<Position>(&ORFEN_OID)
        .unwrap()
        .x = 999;
    crate::game_loop::orfen::on_orfen_attacked(&mut world, ORFEN_OID, PLAYER);
    assert_eq!(
        world
            .objects
            .get_component::<Position>(&ORFEN_OID)
            .unwrap()
            .x,
        999,
        "once per life, not once per hit"
    );
}

/// Above half health it stays put.
#[test]
fn orfen_does_not_relocate_above_half_health() {
    let (mut world, _db, _l) = orfen_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    place_player_at(&mut world, 5000); // out of drag range, isolating the test

    crate::game_loop::orfen::on_orfen_attacked(&mut world, ORFEN_OID, PLAYER);
    assert_eq!(
        world
            .objects
            .get_component::<Position>(&ORFEN_OID)
            .unwrap()
            .x,
        0,
        "still home"
    );
}

/// **Riba Iren heals Orfen when *it* is hurt, not when Orfen is** — the
/// opposite of every other healer minion, and easy to port backwards.
#[test]
fn riba_iren_heals_on_its_own_wounds() {
    let (mut world, _db, _l) = orfen_world();
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    let minion = ORFEN_OID + 1;
    add_test_npc(&mut world, minion, RIBA_IREN, "Monster", 50, 40, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&minion).unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
        v.cur_hp = v.max_hp as f64 * 0.4; // the *minion* is hurt
    }
    // Orfen is wounded too, so the heal has somewhere to land and the
    // assertion can measure it.
    let orfen_before = {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&ORFEN_OID)
            .unwrap();
        v.cur_hp = v.max_hp as f64 / 2.0;
        v.cur_hp
    };

    crate::game_loop::orfen::on_riba_iren_attacked(&mut world, minion);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert!(
        world
            .objects
            .get_component::<Vitals>(&ORFEN_OID)
            .unwrap()
            .cur_hp
            > orfen_before,
        "the wounded minion healed Orfen"
    );
}

/// The converse, which is the half that would be wrong if ported backwards: a
/// **healthy** minion does nothing, however hurt Orfen is.
#[test]
fn a_healthy_riba_iren_does_not_heal_orfen() {
    let (mut world, _db, _l) = orfen_world();
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    let minion = ORFEN_OID + 1;
    add_test_npc(&mut world, minion, RIBA_IREN, "Monster", 50, 40, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&minion).unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0; // minion at full health
    }
    let orfen_before = {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&ORFEN_OID)
            .unwrap();
        v.cur_hp = v.max_hp as f64 * 0.1; // Orfen nearly dead
        v.cur_hp
    };

    crate::game_loop::orfen::on_riba_iren_attacked(&mut world, minion);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&ORFEN_OID)
            .unwrap()
            .cur_hp,
        orfen_before,
        "a healthy minion ignores a dying master"
    );
}

/// The leash: dragged more than 10000 from her spawn, Orfen drops her hate (and
/// heads back); kept close, she keeps fighting.
#[test]
fn the_leash_resets_a_dragged_orfen() {
    use crate::model::npc::{AggroInfo, AggroList};

    let add_hate = |world: &mut World, oid: i32| {
        world
            .objects
            .get_component_mut::<AggroList>(&oid)
            .unwrap()
            .0
            .insert(
                PLAYER,
                AggroInfo {
                    hate: 100.0,
                    damage: 0.0,
                },
            );
    };
    let has_hate = |world: &World, oid: i32| {
        !world
            .objects
            .get_component::<AggroList>(&oid)
            .unwrap()
            .0
            .is_empty()
    };

    // Spawned at the origin → home is (0,0,0).
    let (mut world, _db, _l) = orfen_world();
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    crate::game_loop::orfen::on_orfen_spawned(&mut world, ORFEN_OID);
    add_hate(&mut world, ORFEN_OID);

    // Still near home: the leash leaves her be.
    crate::game_loop::orfen::handle_distance_check(&mut world, ORFEN_OID);
    assert!(has_hate(&world, ORFEN_OID), "at home she keeps fighting");

    // Dragged far past the 10000 leash → she drops her hate.
    if let Some(p) = world.objects.get_component_mut::<Position>(&ORFEN_OID) {
        p.x = 50_000;
    }
    crate::game_loop::orfen::handle_distance_check(&mut world, ORFEN_OID);
    assert!(!has_hate(&world, ORFEN_OID), "dragged too far, she resets");
}

/// `OnAttackableFactionCall`'s Orfen listener, Riba Iren arm: a faction call
/// about a half-dead Orfen has a 9-in-10 chance of an immediate Orfen Heal at
/// her; a healthy caller is ignored, and the roll can miss.
#[test]
fn riba_faction_call_heals_half_dead_orfen() {
    use crate::model::components::{Casting, Vitals};

    let (mut world, _db, _l) = orfen_world();
    add_test_npc(&mut world, ORFEN_OID, ORFEN, "RaidBoss", 50, 0, 0, 0);
    let riba = ORFEN_OID + 1;
    add_test_npc(&mut world, riba, RIBA_IREN, "Monster", 50, 30, 0, 0);
    let _rx = ingame_caster(&mut world, CID, PLAYER, 200, 0);

    // Healthy Orfen: nothing, no roll consumed.
    crate::game_loop::ai::on_faction_call_script_for_test(&mut world, riba, ORFEN_OID, PLAYER);
    assert!(!world.objects.has_component::<Casting>(&riba));

    // Half-dead Orfen, roll 9 (>= chance 9): the 1-in-10 miss.
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&ORFEN_OID)
            .unwrap();
        v.cur_hp = v.max_hp as f64 * 0.3;
    }
    world.forced_rolls.push_back(9);
    crate::game_loop::ai::on_faction_call_script_for_test(&mut world, riba, ORFEN_OID, PLAYER);
    assert!(
        !world.objects.has_component::<Casting>(&riba),
        "roll 9 misses the 9-in-10 chance"
    );

    // Roll 0: the heal fires.
    world.forced_rolls.push_back(0);
    crate::game_loop::ai::on_faction_call_script_for_test(&mut world, riba, ORFEN_OID, PLAYER);
    assert!(
        world.objects.has_component::<Casting>(&riba),
        "the recruited Riba Iren heals the half-dead Orfen"
    );
}
