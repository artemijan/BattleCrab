//! The Salvation Ertheia class transfers: Winds of Fate (10751–10753), the
//! Mysterious Wizard and the instances Fortress of the Dead (254) and
//! Chamber of Prophecies (255), on the real templates and htmls.

use super::*;
use crate::game_loop::quests;
use crate::model::components::social::Quests;
use crate::model::quest::{QuestState, state};
use crate::network::server_packets::opcodes::{TUTORIAL_SHOW_HTML, TUTORIAL_SHOW_QUESTION_MARK};

const PLAYER: i32 = 7305;
const Q10751: &str = "Q10751_WindsOfFateEncounters";
const Q10752: &str = "Q10752_WindsOfFateAPromise";
const Q10753: &str = "Q10753_WindsOfFateChoices";
const FORTRESS: &str = "FortressOfTheDead";
const CHAMBER: &str = "ChamberOfProphecies";
const WIZARD_AI: &str = "MysteriousWizard";

const NAVARI: i32 = 33931;
const KATALIN: i32 = 33943;
const RAYMOND: i32 = 30289;
const KARLA: i32 = 33933;
const TELESHA: i32 = 33981;
const MYSTERIOUS_WIZARD: i32 = 33980;
const SKELETON_WARRIOR: i32 = 27528;
const BROKEN_BOOKSHELF: i32 = 31526;
const VAMPIRIC_SOLDIER: i32 = 19567;
const VON_HELLMAN: i32 = 19566;
const KAIN_FORTRESS: i32 = 33979;
const ATHREA: i32 = 30758;
const ATHREAS_BOX: i32 = 33997;
const GERETH: i32 = 33932;
const KAIN_CHAMBER: i32 = 31639;
const HELPER_VAN_HALTER: i32 = 33999;
const GRAIL: i32 = 33996;
const NEBULITE_EYE: i32 = 27544;
const NEBULITE_WATCH: i32 = 27545;
const NEBULITE_GOLEM: i32 = 27546;

const WIND_SPIRIT_REALMS_RELIC: i32 = 39535;
const PAULINA_EQUIPMENT_SET: i32 = 46850;
const KAIN_PROPHECY_MACHINE_FRAGMENT: i32 = 39538;
const PROPHECY_MACHINE: i32 = 39540;
const ATELIA: i32 = 39542;
const WHITE_ROSE: i32 = 39551;
const ATHREAS_BELONGINGS: i32 = 39550;
const SAYHAS_BOX_M: i32 = 40269;

const ERTHEIA: i32 = 6;
const ERTHEIA_FIGHTER: i32 = 182;
const MARAUDER: i32 = 184;
const RIPPER: i32 = 186;
const STRATOMANCER: i32 = 187;
const SAYHA_SEER: i32 = 189;

const FORTRESS_TEMPLATE: i32 = 254;
const CHAMBER_TEMPLATE: i32 = 255;
const DOOR_2: i32 = 17230102;
const DOOR_4: i32 = 17230104;

const SPOT: (i32, i32, i32) = (-80_565, 251_763, -3_080);

/// An Ertheia of `level` and `class_id` in Faeron.
fn ertheia_world(level: i32, class_id: i32) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = quest_test_world();
    world.data = dist::game_data_owned();
    // The dist's `MaximumPlayerLevel = 110`, incremented; the config
    // default (81) would cap the instances' level conditions.
    world.cfg.character.maximum_player_level = 111;
    let rx = ingame_player(&mut world, 1, PLAYER, SPOT.0, SPOT.1, SPOT.2);
    set_level_race_class(&mut world, PLAYER, level, ERTHEIA, class_id);
    let exp = world.data.experience.exp_for_level(level);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .exp = exp;
    (world, rx)
}

/// Put `quest` on `cond` (STARTED), or COMPLETED for `None`.
fn set_quest(world: &mut World, quest: &str, cond: Option<i32>) {
    let mut vars = std::collections::HashMap::new();
    let state = match cond {
        Some(c) => {
            vars.insert("cond".to_string(), c.to_string());
            state::STARTED
        }
        None => state::COMPLETED,
    };
    world
        .objects
        .get_component_mut::<Quests>(&PLAYER)
        .unwrap()
        .0
        .insert(quest.to_string(), QuestState { state, vars });
}

