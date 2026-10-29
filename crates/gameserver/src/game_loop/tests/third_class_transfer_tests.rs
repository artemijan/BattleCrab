//! The Salvation third class transfer: Day of Destiny (10341–10346) and its
//! instance, the Evil Incubator (185), on the real templates and htmls.

use super::*;
use crate::game_loop::quests;
use crate::model::components::social::Quests;
use crate::network::server_packets::opcodes::{TUTORIAL_SHOW_HTML, TUTORIAL_SHOW_QUESTION_MARK};

const PLAYER: i32 = 7303;
const Q10341: &str = "Q10341_DayOfDestinyHumansFate";
const INCUBATOR: i32 = 185;

const ORVEN: i32 = 30857;
const QUARTERMASTER: i32 = 33407;
const VANGUARD_MEMBER: i32 = 33165;
const VANGUARDS: [i32; 4] = [33166, 33167, 33168, 33169];
const ADOLPH: i32 = 33170;
const ALICE: i32 = 33171;
const BARTON: i32 = 33172;
const HAYUK: i32 = 33173;
const ELIYAH: i32 = 33174;
const MONSTERS: [i32; 5] = [27430, 27431, 27432, 27433, 27434];
const GLADIATOR_BOSS: i32 = 27426;

const HUMAN_TAG: i32 = 17748;
const GLADIATOR_PENDANT: i32 = 17484;
const SOE: i32 = 736;

const HUMAN: i32 = 0;
const WARRIOR: i32 = 1;
const GLADIATOR: i32 = 2;
const DUELIST: i32 = 88;

const SPOT: (i32, i32, i32) = (147_450, 26_740, -2_200);

/// A level-76 Human Gladiator.
fn destiny_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = quest_test_world();
    world.data = dist::game_data_owned();
    let rx = ingame_player(&mut world, 1, PLAYER, SPOT.0, SPOT.1, SPOT.2);
    set_level_race_class(&mut world, PLAYER, 76, HUMAN, GLADIATOR);
    // The payout's exp recomputes the level from the total.
    let exp = world.data.experience.exp_for_level(76);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .exp = exp;
    (world, rx)
}

fn spawn(world: &mut World, npc_id: i32) -> i32 {
    game_loop::npc::spawn_npc_at(world, npc_id, SPOT.0 + 50, SPOT.1, SPOT.2, 0).expect("spawned")
}

fn stand_by(world: &mut World, npc: i32) {
    let p = *world.objects.get_component::<Position>(&npc).unwrap();
    if let Some(me) = world.objects.get_component_mut::<Position>(&PLAYER) {
        (me.x, me.y, me.z) = (p.x + 50, p.y, p.z);
    }
}

fn event(world: &mut World, npc: i32, script: &str, event: &str) {
    stand_by(world, npc);
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {script} {event}")),
    );
}

fn talk(world: &mut World, npc: i32, script: &str) {
    stand_by(world, npc);
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {script}")),
    );
}

fn page(file: &str, npc: i32) -> String {
    dist_page(
        &format!("data/scripts/quests/Q10341_DayOfDestinyHumansFate/{file}"),
        npc,
    )
}

fn instance_page(file: &str, npc: i32) -> String {
    dist_page(&format!("data/scripts/instances/EvilIncubator/{file}"), npc)
}

fn cond(world: &World) -> Option<i32> {
    quest_cond(world, PLAYER, Q10341)
}

fn class_id(world: &World) -> i32 {
    world
        .objects
        .get_component::<Player>(&PLAYER)
        .unwrap()
        .class_id
}

/// The instance's NPCs with `npc_id` still in the world.
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

fn alive_monsters(world: &World, id: i32) -> Vec<i32> {
    MONSTERS
        .iter()
        .flat_map(|&m| in_world(world, id, m))
        .filter(|&oid| !game_loop::helpers::is_dead(world, oid))
        .collect()
}

fn wave(world: &World, id: i32) -> i64 {
    world.instances.get_var(id, "WORLD_WAVE")
}

/// A started Q10341 at cond 4: the tags collected and handed in.
fn ready_for_the_incubator() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, rx) = destiny_world();
    let orven = spawn(&mut world, ORVEN);
    talk(&mut world, orven, Q10341);
    event(&mut world, orven, Q10341, "30857-05.htm");
    let qm = spawn(&mut world, QUARTERMASTER);
    event(&mut world, qm, Q10341, "33407-02.html");
    for id in VANGUARDS {
        let v = spawn(&mut world, id);
        event(&mut world, v, Q10341, "collectTag");
    }
    event(&mut world, qm, Q10341, "33407-05.html");
    assert_eq!(cond(&world), Some(4));
    (world, rx)
}

