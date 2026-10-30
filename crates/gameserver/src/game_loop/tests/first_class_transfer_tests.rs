//! The Salvation first class transfer: Start of Fate (10331) and its
//! instance, the Labyrinth of Belis (178), on the real templates and htmls.

use super::*;
use crate::game_loop::quests;
use crate::model::components::space::{InMovie, NpcFollow};
use crate::model::npc::NpcAi;
use crate::network::server_packets::opcodes::{TUTORIAL_SHOW_HTML, TUTORIAL_SHOW_QUESTION_MARK};

const PLAYER: i32 = 7301;
const Q10331: &str = "Q10331_StartOfFate";
const BELIS: i32 = 178;

const SEBION: i32 = 32978;
const FRANCO: i32 = 32153;
const OFFICER: i32 = 19155;
const VERIFIER: i32 = 33215;
const OPERATIVE: i32 = 22998;
const GENERATOR: i32 = 33216;
const NEMERTESS: i32 = 22984;

const SARIL_NECKLACE: i32 = 17580;
const BELIS_MARK: i32 = 17615;

/// Where the instance's exit (and Sebion's spot) is.
const OUTSIDE: (i32, i32, i32) = (-111782, 231892, -3178);

fn fate_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    // `quest_test_world` gives the player an item id pool to grow from.
    let (mut world, ..) = quest_test_world();
    world.data = dist::game_data_owned();
    let rx = ingame_player(&mut world, 1, PLAYER, OUTSIDE.0, OUTSIDE.1, OUTSIDE.2);
    set_level(&mut world, PLAYER, 20);
    (world, rx)
}

fn event(world: &mut World, npc: i32, script: &str, event: &str) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {script} {event}")),
    );
}

fn talk(world: &mut World, npc: i32) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {Q10331}")),
    );
}

fn first_talk(world: &mut World, npc: i32) {
    let npc_id = game_loop::npc::npc_id_of(world, npc).unwrap();
    world
        .objects
        .add_components(&PLAYER, crate::model::components::player::LastFolkNpc(npc));
    quests::notify_first_talk(world, 1, PLAYER, npc, npc_id);
}

fn stand_by(world: &mut World, npc: i32) {
    let p = *world.objects.get_component::<Position>(&npc).unwrap();
    if let Some(me) = world.objects.get_component_mut::<Position>(&PLAYER) {
        (me.x, me.y, me.z) = (p.x + 50, p.y, p.z);
    }
}

fn spawn_outside(world: &mut World, npc_id: i32) -> i32 {
    game_loop::npc::spawn_npc_at(world, npc_id, OUTSIDE.0 + 50, OUTSIDE.1, OUTSIDE.2, 0)
        .expect("spawned")
}

/// The NPCs of the instance with `npc_id` that are still in the world.
fn in_world(world: &World, id: i32, npc_id: i32) -> Vec<i32> {
    world
        .instances
        .get(id)
        .unwrap()
        .npcs
        .iter()
        .copied()
        .filter(|oid| game_loop::npc::npc_id_of(world, *oid) == Some(npc_id))
        .collect()
}

fn script_value(world: &World, npc: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc)
        .unwrap()
        .script_value
}

fn status(world: &World, id: i32) -> i32 {
    world.instances.status(id)
}

/// A started Q10331 and a fresh Labyrinth with the player beside the officer.
fn in_labyrinth() -> (World, UnboundedReceiver<bytes::Bytes>, i32, i32) {
    let (mut world, rx) = fate_world();
    let sebion = spawn_outside(&mut world, SEBION);
    talk(&mut world, sebion);
    event(&mut world, sebion, Q10331, "32978-03.htm");
    event(&mut world, sebion, "LabyrinthOfBelis", "enter_instance");
    let id = world
        .instances
        .allowed_instance_of(PLAYER)
        .expect("entered");
    let officer = in_world(&world, id, OFFICER)[0];
    stand_by(&mut world, officer);
    (world, rx, id, officer)
}

/// Login and the level-up to exactly 18 show mark 10331 while the quest is
/// not taken; its click opens the race's popup.
#[test]
fn the_class_mark_shows_from_level_18_and_opens_the_race_popup() {
    let (mut world, mut rx) = fate_world();
    set_level(&mut world, PLAYER, 17);
    drain(&mut rx);
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));

    set_level(&mut world, PLAYER, 18);
    quests::dispatch::notify_level_changed(&mut world, PLAYER, 17, 18);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
    quests::dispatch::notify_level_changed(&mut world, PLAYER, 18, 19);
    assert!(
        !has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK),
        "only the step onto 18"
    );
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));

    quests::notify_tutorial_mark(&mut world, 1, PLAYER, 10331);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_HTML));

    world.cfg.character.disable_tutorial = true;
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
}