fn spawn(world: &mut World, npc_id: i32) -> i32 {
    let p = *world.objects.get_component::<Position>(&PLAYER).unwrap();
    game_loop::npc::spawn_npc_at(world, npc_id, p.x + 50, p.y, p.z, 0).expect("spawned")
}

fn stand_by(world: &mut World, npc: i32) {
    let p = *world.objects.get_component::<Position>(&npc).unwrap();
    if let Some(me) = world.objects.get_component_mut::<Position>(&PLAYER) {
        (me.x, me.y, me.z) = (p.x + 50, p.y, p.z);
    }
}

fn event(world: &mut World, npc: i32, script: &str, event: &str) {
    stand_by(world, npc);
    world
        .objects
        .add_components(&PLAYER, crate::model::components::player::LastFolkNpc(npc));
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {script} {event}")),
    );
}

fn talk(world: &mut World, npc: i32, quest: &str) {
    stand_by(world, npc);
    handle_request_bypass_to_server(world, 1, &bypass_body(&format!("npc_{npc}_Script {quest}")));
}

fn first_talk(world: &mut World, npc: i32) {
    stand_by(world, npc);
    let npc_id = game_loop::npc::npc_id_of(world, npc).unwrap();
    world
        .objects
        .add_components(&PLAYER, crate::model::components::player::LastFolkNpc(npc));
    quests::notify_first_talk(world, 1, PLAYER, npc, npc_id);
}

/// The first `NpcHtmlMessage` waiting on `rx`, empty when none was sent.
fn html(rx: &mut UnboundedReceiver<bytes::Bytes>) -> String {
    served_html(rx).unwrap_or_default()
}

fn page(dir: &str, file: &str, npc: i32) -> String {
    dist_page(&format!("data/scripts/{dir}/{file}"), npc)
}

fn class_id(world: &World) -> i32 {
    world
        .objects
        .get_component::<Player>(&PLAYER)
        .unwrap()
        .class_id
}

fn position(world: &World) -> (i32, i32) {
    let p = world.objects.get_component::<Position>(&PLAYER).unwrap();
    (p.x, p.y)
}

/// The instance's living NPCs with `npc_id`.
fn alive(world: &World, id: i32, npc_id: i32) -> Vec<i32> {
    world
        .instances
        .get(id)
        .unwrap()
        .npcs
        .iter()
        .copied()
        .filter(|oid| game_loop::npc::npc_id_of(world, *oid) == Some(npc_id))
        .filter(|&oid| !game_loop::helpers::is_dead(world, oid))
        .collect()
}

/// Kill every living monster of the instance.
fn clear_monsters(world: &mut World, id: i32) {
    let monsters: Vec<i32> = world
        .instances
        .get(id)
        .unwrap()
        .npcs
        .clone()
        .into_iter()
        .filter(|&oid| {
            game_loop::npc::npc_template(world, oid).is_some_and(|t| t.is_monster())
                && !game_loop::helpers::is_dead(world, oid)
        })
        .collect();
    assert!(
        !monsters.is_empty(),
        "a room to clear at status {}",
        world.instances.status(id)
    );
    for npc in monsters {
        game_loop::npc::npc_do_die(world, npc, PLAYER);
    }
}

fn door_open(world: &World, id: i32, door_id: i32) -> bool {
    world.instances.get(id).unwrap().doors.iter().any(|oid| {
        world
            .objects
            .get_component::<crate::model::door::Door>(oid)
            .is_some_and(|d| d.door_id == door_id)
            && world
                .objects
                .get_component::<crate::model::components::space::InstanceDoorOpen>(oid)
                .is_some_and(|o| o.0)
    })
}

fn entered(world: &World, template: i32) -> i32 {
    let id = world
        .instances
        .allowed_instance_of(PLAYER)
        .expect("entered");
    assert_eq!(world.instances.get(id).unwrap().template_id, template);
    id
}

fn npc_log_counts(pkt: &[u8]) -> Vec<(i32, i32)> {
    let n = pkt[7] as usize;
    (0..n)
        .map(|i| {
            let at = 8 + i * 9;
            (
                i32::from_le_bytes(pkt[at..at + 4].try_into().unwrap()),
                i32::from_le_bytes(pkt[at + 5..at + 9].try_into().unwrap()),
            )
        })
        .collect()
}

