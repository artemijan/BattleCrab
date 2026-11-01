//! The Salvation awakening: Seize Your Destiny (10338), its instance Harnak
//! Underground Ruins (195) and the Awakening Masters, on the real templates
//! and htmls.

use super::*;
use crate::game_loop::quests;

const PLAYER: i32 = 7304;
const Q10338: &str = "Q10338_SeizeYourDestiny";
const HARNAK: &str = "HarnakUndergroundRuins";
const MASTER: &str = "AwakeningMaster";
const HARNAK_TEMPLATE: i32 = 195;

const CELLPHINE: i32 = 33477;
const HADEL: i32 = 33344;
const HERMUNCUS: i32 = 33340;
const SIGEL_MASTER: i32 = 33397;
const TYRR_MASTER: i32 = 33398;
const HARNAKS_WRAITH: i32 = 27445;
const POWER_SOURCES: [i32; 3] = [33501, 33556, 33557];
/// The first room's guards, in the order they are called up.
const FIRST_ROOM: [i32; 8] = [27440, 27437, 27442, 27438, 27441, 27443, 27439, 27454];
const KRAKIA_BATHUS: i32 = 27437;
const SEAL_CONTROL_DEVICE: i32 = 33548;

const SCROLL_OF_AFTERLIFE: i32 = 17600;
const STEEL_DOOR_GUILD_COIN: i32 = 37045;
const TYRR_AWAKE_POWER: i32 = 32265;
const CHAOS_POMANDER: i32 = 37374;

const HUMAN: i32 = 0;
const ERTHEIA: i32 = 6;
const DUELIST: i32 = 88;
const TYRR_DUELIST: i32 = 152;
const POWER_STRIKE: i32 = 3;
const RACE_SKILLS: [i32; 2] = [1901, 1902]; // Aim, Mental Fortress

const ZONE_ROOM_2: i32 = 200032;
const ZONE_ROOM_3: i32 = 200033;

const SPOT: (i32, i32, i32) = (-114_962, 226_564, -2_864);

/// A level-85 Human Duelist.
fn destiny_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = quest_test_world();
    world.data = dist::game_data_owned();
    // The dist's `MaximumPlayerLevel = 110`, incremented; the config
    // default is still 81, which would cap Harnak's level-85 entry.
    world.cfg.character.maximum_player_level = 111;
    let rx = ingame_player(&mut world, 1, PLAYER, SPOT.0, SPOT.1, SPOT.2);
    set_level_race_class(&mut world, PLAYER, 85, HUMAN, DUELIST);
    let exp = world.data.experience.exp_for_level(85);
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
    world
        .objects
        .add_components(&PLAYER, crate::model::components::player::LastFolkNpc(npc));
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {script} {event}")),
    );
}

fn talk(world: &mut World, npc: i32) {
    stand_by(world, npc);
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{npc}_Script {Q10338}")),
    );
}

fn page(file: &str, npc: i32) -> String {
    dist_page(
        &format!("data/scripts/quests/Q10338_SeizeYourDestiny/{file}"),
        npc,
    )
}

fn master_page(file: &str, npc: i32) -> String {
    dist_page(
        &format!("data/scripts/ai/areas/TalkingIsland/AwakeningMaster/{file}"),
        npc,
    )
}

fn cond(world: &World) -> Option<i32> {
    quest_cond(world, PLAYER, Q10338)
}

fn class_id(world: &World) -> i32 {
    world
        .objects
        .get_component::<Player>(&PLAYER)
        .unwrap()
        .class_id
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

fn kill_all(world: &mut World, id: i32, npc_id: i32) {
    for npc in alive(world, id, npc_id) {
        game_loop::npc::npc_do_die(world, npc, PLAYER);
    }
}

fn status(world: &World, id: i32) -> i32 {
    world.instances.status(id)
}

/// Strike `npc` at `hp_percent` of its HP (`onAttack`).
fn strike_at(world: &mut World, npc: i32, hp_percent: f64) {
    let v = world
        .objects
        .get_component_mut::<crate::model::components::stats::Vitals>(&npc)
        .unwrap();
    v.cur_hp = v.max_hp as f64 * hp_percent / 100.0;
    let npc_id = game_loop::npc::npc_id_of(world, npc).unwrap();
    quests::notify_attack(world, PLAYER, npc, npc_id, None, false);
}

/// Q10338 at cond 2: Cellphine's start and Hadel's errand.
fn started_at_cond_2() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, rx) = destiny_world();
    let cellphine = spawn(&mut world, CELLPHINE);
    talk(&mut world, cellphine);
    event(&mut world, cellphine, Q10338, "33477-03.html");
    assert_eq!(cond(&world), Some(1));
    assert_eq!(
        quest_var_int(&world, PLAYER, Q10338, "STARTED_CLASS"),
        DUELIST
    );
    let hadel = spawn(&mut world, HADEL);
    event(&mut world, hadel, Q10338, "33344-05.html");
    assert_eq!(cond(&world), Some(2));
    (world, rx)
}