/// Login and the level-up onto exactly 76 show mark 101 to a second class
/// of the quest's race; its click opens the invitation.
#[test]
fn mark_101_shows_from_level_76() {
    let (mut world, mut rx) = destiny_world();
    set_level(&mut world, PLAYER, 75);
    drain(&mut rx);
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));

    set_level(&mut world, PLAYER, 76);
    quests::dispatch::notify_level_changed(&mut world, PLAYER, 75, 76);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
    quests::notify_tutorial_mark(&mut world, 1, PLAYER, 101);
    assert!(has_opcode(&drain(&mut rx), TUTORIAL_SHOW_HTML));

    world.cfg.character.disable_tutorial = true;
    quests::notify_login(&mut world, 1, PLAYER);
    assert!(!has_opcode(&drain(&mut rx), TUTORIAL_SHOW_QUESTION_MARK));
}

/// Orven turns away a first class (`-12`) and a low level (`-11`), and
/// offers the quest to a level-76 Gladiator.
#[test]
fn orven_gates_on_level_race_and_class_group() {
    let (mut world, mut rx) = destiny_world();
    let orven = spawn(&mut world, ORVEN);
    set_level_class(&mut world, PLAYER, 76, WARRIOR);
    drain(&mut rx);
    talk(&mut world, orven, Q10341);
    assert_eq!(any_html(&mut rx), page("30857-12.html", orven));

    set_level_class(&mut world, PLAYER, 75, GLADIATOR);
    talk(&mut world, orven, Q10341);
    assert_eq!(any_html(&mut rx), page("30857-11.html", orven));

    set_level_class(&mut world, PLAYER, 76, GLADIATOR);
    talk(&mut world, orven, Q10341);
    assert_eq!(any_html(&mut rx), page("30857-01.htm", orven));
}

/// The Quartermaster's errand: one race tag per fallen Vanguard, the same
/// Vanguard twice gives nothing, and the fourth tag makes cond 3. Handing
/// them in takes the tags and makes cond 4.
#[test]
fn the_four_vanguard_tags() {
    let (mut world, mut rx) = destiny_world();
    let orven = spawn(&mut world, ORVEN);
    talk(&mut world, orven, Q10341);
    event(&mut world, orven, Q10341, "30857-05.htm");
    assert_eq!(cond(&world), Some(1));
    assert_eq!(
        quest_var_int(&world, PLAYER, Q10341, "STARTED_CLASS"),
        GLADIATOR
    );

    let qm = spawn(&mut world, QUARTERMASTER);
    event(&mut world, qm, Q10341, "33407-02.html");
    assert_eq!(cond(&world), Some(2));

    let first = spawn(&mut world, VANGUARDS[0]);
    drain(&mut rx);
    event(&mut world, first, Q10341, "collectTag");
    // The tag's inventory update leads with an EX packet, which `any_html`
    // would misread as a quest window.
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        page("vanguard-02.html", first)
    );
    event(&mut world, first, Q10341, "collectTag");
    assert_eq!(any_html(&mut rx), page("vanguard-03.html", first));
    assert_eq!(item_count(&world, PLAYER, HUMAN_TAG), 1);

    for &id in &VANGUARDS[1..] {
        let v = spawn(&mut world, id);
        event(&mut world, v, Q10341, "collectTag");
    }
    assert_eq!(item_count(&world, PLAYER, HUMAN_TAG), 4);
    assert_eq!(cond(&world), Some(3));

    event(&mut world, qm, Q10341, "33407-05.html");
    assert_eq!(cond(&world), Some(4));
    assert_eq!(item_count(&world, PLAYER, HUMAN_TAG), 0);
}