/// Sebion starts the quest, and the bare `Script LabyrinthOfBelis
/// enter_instance` link on his page puts the player in a fresh Labyrinth.
#[test]
fn sebion_starts_the_quest_and_sends_the_player_into_the_labyrinth() {
    let (mut world, mut rx) = fate_world();
    let sebion = spawn_outside(&mut world, SEBION);
    drain(&mut rx);
    talk(&mut world, sebion);
    assert_eq!(
        any_html(&mut rx),
        dist_page(
            "data/scripts/quests/Q10331_StartOfFate/32978-01.htm",
            sebion
        )
    );
    event(&mut world, sebion, Q10331, "32978-03.htm");
    assert_eq!(quest_cond(&world, PLAYER, Q10331), Some(1));

    world.objects.add_components(
        &PLAYER,
        crate::model::components::player::LastFolkNpc(sebion),
    );
    crate::game_loop::client::bypass::handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("Script LabyrinthOfBelis enter_instance"),
    );
    let id = world
        .instances
        .allowed_instance_of(PLAYER)
        .expect("entered");
    assert_eq!(world.instances.get(id).unwrap().template_id, BELIS);
    assert_eq!(in_world(&world, id, OFFICER).len(), 1);
}

/// Room 1: the officer follows, six Operatives come, and their deaths send
/// him on to room 2, where he stops and turns.
#[test]
fn room_one_clears_when_every_operative_falls() {
    let (mut world, mut rx, id, officer) = in_labyrinth();
    drain(&mut rx);
    first_talk(&mut world, officer);
    assert_eq!(
        any_html(&mut rx),
        dist_page(
            "data/scripts/instances/LabyrinthOfBelis/19155-01.html",
            officer
        )
    );
    event(&mut world, officer, "LabyrinthOfBelis", "room1");
    assert_eq!(status(&world, id), 1);
    assert!(world.objects.has_component::<NpcFollow>(&officer));
    let operatives = in_world(&world, id, OPERATIVE);
    assert_eq!(operatives.len(), 6);

    for &op in &operatives {
        game_loop::npc::npc_do_die(&mut world, op, PLAYER);
    }
    assert_eq!(status(&world, id), 2);
    advance_ticks(&mut world, 5);
    assert_eq!(status(&world, id), 3);
    assert!(!world.objects.has_component::<NpcFollow>(&officer));
    advance_world(&mut world, 300);
    let at = *world.objects.get_component::<Position>(&officer).unwrap();
    assert_eq!(
        (at.x, at.y),
        (-117040, 212502),
        "INFILTRATION_OFFICER_ROOM_2"
    );
    assert_eq!(script_value(&world, officer), 0, "ready to talk again");
}

/// Room 2: three Marks of Belis fed to the Verification System end it.
#[test]
fn three_marks_of_belis_open_room_three() {
    let (mut world, mut rx, id, _officer) = in_labyrinth();
    world.instances.set_status(id, 4);
    let verifier = in_world(&world, id, VERIFIER)[0];
    stand_by(&mut world, verifier);
    first_talk(&mut world, verifier);
    give_item(&mut world, PLAYER, 990_001, BELIS_MARK, 3);
    drain(&mut rx);
    for page in ["33215-01.html", "33215-02.html", "33215-03.html"] {
        event(&mut world, verifier, "LabyrinthOfBelis", "giveBelisMark");
        assert_eq!(
            served_html(&mut rx).unwrap_or_default(),
            dist_page(
                &format!("data/scripts/instances/LabyrinthOfBelis/{page}"),
                verifier
            )
        );
    }
    assert_eq!(status(&world, id), 5);
    assert_eq!(item_count(&world, PLAYER, BELIS_MARK), 0);
    event(&mut world, verifier, "LabyrinthOfBelis", "giveBelisMark");
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        dist_page(
            "data/scripts/instances/LabyrinthOfBelis/33215-05.html",
            verifier
        ),
        "only while the room is on"
    );
}

/// Room 3: the generator glows, six waves of attackers come for the
/// officer, and one of them falling once the sixth is out destroys it.
#[test]
fn room_three_ends_after_six_waves() {
    let (mut world, _rx, id, officer) = in_labyrinth();
    world.instances.set_status(id, 5);
    event(&mut world, officer, "LabyrinthOfBelis", "room3");
    assert_eq!(status(&world, id), 6);
    let generator = in_world(&world, id, GENERATOR)[0];
    advance_ticks(&mut world, 3);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&generator)
            .unwrap()
            .display_effect,
        1
    );
    advance_ticks(&mut world, 6 * 125);
    assert_eq!(world.instances.get_var(id, "counter"), 6);
    let attackers: Vec<i32> = [OPERATIVE, 22997]
        .iter()
        .flat_map(|&npc_id| in_world(&world, id, npc_id))
        .filter(|&oid| script_value(&world, oid) == 1)
        .collect();
    assert_eq!(attackers.len(), 6);
    advance_ticks(&mut world, 200);
    assert_eq!(world.instances.get_var(id, "counter"), 6, "no seventh wave");

    game_loop::npc::npc_do_die(&mut world, attackers[0], PLAYER);
    advance_ticks(&mut world, 20);
    assert_eq!(status(&world, id), 7);
    assert!(in_world(&world, id, GENERATOR).is_empty());
}