/// Hadel's `enter_instance`: the player is in a fresh Harnak world.
fn enter_harnak(world: &mut World) -> i32 {
    let hadel = spawn(world, HADEL);
    event(world, hadel, HARNAK, "enter_instance");
    let id = world
        .instances
        .allowed_instance_of(PLAYER)
        .expect("entered");
    assert_eq!(
        world.instances.get(id).unwrap().template_id,
        HARNAK_TEMPLATE
    );
    id
}

/// Harnak at status 3: the opening's timer has spawned the Wraith's room.
fn at_the_wraith(world: &mut World, id: i32) -> i32 {
    quests::dispatch::notify_enter_zone(world, PLAYER, ZONE_ROOM_3);
    advance_ticks(world, 300);
    assert_eq!(status(world, id), 3);
    alive(world, id, HARNAKS_WRAITH)[0]
}

/// Cellphine: an Ertheia gets `-08`, a level 84 `-07`, and a level-85
/// fourth class her `-01.htm` as a plain chat window.
#[test]
fn cellphine_gates_on_race_level_and_class() {
    let (mut world, mut rx) = destiny_world();
    let cellphine = spawn(&mut world, CELLPHINE);
    set_level_race_class(&mut world, PLAYER, 85, ERTHEIA, 182);
    drain(&mut rx);
    talk(&mut world, cellphine);
    assert_eq!(any_html(&mut rx), page("33477-08.html", cellphine));

    set_level_race_class(&mut world, PLAYER, 84, HUMAN, DUELIST);
    talk(&mut world, cellphine);
    assert_eq!(any_html(&mut rx), page("33477-07.html", cellphine));

    set_level_race_class(&mut world, PLAYER, 85, HUMAN, DUELIST);
    talk(&mut world, cellphine);
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        page("33477-01.htm", cellphine)
    );
}

/// The whole quest: Harnak's three rooms on a Duelist (the warrior wave is
/// Krakia Bathus), the Wraith's fall (cond 3) and Hermuncus, then the
/// Scroll of Afterlife.
#[test]
fn a_duelist_frees_hermuncus_and_gets_the_scroll() {
    let (mut world, _rx) = started_at_cond_2();
    let id = enter_harnak(&mut world);

    // `onEnter`'s 10 s timer brings the first room, Rakzan first.
    advance_ticks(&mut world, 101);
    assert_eq!(status(&world, id), 1);
    assert_eq!(world.instances.get_var(id, "currentNpc"), 27440);
    for (i, &guard) in FIRST_ROOM.iter().enumerate() {
        kill_all(&mut world, id, guard);
        if let Some(&next) = FIRST_ROOM.get(i + 1) {
            assert_eq!(world.instances.get_var(id, "currentNpc"), i64::from(next));
        }
    }
    advance_ticks(&mut world, 2);
    for source in POWER_SOURCES {
        assert_eq!(alive(&world, id, source).len(), 1, "power source {source}");
    }

    // The second room: three waves of the warrior monster.
    quests::dispatch::notify_enter_zone(&mut world, PLAYER, ZONE_ROOM_2);
    assert_eq!(status(&world, id), 2);
    advance_ticks(&mut world, 52);
    for (wave, count) in [(1, 1), (2, 3), (3, 11)] {
        assert_eq!(world.instances.get_var(id, "wave"), wave);
        assert_eq!(alive(&world, id, KRAKIA_BATHUS).len(), count, "wave {wave}");
        kill_all(&mut world, id, KRAKIA_BATHUS);
        advance_ticks(&mut world, 2);
    }

    // The third room: the Wraith's thresholds, then its fall.
    let wraith = at_the_wraith(&mut world, id);
    strike_at(&mut world, wraith, 70.0);
    assert_eq!(status(&world, id), 3);
    game_loop::npc::npc_do_die(&mut world, wraith, PLAYER);
    assert_eq!(cond(&world), Some(3));
    assert!(alive(&world, id, HERMUNCUS).is_empty(), "removeNpcs");
    advance_ticks(&mut world, 251);
    let hermuncus = alive(&world, id, HERMUNCUS)[0];

    event(&mut world, hermuncus, Q10338, "33340-02.html");
    assert_eq!(item_count(&world, PLAYER, SCROLL_OF_AFTERLIFE), 1);
    assert_eq!(item_count(&world, PLAYER, STEEL_DOOR_GUILD_COIN), 400);
    assert!(!quest_started(&world, PLAYER, Q10338));
}