/// Login from 38 shows Navari's letter; the mark opens the popup, whose
/// `Q10751_teleport` refuses a transformed player and then teleports.
#[test]
fn navaris_letter_shows_from_38_and_its_popup_teleports() {
    let (mut world, mut rx) = ertheia_world(37, ERTHEIA_FIGHTER);
    drain(&mut rx);
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
    set_level(&mut world, PLAYER, 38);
    quests::dispatch::notify_level_changed(&mut world, PLAYER, 37, 38);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));

    quests::notify_tutorial_mark(&mut world, 1, PLAYER, 10751);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_HTML));

    if let Some(me) = world.objects.get_component_mut::<Position>(&PLAYER) {
        (me.x, me.y) = (0, 0);
    }
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .transform_id = 1;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Q10751_teleport"));
    assert_eq!(position(&world), (0, 0), "transformed: refused");
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .transform_id = 0;
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Q10751_teleport"));
    assert_eq!(position(&world), (-80_565, 251_763));
}

/// The whole of Q10751 on a fighter: Katalin, Raymond, five skeletons with
/// the kill log, Telesha and the wizard, then the Marauder transfer.
#[test]
fn an_ertheia_fighter_becomes_a_marauder() {
    let (mut world, mut rx) = ertheia_world(40, ERTHEIA_FIGHTER);
    let navari = spawn(&mut world, NAVARI);
    talk(&mut world, navari, Q10751);
    drain(&mut rx);
    event(&mut world, navari, Q10751, "33931-02.htm");
    assert_eq!(quest_cond(&world, PLAYER, Q10751), Some(2));
    assert_eq!(
        quest_html(&mut rx),
        page(
            "quests/Q10751_WindsOfFateEncounters",
            "33931-03.htm",
            navari
        )
    );

    let katalin = spawn(&mut world, KATALIN);
    event(&mut world, katalin, Q10751, "33943-02.html");
    assert_eq!(quest_cond(&world, PLAYER, Q10751), Some(4));
    let raymond = spawn(&mut world, RAYMOND);
    event(&mut world, raymond, Q10751, "30289-03.html");
    assert_eq!(quest_cond(&world, PLAYER, Q10751), Some(6));
    assert_eq!(item_count(&world, PLAYER, WIND_SPIRIT_REALMS_RELIC), 1);

    drain(&mut rx);
    for kill in 1..=5 {
        let skeleton = spawn(&mut world, SKELETON_WARRIOR);
        game_loop::npc::npc_do_die(&mut world, skeleton, PLAYER);
        let log = find_ex_opcode(&mut rx, server_packets::opcodes::EX_QUEST_NPC_LOG_LIST)
            .expect("ExQuestNpcLogList");
        assert_eq!(npc_log_counts(&log), vec![(575113, kill)]);
        assert_eq!(npcs_of(&mut world, TELESHA).len(), usize::from(kill == 5));
    }
    let telesha = npcs_of(&mut world, TELESHA)[0];
    // A sixth kill beside her summons no second Telesha.
    let skeleton = spawn(&mut world, SKELETON_WARRIOR);
    game_loop::npc::npc_do_die(&mut world, skeleton, PLAYER);
    assert_eq!(npcs_of(&mut world, TELESHA).len(), 1);

    first_talk(&mut world, telesha);
    assert_eq!(
        html(&mut rx),
        page(
            "quests/Q10751_WindsOfFateEncounters",
            "33981-01.html",
            telesha
        )
    );
    event(&mut world, telesha, Q10751, "SPAWN_WIZZARD");
    assert!(npcs_of(&mut world, TELESHA).is_empty());
    let wizard = npcs_of(&mut world, MYSTERIOUS_WIZARD)[0];
    drain(&mut rx);
    first_talk(&mut world, wizard);
    assert_eq!(
        html(&mut rx),
        page("ai/others/MysteriousWizard", "33980-05.html", wizard)
    );
    event(&mut world, wizard, Q10751, "33980-06.html");
    assert_eq!(quest_cond(&world, PLAYER, Q10751), Some(7));
    assert_eq!(item_count(&world, PLAYER, WIND_SPIRIT_REALMS_RELIC), 2);

    event(&mut world, raymond, Q10751, "30289-07.html");
    assert_eq!(quest_cond(&world, PLAYER, Q10751), Some(8));
    event(&mut world, katalin, Q10751, "33943-11.html");
    assert_eq!(class_id(&world), MARAUDER);
    assert!(quest_completed(&world, PLAYER, Q10751));
    assert_eq!(item_count(&world, PLAYER, WIND_SPIRIT_REALMS_RELIC), 0);
    assert_eq!(item_count(&world, PLAYER, PAULINA_EQUIPMENT_SET), 1);
}

