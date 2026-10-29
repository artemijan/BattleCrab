//! The Salvation second class transfer: Certification of Fate (10360), on
//! the real templates and htmls.

use super::*;
use crate::game_loop::quests;
use crate::model::components::social::Quests;
use crate::model::quest::{QuestState, state};
use crate::network::server_packets::opcodes::{TUTORIAL_SHOW_HTML, TUTORIAL_SHOW_QUESTION_MARK};

const PLAYER: i32 = 7302;
const Q10360: &str = "Q10360_CertificationOfFate";
const Q10331: &str = "Q10331_StartOfFate";

const RAINS: i32 = 30288;
const RAYMOND: i32 = 30289;
const RENFAD: i32 = 33524;
const JOEL: i32 = 33516;
const SCHUAZEN: i32 = 33517;
const SELON: i32 = 33518;
const KANILOV: i32 = 27459;
const POSLOF: i32 = 27460;
const SAKUM: i32 = 27453;

const SHINE_STONE: i32 = 17587;
const PAULINA_EQUIPMENT_SET: i32 = 46850;

const HUMAN: i32 = 0;
const WARRIOR: i32 = 1;
const GLADIATOR: i32 = 2;
const FIGHTER: i32 = 0;

const SPOT: (i32, i32, i32) = (-24795, 188754, -3960);

/// A level-40 Human Warrior in the Wastelands, Q10331 done unless `fresh`.
fn certification_world(start_of_fate_done: bool) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = quest_test_world();
    world.data = dist::game_data_owned();
    let rx = ingame_player(&mut world, 1, PLAYER, SPOT.0, SPOT.1, SPOT.2);
    set_level_race_class(&mut world, PLAYER, 40, HUMAN, WARRIOR);
    // The payout's exp recomputes the level from the total.
    let exp = world.data.experience.exp_for_level(40);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .exp = exp;
    if start_of_fate_done {
        world
            .objects
            .get_component_mut::<Quests>(&PLAYER)
            .unwrap()
            .0
            .insert(
                Q10331.to_string(),
                QuestState {
                    state: state::COMPLETED,
                    vars: Default::default(),
                },
            );
    }
    (world, rx)
}

fn spawn(world: &mut World, npc_id: i32) -> i32 {
    game_loop::npc::spawn_npc_at(world, npc_id, SPOT.0 + 50, SPOT.1, SPOT.2, 0).expect("spawned")
}

fn event(world: &mut World, npc: i32, event: &str) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {Q10360} {event}")),
    );
}

fn talk(world: &mut World, npc: i32) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {Q10360}")),
    );
}

fn first_talk(world: &mut World, npc: i32) {
    let npc_id = game_loop::npc::npc_id_of(world, npc).unwrap();
    world
        .objects
        .add_components(&PLAYER, crate::model::components::player::LastFolkNpc(npc));
    quests::notify_first_talk(world, 1, PLAYER, npc, npc_id);
}

fn page(file: &str, npc: i32) -> String {
    dist_page(
        &format!("data/scripts/quests/Q10360_CertificationOfFate/{file}"),
        npc,
    )
}

fn class_id(world: &World) -> i32 {
    world
        .objects
        .get_component::<Player>(&PLAYER)
        .unwrap()
        .class_id
}

fn has_usm(pkts: &[Vec<u8>], video_id: i32) -> bool {
    pkts.iter().any(|p| {
        p[0] == server_packets::opcodes::EX
            && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_SHOW_USM
            && i32::from_le_bytes([p[3], p[4], p[5], p[6]]) == video_id
    })
}

/// Login from 38 and the level-up onto exactly 38 show mark 10360 to a first
/// class; its click opens the Human fighter popup.
#[test]
fn the_second_class_mark_shows_from_level_38() {
    let (mut world, mut rx) = certification_world(true);
    set_level(&mut world, PLAYER, 37);
    drain(&mut rx);
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));

    set_level(&mut world, PLAYER, 38);
    quests::dispatch::notify_level_changed(&mut world, PLAYER, 37, 38);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
    quests::dispatch::notify_level_changed(&mut world, PLAYER, 38, 39);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));

    quests::notify_tutorial_mark(&mut world, 1, PLAYER, 10360);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_HTML));

    // A base class is not in SECOND_CLASS_GROUP.
    set_level_class(&mut world, PLAYER, 38, FIGHTER);
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
}