/// Hitting a guard other than the one called up turns the whole first room
/// on the player; clearing it still opens the way.
#[test]
fn striking_out_of_turn_rouses_the_first_room() {
    let (mut world, _rx) = started_at_cond_2();
    let id = enter_harnak(&mut world);
    advance_ticks(&mut world, 101);
    let seknus = alive(&world, id, 27443)[0];
    strike_at(&mut world, seknus, 100.0);
    assert_eq!(status(&world, id), 0);
    for guard in FIRST_ROOM {
        kill_all(&mut world, id, guard);
    }
    assert_eq!(status(&world, id), 1);
    advance_ticks(&mut world, 2);
    assert_eq!(alive(&world, id, POWER_SOURCES[0]).len(), 1);
}

/// The Wraith at half HP exposes the seals on a 60 s clock; letting it run
/// out clears the room and ends the instance.
#[test]
fn the_seal_clock_running_out_fails_the_instance() {
    let (mut world, _rx) = started_at_cond_2();
    let id = enter_harnak(&mut world);
    // Skip ahead to the second room cleared.
    advance_ticks(&mut world, 101);
    world.instances.set_status(id, 2);
    world
        .instances
        .set_var(id, "waveNpcId", i64::from(KRAKIA_BATHUS));
    let wraith = at_the_wraith(&mut world, id);
    // One message per strike: below 80, below 60, then the seals.
    for hp in [70.0, 55.0, 45.0] {
        strike_at(&mut world, wraith, hp);
    }
    assert_eq!(status(&world, id), 4);
    // The first room's Krakia Bathus is still standing in this shortcut.
    let before = alive(&world, id, KRAKIA_BATHUS).len();
    advance_ticks(&mut world, 2);
    assert_eq!(alive(&world, id, SEAL_CONTROL_DEVICE).len(), 2);
    assert_eq!(alive(&world, id, KRAKIA_BATHUS).len(), before + 4);

    advance_ticks(&mut world, 600);
    assert!(alive(&world, id, HARNAKS_WRAITH).is_empty(), "removeNpcs");
    advance_ticks(&mut world, 140);
    assert_eq!(game_loop::helpers::instance_of(&world, PLAYER), 0);
    assert_eq!(cond(&world), Some(2));
}

/// The masters' `awakening`: no scroll is `-no.htm`, the wrong category's
/// master `-no_class.htm`, and the right one sends the awakening dialog.
#[test]
fn the_masters_answer_by_scroll_and_category() {
    let (mut world, mut rx) = destiny_world();
    let tyrr = spawn(&mut world, TYRR_MASTER);
    let sigel = spawn(&mut world, SIGEL_MASTER);
    drain(&mut rx);
    event(&mut world, tyrr, MASTER, "awakening");
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        master_page("33398-no.htm", tyrr)
    );

    give_item(&mut world, PLAYER, 9_000_001, SCROLL_OF_AFTERLIFE, 1);
    event(&mut world, sigel, MASTER, "awakening");
    assert_eq!(
        served_html(&mut rx).unwrap_or_default(),
        master_page("33397-no_class.htm", sigel)
    );

    event(&mut world, tyrr, MASTER, "awakening");
    let dialog = find_ex_opcode(
        &mut rx,
        server_packets::opcodes::EX_CHANGE_TO_AWAKENED_CLASS,
    )
    .expect("ExChangeToAwakenedClass");
    assert_eq!(
        i32::from_le_bytes(dialog[3..7].try_into().unwrap()),
        TYRR_DUELIST
    );
}