/// Q10752's starters: Katalin needs level 76 and Q10751 done, and turns a
/// mage away.
#[test]
fn katalin_offers_q10752_to_a_level_76_marauder() {
    let (mut world, mut rx) = ertheia_world(75, MARAUDER);
    set_quest(&mut world, Q10751, None);
    let katalin = spawn(&mut world, KATALIN);
    let dir = "quests/Q10752_WindsOfFateAPromise";
    drain(&mut rx);
    talk(&mut world, katalin, Q10752);
    assert_eq!(html(&mut rx), page(dir, "33943-12.html", katalin));
    set_level(&mut world, PLAYER, 76);
    talk(&mut world, katalin, Q10752);
    assert_eq!(quest_html(&mut rx), page(dir, "33943-01.htm", katalin));
    event(&mut world, katalin, Q10752, "33943-05.html");
    assert_eq!(quest_cond(&world, PLAYER, Q10752), Some(1));
}

/// Fortress of the Dead: the vampires, Von Hellman, Kain and the wizard;
/// `endCinematic` sets Q10752 cond 9 and the instance closes 25 s later.
/// Karla's `checkClass` then leads to the Ripper.
#[test]
fn the_fortress_of_the_dead_brings_q10752_to_the_ripper() {
    let (mut world, mut rx) = ertheia_world(76, MARAUDER);
    set_quest(&mut world, Q10752, Some(8));
    let bookshelf = spawn(&mut world, BROKEN_BOOKSHELF);
    event(&mut world, bookshelf, FORTRESS, "enterInstance");
    let id = entered(&world, FORTRESS_TEMPLATE);

    for soldier in alive(&world, id, VAMPIRIC_SOLDIER) {
        game_loop::npc::npc_do_die(&mut world, soldier, PLAYER);
    }
    advance_ticks(&mut world, 3);
    let hellman = alive(&world, id, VON_HELLMAN)[0];
    drain(&mut rx);
    game_loop::npc::npc_do_die(&mut world, hellman, PLAYER);
    assert!(alive(&world, id, VON_HELLMAN).is_empty(), "deleted");
    advance_ticks(&mut world, 3);
    let kain = alive(&world, id, KAIN_FORTRESS)[0];

    first_talk(&mut world, kain);
    assert_eq!(
        html(&mut rx),
        page("instances/FortressOfTheDead", "33979.html", kain)
    );
    event(&mut world, kain, FORTRESS, "spawnWizard");
    let wizard = alive(&world, id, MYSTERIOUS_WIZARD)[0];
    drain(&mut rx);
    first_talk(&mut world, wizard);
    assert_eq!(
        html(&mut rx),
        page("ai/others/MysteriousWizard", "33980.html", wizard)
    );
    event(&mut world, wizard, WIZARD_AI, "33980-01.html");
    event(&mut world, wizard, FORTRESS, "endCinematic");
    assert_eq!(quest_cond(&world, PLAYER, Q10752), Some(9));
    assert_eq!(
        item_count(&world, PLAYER, KAIN_PROPHECY_MACHINE_FRAGMENT),
        1
    );
    assert!(alive(&world, id, KAIN_FORTRESS).is_empty());
    assert!(alive(&world, id, MYSTERIOUS_WIZARD).is_empty());
    advance_ticks(&mut world, 251);
    assert_eq!(game_loop::helpers::instance_of(&world, PLAYER), 0);

    let karla = spawn(&mut world, KARLA);
    event(&mut world, karla, Q10752, "checkClass");
    assert_eq!(quest_cond(&world, PLAYER, Q10752), Some(11));
    assert_eq!(
        item_count(&world, PLAYER, KAIN_PROPHECY_MACHINE_FRAGMENT),
        0
    );
    let katalin = spawn(&mut world, KATALIN);
    event(&mut world, katalin, Q10752, "33943-10.html");
    assert_eq!(class_id(&world), RIPPER);
    assert!(quest_completed(&world, PLAYER, Q10752));
}