/// Without Start of Fate the master answers `restriction.html`; with it, the
/// fighter master offers the quest and the mystic master turns a fighter away.
#[test]
fn the_masters_gate_on_start_of_fate_and_the_players_side() {
    let (mut world, mut rx) = certification_world(false);
    let rains = spawn(&mut world, RAINS);
    drain(&mut rx);
    talk(&mut world, rains);
    assert_eq!(any_html(&mut rx), page("restriction.html", rains));

    let (mut world, mut rx) = certification_world(true);
    let rains = spawn(&mut world, RAINS);
    let raymond = spawn(&mut world, RAYMOND);
    drain(&mut rx);
    talk(&mut world, rains);
    assert_eq!(any_html(&mut rx), page("30288-02.htm", rains));
    talk(&mut world, raymond);
    assert_eq!(any_html(&mut rx), page("30289-01.html", raymond));
}

/// The whole chain on a Human Warrior: Rains, Renfad, Kanilov, Joel, Poslof,
/// Schuazen (and the transfer video), Sakum's Shine Stone, Selon's race
/// cond, then Rains' class change into a Gladiator with its payout.
#[test]
fn a_human_warrior_becomes_a_gladiator() {
    let (mut world, mut rx) = certification_world(true);
    let rains = spawn(&mut world, RAINS);
    talk(&mut world, rains);
    event(&mut world, rains, "30288-04.htm");
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(1));

    let renfad = spawn(&mut world, RENFAD);
    drain(&mut rx);
    talk(&mut world, renfad);
    assert_eq!(any_html(&mut rx), page("33524-01.htm", renfad));
    event(&mut world, renfad, "33524-03.htm");
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(2));

    let kanilov = spawn(&mut world, KANILOV);
    game_loop::npc::npc_do_die(&mut world, kanilov, PLAYER);
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(3));

    let joel = spawn(&mut world, JOEL);
    event(&mut world, joel, "33516-03.htm");
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(4));
    let poslof = spawn(&mut world, POSLOF);
    game_loop::npc::npc_do_die(&mut world, poslof, PLAYER);
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(5));

    let schuazen = spawn(&mut world, SCHUAZEN);
    drain(&mut rx);
    event(&mut world, schuazen, "33517-03.htm");
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(6));
    assert!(has_usm(&drain(&mut rx), 4), "SECOND_TRANSFER_QUEST");

    let sakum = spawn(&mut world, SAKUM);
    game_loop::npc::npc_do_die(&mut world, sakum, PLAYER);
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(7));
    assert_eq!(item_count(&world, PLAYER, SHINE_STONE), 1);

    let selon = spawn(&mut world, SELON);
    drain(&mut rx);
    event(&mut world, selon, "33518-06.htm");
    assert_eq!(quest_cond(&world, PLAYER, Q10360), Some(8));
    assert_eq!(quest_html(&mut rx), page("33518-07.htm", selon));

    talk(&mut world, rains);
    assert_eq!(any_html(&mut rx), page("class_select_warrior.html", rains));

    // Back to the Fighter the Warrior came from: a cheat.
    event(&mut world, rains, &format!("classChange;{FIGHTER}"));
    assert_eq!(class_id(&world), WARRIOR);
    assert!(quest_started(&world, PLAYER, Q10360));

    drain(&mut rx);
    event(&mut world, rains, &format!("classChange;{GLADIATOR}"));
    assert_eq!(class_id(&world), GLADIATOR);
    // The payout's inventory updates lead with EX packets, which
    // `any_html` would misread as a quest window.
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        page("transfer_complete_gladiator.html", rains)
    );
    assert!(quest_completed(&world, PLAYER, Q10360));
    assert_eq!(item_count(&world, PLAYER, SHINE_STONE), 0);
    assert_eq!(item_count(&world, PLAYER, PAULINA_EQUIPMENT_SET), 1);

    talk(&mut world, rains);
    assert_eq!(any_html(&mut rx), page("30288-07.html", rains));
}

/// Joel's first talk: his quest page before his step, even with no quest
/// state at all, and the NPC's own window once the quest is done.
#[test]
fn joels_first_talk_shows_his_page_until_the_quest_is_done() {
    let (mut world, mut rx) = certification_world(true);
    let joel = spawn(&mut world, JOEL);
    drain(&mut rx);
    first_talk(&mut world, joel);
    assert_eq!(any_html(&mut rx), page("33516.html", joel));

    world
        .objects
        .get_component_mut::<Quests>(&PLAYER)
        .unwrap()
        .0
        .insert(
            Q10360.to_string(),
            QuestState {
                state: state::COMPLETED,
                vars: Default::default(),
            },
        );
    first_talk(&mut world, joel);
    assert_ne!(any_html(&mut rx), page("33516.html", joel));
}
