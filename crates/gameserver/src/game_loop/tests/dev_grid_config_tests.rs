//! `General.ini`'s developer switches, packet tracing and the region grid:
//! `AltDevNoSpawns`, `AltDevShowScriptsLoadInLogs`, the four `Debug*Packets`
//! keys plus `ExcludedPacketList`, `GridsAlwaysOn`,
//! `GridNeighborTurnOn/OffTime` and `ShowServerNews`.
//!
//! Two of the eleven have no test, for the same reason each time — there is no
//! behaviour to assert:
//!
//! * **`AltDevShowScriptsLoadInLogs`** emits boot log lines and nothing else,
//!   like `AltDevShowQuestsLoadInLogs` beside it (untested for the same reason
//!   when the quest cluster landed). The *split* between the two keys is real
//!   — `id() > 0` picks which one a script answers to — but both sides of it
//!   are `info!` calls.
//! * **`Developer`** has no consumer at all: all seven Java sites guard a
//!   `catch` block logging an exception the port cannot throw, so it is
//!   recorded in `config::general`'s module header rather than given a field.

use super::*;
use crate::game_loop::npc::ai::refresh_active_regions;
use crate::world::region_of;

/// **The grid keys are hysteresis, and the port had none.**
///
/// A region stays awake for `GridNeighborTurnOffTime` after the last player
/// leaves its neighbourhood. Without it an NPC walking home after losing its
/// target froze the instant the player moved two cells away and resumed when
/// they came back; Java keeps it thinking for another 90 s, which is long
/// enough for the walk to finish.
#[test]
fn a_region_stays_active_after_the_last_player_leaves() {
    let (mut world, ..) = test_world();
    world.cfg.general.grids_always_on = false;
    world.cfg.general.grid_neighbor_turn_on_secs = 0;
    world.cfg.general.grid_neighbor_turn_off_secs = 90;

    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    let home = region_of(0, 0);
    assert!(refresh_active_regions(&mut world).contains(&home));

    // The player walks well clear of the neighbourhood.
    world.set_player_region(100, region_of(600_000, 600_000));
    world.tick += 10 * 10;
    assert!(
        refresh_active_regions(&mut world).contains(&home),
        "ten seconds later the region is still thinking"
    );

    world.tick += 81 * 10;
    assert!(
        !refresh_active_regions(&mut world).contains(&home),
        "…and sleeps once GridNeighborTurnOffTime has passed"
    );
    assert!(
        !world.region_activation.contains_key(&home),
        "the expired entry is pruned rather than accumulating forever"
    );
}

/// `GridNeighborTurnOnTime` delays the *neighbours* and never the player's own
/// cell — Java activates that one inline and schedules the other eight.
///
/// Both halves matter: a delay applied to the player's own cell would freeze
/// every mob standing next to them for a second on each region crossing.
#[test]
fn the_turn_on_delay_applies_to_neighbours_but_not_the_players_own_region() {
    let (mut world, ..) = test_world();
    world.cfg.general.grids_always_on = false;
    world.cfg.general.grid_neighbor_turn_on_secs = 5;
    world.cfg.general.grid_neighbor_turn_off_secs = 90;

    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    let home = region_of(0, 0);
    let neighbour = (home.0 + 1, home.1);

    let active = refresh_active_regions(&mut world);
    assert!(
        active.contains(&home),
        "the player's own cell wakes at once"
    );
    assert!(
        !active.contains(&neighbour),
        "a neighbour waits GridNeighborTurnOnTime first"
    );
    assert!(
        world.region_activation.contains_key(&neighbour),
        "…but it is already scheduled, not merely absent"
    );

    world.tick += 5 * 10;
    assert!(
        refresh_active_regions(&mut world).contains(&neighbour),
        "the neighbour joins once the delay elapses"
    );
}

/// `GridsAlwaysOn` (**False** here) short-circuits both timers: every region
/// holding an NPC is active however far away the nearest player is.
#[test]
fn grids_always_on_activates_every_npc_region() {
    let (mut world, ..) = test_world();
    // Far outside any player's 3x3 neighbourhood — and there are no players.
    add_test_npc(&mut world, 9001, 20001, "Monster", 10, 600_000, 600_000, 0);
    let far = region_of(600_000, 600_000);

    world.cfg.general.grids_always_on = false;
    assert!(
        !refresh_active_regions(&mut world).contains(&far),
        "with the key off an unvisited region never wakes"
    );

    world.cfg.general.grids_always_on = true;
    assert!(
        refresh_active_regions(&mut world).contains(&far),
        "with it on every NPC region is active"
    );
}

/// `AltDevNoSpawns` (**False**) boots a world with no NPCs in it at all.
#[test]
fn alt_dev_no_spawns_places_nothing() {
    let spawn_world = || {
        let (mut world, ..) = test_world();
        let mut t = crate::data::npc_data::default_template(20001);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
        world
            .data
            .spawn_data
            .spawns
            .push(crate::data::spawn_data::SpawnTemplate {
                file: "test/dev.xml".to_string(),
                name: None,
                ai: None,
                parameters: Default::default(),
                territories: vec![],
                groups: vec![crate::data::spawn_data::SpawnGroup {
                    name: None,
                    spawn_by_default: true,
                    territories: vec![],
                    npcs: vec![crate::data::spawn_data::NpcSpawnDef {
                        npc_id: 20001,
                        count: 1,
                        loc: Some(crate::data::spawn_data::FixedLoc {
                            x: 0,
                            y: 0,
                            z: 0,
                            heading: 0,
                        }),
                        respawn_secs: 60,
                        respawn_random_secs: 0,
                        chase_range: 0,
                        db_save: false,
                    }],
                }],
            });
        world
    };

    let mut off = spawn_world();
    assert!(
        crate::game_loop::npc::spawn_all(&mut off) > 0,
        "sanity: the spawn list places an NPC with the key off"
    );

    let mut on = spawn_world();
    on.cfg.general.alt_dev_no_spawns = true;
    assert_eq!(
        crate::game_loop::npc::spawn_all(&mut on),
        0,
        "AltDevNoSpawns places nothing"
    );
    assert!(
        on.npc_regions.is_empty(),
        "and leaves the region index empty, not merely the count at zero"
    );
}