/// The Nebulite drops: a winning roll counts each kind up to three, the
/// kill log lists the non-zero counters, and nine drops reach cond 3.
#[test]
fn nebulite_drops_fill_the_kill_log_and_reach_cond_3() {
    let (mut world, mut rx) = ertheia_world(85, RIPPER);
    set_quest(&mut world, Q10753, Some(2));
    drain(&mut rx);
    world.force_roll(99);
    let eye = spawn(&mut world, NEBULITE_EYE);
    game_loop::npc::npc_do_die(&mut world, eye, PLAYER);
    world.clear_forced_rolls();
    let log = find_ex_opcode(&mut rx, server_packets::opcodes::EX_QUEST_NPC_LOG_LIST)
        .expect("ExQuestNpcLogList");
    assert_eq!(npc_log_counts(&log), vec![(575311, 1)]);

    for npc_id in [
        NEBULITE_EYE,
        NEBULITE_EYE,
        NEBULITE_WATCH,
        NEBULITE_WATCH,
        NEBULITE_WATCH,
    ] {
        world.force_roll(99);
        let npc = spawn(&mut world, npc_id);
        game_loop::npc::npc_do_die(&mut world, npc, PLAYER);
        world.clear_forced_rolls();
    }
    // A losing roll drops nothing.
    world.force_roll(50);
    let golem = spawn(&mut world, NEBULITE_GOLEM);
    game_loop::npc::npc_do_die(&mut world, golem, PLAYER);
    world.clear_forced_rolls();
    assert_eq!(quest_var_int(&world, PLAYER, Q10753, "FlaskCount"), 0);
    for _ in 0..3 {
        world.force_roll(99);
        let npc = spawn(&mut world, NEBULITE_GOLEM);
        game_loop::npc::npc_do_die(&mut world, npc, PLAYER);
        world.clear_forced_rolls();
    }
    assert_eq!(quest_cond(&world, PLAYER, Q10753), Some(3));
}

/// Athrea's hunt: sixteen boxes for 180 s, four belongings reach cond 10,
/// and her next talk clears the boxes.
#[test]
fn athreas_boxes_turn_up_her_belongings() {
    let (mut world, mut rx) = ertheia_world(85, RIPPER);
    // Athrea stands among her boxes.
    if let Some(me) = world.objects.get_component_mut::<Position>(&PLAYER) {
        (me.x, me.y, me.z) = (101_979, 103_310, -3_506);
    }
    set_quest(&mut world, Q10753, Some(8));
    give_item(&mut world, PLAYER, 9_000_001, WHITE_ROSE, 1);
    let athrea = spawn(&mut world, ATHREA);
    event(&mut world, athrea, Q10753, "30758-04.html");
    assert_eq!(quest_cond(&world, PLAYER, Q10753), Some(9));
    let boxes = npcs_of(&mut world, ATHREAS_BOX);
    assert_eq!(boxes.len(), 16);
    // She is busy until the hunt's timer runs out.
    drain(&mut rx);
    event(&mut world, athrea, Q10753, "30758-02.html");
    assert_eq!(
        html(&mut rx),
        page("quests/Q10753_WindsOfFateChoices", "busy.html", athrea)
    );

    first_talk(&mut world, boxes[0]);
    assert_eq!(
        html(&mut rx),
        page("quests/Q10753_WindsOfFateChoices", "33997.html", boxes[0])
    );
    world.force_roll(99);
    event(&mut world, boxes[0], Q10753, "open");
    world.clear_forced_rolls();
    assert_eq!(item_count(&world, PLAYER, ATHREAS_BELONGINGS), 0);
    drain(&mut rx);
    first_talk(&mut world, boxes[0]);
    assert_eq!(
        html(&mut rx),
        page("quests/Q10753_WindsOfFateChoices", "33997-1.html", boxes[0]),
        "an opened box"
    );
    for &b in &boxes[1..5] {
        world.force_roll(0);
        event(&mut world, b, Q10753, "open");
        world.clear_forced_rolls();
    }
    assert_eq!(item_count(&world, PLAYER, ATHREAS_BELONGINGS), 4);
    assert_eq!(quest_cond(&world, PLAYER, Q10753), Some(10));

    talk(&mut world, athrea, Q10753);
    assert!(npcs_of(&mut world, ATHREAS_BOX).is_empty());
    event(&mut world, athrea, Q10753, "30758-09.html");
    assert_eq!(quest_cond(&world, PLAYER, Q10753), Some(11));
}