/// The whole Evil Incubator run, then Orven's transfer into a Duelist.
#[test]
fn a_gladiator_fights_through_the_incubator_and_becomes_a_duelist() {
    let (mut world, mut rx) = ready_for_the_incubator();
    let member = spawn(&mut world, VANGUARD_MEMBER);
    world.objects.add_components(
        &PLAYER,
        crate::model::components::player::LastFolkNpc(member),
    );
    event(&mut world, member, "EvilIncubator", "enterInstance");
    let id = world
        .instances
        .allowed_instance_of(PLAYER)
        .expect("entered");
    assert_eq!(world.instances.get(id).unwrap().template_id, INCUBATOR);
    assert_eq!(cond(&world), Some(5));

    let adolph = in_world(&world, id, ADOLPH)[0];
    event(&mut world, adolph, "EvilIncubator", "33170-02.html");
    assert_eq!(cond(&world), Some(6));

    // Pick Barton and Hayuk; Alice and Eliyah are sent away.
    let barton = in_world(&world, id, BARTON)[0];
    let hayuk = in_world(&world, id, HAYUK)[0];
    event(&mut world, barton, "EvilIncubator", "selectHelper");
    assert_eq!(cond(&world), Some(6));
    event(&mut world, hayuk, "EvilIncubator", "selectHelper");
    assert_eq!(cond(&world), Some(7));
    assert!(in_world(&world, id, ALICE).is_empty());
    assert!(in_world(&world, id, ELIYAH).is_empty());
    assert_eq!(in_world(&world, id, BARTON), vec![barton]);
    let at = *world.objects.get_component::<Position>(&barton).unwrap();
    assert_eq!((at.x, at.y), (56005, -175536));

    event(&mut world, adolph, "EvilIncubator", "teamSelected");
    assert_eq!(cond(&world), Some(8));
    assert!(alive_monsters(&world, id).is_empty());
    advance_ticks(&mut world, 60);
    assert_eq!(wave(&world, id), 2);
    assert_eq!(alive_monsters(&world, id).len(), 5, "wave_1");
    // `managerWorldAttack`: the picked officers go for the new monsters.
    let wave_1 = alive_monsters(&world, id);
    for officer in [barton, hayuk] {
        let ai = world
            .objects
            .get_component::<crate::model::npc::NpcAi>(&officer)
            .unwrap();
        assert_eq!(ai.intention, crate::model::npc::NpcIntention::Attack);
        assert!(
            wave_1.contains(&ai.target),
            "officer {officer} attacks a wave monster"
        );
    }

    // Waves 1–6 follow one another as each is cleared; then the pause.
    for next in 3..=8 {
        for mob in alive_monsters(&world, id) {
            game_loop::npc::npc_do_die(&mut world, mob, PLAYER);
        }
        advance_ticks(&mut world, 60);
        assert_eq!(wave(&world, id), next);
    }
    assert_eq!(cond(&world), Some(9));

    drain(&mut rx);
    event(&mut world, adolph, "EvilIncubator", "giveItem");
    assert_eq!(cond(&world), Some(10));
    assert_eq!(item_count(&world, PLAYER, GLADIATOR_PENDANT), 1);
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        instance_page("33170-06.html", adolph)
    );

    // Waves 8–15 come every 20 s whatever is left standing; 15 brings the
    // Gladiator's Death Wound.
    event(&mut world, adolph, "EvilIncubator", "resumeFight");
    assert_eq!(cond(&world), Some(11));
    assert_eq!(wave(&world, id), 9);
    for next in 10..=16 {
        advance_ticks(&mut world, 210);
        assert_eq!(wave(&world, id), next);
    }
    let boss = in_world(&world, id, GLADIATOR_BOSS);
    assert_eq!(boss.len(), 1);
    game_loop::npc::npc_do_die(&mut world, boss[0], PLAYER);
    assert_eq!(cond(&world), Some(12));

    talk(&mut world, adolph, "EvilIncubator");
    assert_eq!(cond(&world), Some(13));
    assert_eq!(item_count(&world, PLAYER, SOE), 1);

    // Out of the instance, Orven does the transfer.
    game_loop::space::instances::exit(&mut world, PLAYER);
    let orven = spawn(&mut world, ORVEN);
    drain(&mut rx);
    talk(&mut world, orven, Q10341);
    assert_eq!(any_html(&mut rx), page("30857-07.html", orven));
    event(&mut world, orven, Q10341, "nextClassInfo");
    assert_eq!(any_html(&mut rx), page("class_preview_duelist.html", orven));

    event(&mut world, orven, Q10341, "classTransfer");
    assert_eq!(class_id(&world), DUELIST);
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        page("30857-09.html", orven)
    );
    assert_eq!(item_count(&world, PLAYER, GLADIATOR_PENDANT), 0);
    assert_eq!(item_count(&world, PLAYER, 46852), 1, "Paulina's set");
    assert!(
        world
            .objects
            .get_component::<Quests>(&PLAYER)
            .is_none_or(|q| !q.0.contains_key(Q10341)),
        "exitQuest(true): repeatable, so the state is dropped"
    );
}