/// Accepting the dialog: a Tyrr Duelist, the scroll spent, the Tyrr power
/// and two Chaos Pomanders, the Duelist skills gone for the race ones, and
/// the closing video 10 s later.
#[test]
fn accepting_the_dialog_awakens_a_duelist() {
    let (mut world, mut rx) = destiny_world();
    give_item(&mut world, PLAYER, 9_000_001, SCROLL_OF_AFTERLIFE, 1);
    let max = world.data.skill_data.max_level(POWER_STRIKE);
    assert!(
        !world
            .data
            .skill_trees
            .kept_upon_awakening(TYRR_DUELIST, POWER_STRIKE, max, max)
    );
    world
        .objects
        .get_component_mut::<SkillBook>(&PLAYER)
        .unwrap()
        .0
        .insert(POWER_STRIKE, max);
    drain(&mut rx);

    quests::dispatch::handle_request_change_to_awakened_class(&mut world, 1, &1i32.to_le_bytes());
    assert_eq!(class_id(&world), TYRR_DUELIST);
    let p = world.objects.get_component::<Player>(&PLAYER).unwrap();
    assert_eq!(p.base_class_id, TYRR_DUELIST);
    assert_eq!(item_count(&world, PLAYER, SCROLL_OF_AFTERLIFE), 0);
    assert_eq!(item_count(&world, PLAYER, TYRR_AWAKE_POWER), 1);
    assert_eq!(item_count(&world, PLAYER, CHAOS_POMANDER), 2);
    assert!(!knows(&world, POWER_STRIKE, PLAYER));
    for skill in RACE_SKILLS {
        assert!(knows(&world, skill, PLAYER), "race skill {skill}");
    }
    let out = drain(&mut rx);
    assert!(has_sm(
        &out,
        server_packets::sm_ids::CONGRATULATIONS_YOU_VE_COMPLETED_A_CLASS_TRANSFER
    ));

    advance_ticks(&mut world, 101);
    let usm = find_ex_opcode(&mut rx, server_packets::opcodes::EX_SHOW_USM).expect("ExShowUsm");
    assert_eq!(i32::from_le_bytes(usm[3..7].try_into().unwrap()), 10);
}

/// The refusals: a declined dialog is `ActionFailed`, a hero and a rider
/// get their messages, and none of them changes the class.
#[test]
fn awakening_refuses_heroes_riders_and_a_declined_dialog() {
    let (mut world, mut rx) = destiny_world();
    give_item(&mut world, PLAYER, 9_000_001, SCROLL_OF_AFTERLIFE, 1);
    drain(&mut rx);
    quests::dispatch::handle_request_change_to_awakened_class(&mut world, 1, &0i32.to_le_bytes());
    assert!(has_opcode(
        &drain(&mut rx),
        server_packets::opcodes::ACTION_FAIL
    ));

    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .is_hero = true;
    quests::dispatch::handle_request_change_to_awakened_class(&mut world, 1, &1i32.to_le_bytes());
    assert!(has_sm(
        &drain(&mut rx),
        server_packets::sm_ids::YOU_CANNOT_AWAKEN_WHEN_YOU_ARE_A_HERO_OR_ON_THE_WAIT_LIST_FOR_HERO_STATUS
    ));

    let p = world.objects.get_component_mut::<Player>(&PLAYER).unwrap();
    p.is_hero = false;
    p.mount_type = 1;
    quests::dispatch::handle_request_change_to_awakened_class(&mut world, 1, &1i32.to_le_bytes());
    assert!(has_sm(
        &drain(&mut rx),
        server_packets::sm_ids::YOU_CANNOT_AWAKEN_WHILE_YOU_RE_TRANSFORMED_OR_RIDING
    ));
    assert_eq!(class_id(&world), DUELIST);
    assert_eq!(item_count(&world, PLAYER, SCROLL_OF_AFTERLIFE), 1);
}