/// `ShowServerNews` (**False**) is Java's `else if` *after* the clan notice, so
/// a player whose clan has a notice enabled never sees the news however the key
/// is set. Pinned because the obvious implementation — sending both — greets
/// clan members with two popups stacked on login.
#[test]
fn the_server_news_is_the_alternative_to_a_clan_notice() {
    let (mut world, ..) = test_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    drain(&mut rx);

    fn popups(world: &mut World, rx: &mut UnboundedReceiver<bytes::Bytes>) -> usize {
        crate::game_loop::lobby::show_clan_notice_at_login(world, 1, 100);
        drain(rx)
            .iter()
            .filter(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
            .count()
    }

    world.cfg.general.show_server_news = false;
    assert_eq!(popups(&mut world, &mut rx), 0, "no clan, key off: nothing");

    world.cfg.general.show_server_news = true;
    assert_eq!(
        popups(&mut world, &mut rx),
        1,
        "no clan, key on: servnews.htm"
    );

    // Now give the player a clan whose notice is enabled: the news must lose.
    const CLAN: i32 = 900;
    world
        .objects
        .get_component_mut::<model::Player>(&100)
        .expect("player")
        .clan_id = CLAN;
    world
        .clan_notices
        .insert(CLAN, (true, "hello\nthere".to_string()));
    assert_eq!(
        popups(&mut world, &mut rx),
        1,
        "a clan notice is one popup, not the notice plus the news"
    );

    // A *disabled* notice is the same as no notice, so the news comes back.
    world.clan_notices.insert(CLAN, (false, String::new()));
    assert_eq!(
        popups(&mut world, &mut rx),
        1,
        "a disabled notice falls through to the news"
    );
    world.cfg.general.show_server_news = false;
    assert_eq!(
        popups(&mut world, &mut rx),
        0,
        "…and with the key off, to nothing"
    );
}

/// The four trace keys and `ExcludedPacketList`.
///
/// **`DebugUnknownPackets = True` is inert on this dist, and not because of its
/// own value**: Java nests it inside the client-packet trace switch, so with
/// `DebugClientPackets = False` the unknown-packet branch is unreachable too.
/// The port reproduces that nesting; this asserts it, so a later "fix" that
/// hoists the unknown branch out has to argue with a test.
#[test]
fn the_packet_trace_is_gated_per_direction_and_honours_the_exclusion_list() {
    use crate::game_loop::dispatch::{client_packet_trace_line, server_packet_trace_line};
    let (mut world, ..) = test_world();

    // Dist values: everything off.
    for key in [
        world.cfg.general.debug_client_packets,
        world.cfg.general.debug_ex_client_packets,
        world.cfg.general.debug_server_packets,
    ] {
        assert!(!key, "the dist ships the trace off");
    }
    assert!(client_packet_trace_line(&world, 0x49, false).is_none());
    assert!(client_packet_trace_line(&world, 0x005F, true).is_none());
    assert!(server_packet_trace_line(&world, 0x32).is_none());

    // Each key gates exactly its own direction.
    world.cfg.general.debug_client_packets = true;
    assert_eq!(
        client_packet_trace_line(&world, 0x49, false).as_deref(),
        Some("[C] 0x49")
    );
    assert!(
        client_packet_trace_line(&world, 0x005F, true).is_none(),
        "DebugClientPackets does not turn on the extended half"
    );
    world.cfg.general.debug_ex_client_packets = true;
    assert_eq!(
        client_packet_trace_line(&world, 0x005F, true).as_deref(),
        Some("[C-Ex] 0xD0:0x005F")
    );
    assert!(
        server_packet_trace_line(&world, 0x32).is_none(),
        "…nor the outbound one"
    );
    world.cfg.general.debug_server_packets = true;
    assert_eq!(
        server_packet_trace_line(&world, 0x32).as_deref(),
        Some("[S] 0x32")
    );

    // `ExcludedPacketList` matches the opcode text the port logs — Java matches
    // the packet *class* name, which this port has no equivalent of; the
    // deviation is documented on `log_client_packet`.
    world.cfg.general.excluded_packets = vec!["0x49".into(), "0xd0:0x005f".into()];
    assert!(
        client_packet_trace_line(&world, 0x49, false).is_none(),
        "an excluded inbound opcode is skipped"
    );
    assert!(
        client_packet_trace_line(&world, 0x005F, true).is_none(),
        "matching is case-insensitive, as Java's `equalsIgnoreCase` is"
    );
    assert!(
        client_packet_trace_line(&world, 0x4A, false).is_some(),
        "a neighbouring opcode still traces"
    );
    assert!(
        server_packet_trace_line(&world, 0x32).is_some(),
        "the list is matched against the label, so `0x49` does not mute 0x32"
    );
}