/// Room 4: the opening movie, Nemertess 50 s later, her death moves Q10331
/// to cond 2 with the ending movie, and 30 s on the officer walks to her spot.
#[test]
fn nemertess_falls_and_the_quest_moves_on() {
    let (mut world, _rx, id, officer) = in_labyrinth();
    world.instances.set_status(id, 7);
    event(&mut world, officer, "LabyrinthOfBelis", "room4");
    assert_eq!(status(&world, id), 8);
    assert_eq!(
        world
            .objects
            .get_component::<InMovie>(&PLAYER)
            .map(|m| m.movie_id),
        Some(43)
    );
    world.objects.remove_component::<InMovie>(&PLAYER);
    advance_ticks(&mut world, 499);
    assert!(in_world(&world, id, NEMERTESS).is_empty());
    advance_ticks(&mut world, 1);
    let nemertess = in_world(&world, id, NEMERTESS)[0];

    game_loop::npc::npc_do_die(&mut world, nemertess, PLAYER);
    assert_eq!(quest_cond(&world, PLAYER, Q10331), Some(2));
    assert_eq!(
        world
            .objects
            .get_component::<InMovie>(&PLAYER)
            .map(|m| m.movie_id),
        Some(44)
    );
    assert!(in_world(&world, id, NEMERTESS).is_empty(), "deleteMe");
    advance_ticks(&mut world, 300);
    assert_eq!(
        status(&world, id),
        9,
        "the timer on the deleted NPC still runs"
    );
    let ai = world.objects.get_component::<NpcAi>(&officer).unwrap();
    assert_eq!(ai.intention, crate::model::npc::NpcIntention::MoveTo);
}

/// The officer's death fails the instance.
#[test]
fn the_officers_death_fails_the_labyrinth() {
    let (mut world, _rx, id, officer) = in_labyrinth();
    let operative = in_world(&world, id, 22997)[0];
    game_loop::npc::npc_do_die(&mut world, officer, operative);
    assert_eq!(status(&world, id), -1);
}

/// `QuestLink` probes Sebion's `onTalk` before it talks for real, and the
/// probe must cost nothing. Java leaves `giveItems` unguarded under
/// simulation, so its probe would hand out a second necklace and move the
/// cond, and the real talk would then answer cond 3's page. The simulated
/// context here is inert: the `Quest` link behaves as a direct talk.
#[test]
fn the_quest_window_probe_does_not_run_the_talk_twice() {
    let (mut world, mut rx) = fate_world();
    let sebion = spawn_outside(&mut world, SEBION);
    talk(&mut world, sebion);
    event(&mut world, sebion, Q10331, "32978-03.htm");
    if let Some(q) = world
        .objects
        .get_component_mut::<crate::model::components::social::Quests>(&PLAYER)
        .and_then(|q| q.0.get_mut(Q10331))
    {
        q.vars.insert("cond".into(), "2".into());
    }
    drain(&mut rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{sebion}_Script")));
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        dist_page(
            "data/scripts/quests/Q10331_StartOfFate/32978-04.html",
            sebion
        )
    );
    assert_eq!(quest_cond(&world, PLAYER, Q10331), Some(3));
    assert_eq!(item_count(&world, PLAYER, SARIL_NECKLACE), 1);
}

/// Back at Sebion with cond 2: Saril's Necklace and the race's cond; the
/// race master's `classChange` turns a Fighter into a Warrior and pays out,
/// and a class off the Fighter's tree is refused as a cheat.
#[test]
fn franco_changes_a_human_fighter_into_a_warrior() {
    let (mut world, mut rx) = fate_world();
    let sebion = spawn_outside(&mut world, SEBION);
    talk(&mut world, sebion);
    event(&mut world, sebion, Q10331, "32978-03.htm");
    if let Some(q) = world
        .objects
        .get_component_mut::<crate::model::components::social::Quests>(&PLAYER)
        .and_then(|q| q.0.get_mut(Q10331))
    {
        q.vars.insert("cond".into(), "2".into());
    }
    talk(&mut world, sebion);
    assert_eq!(quest_cond(&world, PLAYER, Q10331), Some(3));
    assert_eq!(item_count(&world, PLAYER, SARIL_NECKLACE), 1);

    let franco = spawn_outside(&mut world, FRANCO);
    drain(&mut rx);
    talk(&mut world, franco);
    assert_eq!(
        any_html(&mut rx),
        dist_page(
            "data/scripts/quests/Q10331_StartOfFate/32153-07.html",
            franco
        )
    );
    event(&mut world, franco, Q10331, "classChange;11");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&PLAYER)
            .unwrap()
            .class_id,
        0
    );

    event(&mut world, franco, Q10331, "classChange;1");
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&PLAYER)
            .unwrap()
            .class_id,
        1
    );
    assert_eq!(
        quest_html(&mut rx),
        dist_page(
            "data/scripts/quests/Q10331_StartOfFate/32153-15.htm",
            franco
        )
    );
    assert_eq!(item_count(&world, PLAYER, SARIL_NECKLACE), 0);
    assert_eq!(item_count(&world, PLAYER, 17821), 40, "Proof of Courage");
    assert!(quest_completed(&world, PLAYER, Q10331));
}