/// The Chamber of Prophecies end to end: Gereth's entry (cond 16), Kain's
/// teleport, the four rooms on the 7 s status loop, the Grail's Atelia, the
/// wizard, and Kain's exit (cond 17).
#[test]
fn the_chamber_of_prophecies_runs_its_rooms() {
    let (mut world, mut rx) = ertheia_world(85, STRATOMANCER);
    set_quest(&mut world, Q10753, Some(15));
    give_item(&mut world, PLAYER, 9_000_001, PROPHECY_MACHINE, 1);
    let gereth = spawn(&mut world, GERETH);
    event(&mut world, gereth, CHAMBER, "enterInstance");
    let id = entered(&world, CHAMBER_TEMPLATE);
    assert_eq!(quest_cond(&world, PLAYER, Q10753), Some(16));
    assert_eq!(item_count(&world, PLAYER, PROPHECY_MACHINE), 0);

    let kain = alive(&world, id, KAIN_CHAMBER)[0];
    drain(&mut rx);
    first_talk(&mut world, kain);
    assert_eq!(
        html(&mut rx),
        page("instances/ChamberOfProphecies", "31639.html", kain)
    );
    event(&mut world, kain, CHAMBER, "teleport");
    assert_eq!(position(&world), (-88_503, 184_754));
    assert_eq!(alive(&world, id, HELPER_VAN_HALTER).len(), 1);

    // Room 1 cleared: the next status beat opens door 2 five seconds on.
    clear_monsters(&mut world, id);
    advance_ticks(&mut world, 71 + 51);
    assert_eq!(world.instances.status(id), 1);
    assert!(door_open(&world, id, DOOR_2));
    for status in [2, 3, 4, 5] {
        clear_monsters(&mut world, id);
        advance_ticks(&mut world, 71 + 51);
        assert_eq!(world.instances.status(id), status);
    }
    // Room 4 holds only Makkum (a Folk); its banter ends in SEY_KAIN_4
    // (3 + 7 + 1 s after the last beat): the Grail and door 4.
    advance_ticks(&mut world, 2);
    let grail = alive(&world, id, GRAIL)[0];
    assert!(door_open(&world, id, DOOR_4));

    event(&mut world, grail, CHAMBER, "33996-02.html");
    assert_eq!(item_count(&world, PLAYER, ATELIA), 1);
    assert!(alive(&world, id, GRAIL).is_empty());
    let wizard = alive(&world, id, MYSTERIOUS_WIZARD)[0];
    drain(&mut rx);
    first_talk(&mut world, wizard);
    assert_eq!(
        html(&mut rx),
        page("instances/ChamberOfProphecies", "33980.html", wizard)
    );
    event(&mut world, wizard, CHAMBER, "33980-05.html");
    assert_eq!(world.instances.status(id), 6);
    advance_ticks(&mut world, 21);
    assert!(alive(&world, id, MYSTERIOUS_WIZARD).is_empty());

    let kain = alive(&world, id, KAIN_CHAMBER)[0];
    drain(&mut rx);
    event(&mut world, kain, CHAMBER, "exit");
    assert!(has_sm(
        &drain(&mut rx),
        server_packets::sm_ids::THIS_DUNGEON_WILL_EXPIRE_IN_S1_MIN_YOU_WILL_BE_FORCED_OUT_OF_THE_DUNGEON_WHEN_THE_TIME_EXPIRES
    ));
    assert_eq!(quest_cond(&world, PLAYER, Q10753), Some(17));
    advance_ticks(&mut world, 31);
    assert_eq!(game_loop::helpers::instance_of(&world, PLAYER), 0);
}

/// Navari's choice: a Stratomancer on cond 18 with the Atelia becomes a
/// Sayha's Seer.
#[test]
fn navari_makes_a_sayhas_seer() {
    let (mut world, mut rx) = ertheia_world(85, STRATOMANCER);
    set_quest(&mut world, Q10753, Some(18));
    give_item(&mut world, PLAYER, 9_000_001, ATELIA, 1);
    let navari = spawn(&mut world, NAVARI);
    drain(&mut rx);
    talk(&mut world, navari, Q10753);
    assert_eq!(
        html(&mut rx),
        page("quests/Q10753_WindsOfFateChoices", "33931-01.html", navari)
    );
    event(&mut world, navari, Q10753, "33931-04a.html");
    assert_eq!(class_id(&world), SAYHA_SEER);
    assert!(quest_completed(&world, PLAYER, Q10753));
    assert_eq!(item_count(&world, PLAYER, SAYHAS_BOX_M), 1);
    assert_eq!(item_count(&world, PLAYER, ATELIA), 0);
}
