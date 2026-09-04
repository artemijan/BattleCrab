//! `admin/spawn.rs`, `admin/mobgroup.rs`, `admin/npc_info.rs` — spawning,
//! despawning and listing NPCs, and the scan/show-quests panels.

use super::*;

/// **`//list_spawns` / `goSpawn` has to find territory spawns** (GitHub #3).
/// Java lists `SpawnTable.getSpawns(npcId)` — the live spawn objects — while
/// this walked the *loaded definitions* and kept only those with a fixed
/// `<npc>` location. Most of this dist spawns inside `<territory>` polygons,
/// where the definition carries no point, so the answer was always "No current
/// spawns found".
#[test]
fn list_spawns_finds_a_territory_spawned_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7205, 100);
    drain(&mut gm_rx);

    // A live NPC with no fixed spawn definition behind it — exactly the shape a
    // territory spawn produces.
    let npc_oid = NPC_OID + 61;
    let npc_id = 90301;
    add_test_npc(
        &mut world, npc_oid, npc_id, "Monster", 1, 12_345, 23_456, 780,
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body(&format!("list_spawns {npc_id}")),
        ]
        .concat(),
    );

    let lines = drain(&mut gm_rx);
    let texts: Vec<String> = lines
        .iter()
        .filter_map(|p| system_message_text(p))
        .collect();
    assert!(
        texts.iter().all(|t| !t.contains("No current spawns found")),
        "the territory spawn is found, not reported missing: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("12345")),
        "and its location is listed: {texts:?}"
    );
}

/// `goSpawn` is `//list_spawns <id> 1` — the teleport form. It has to land the
/// GM on the spawn it just listed.
#[test]
fn gospawn_teleports_to_the_listed_spawn() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7206, 100);
    drain(&mut gm_rx);

    let npc_oid = NPC_OID + 62;
    let npc_id = 90302;
    add_test_npc(
        &mut world, npc_oid, npc_id, "Monster", 1, 54_321, 65_432, 900,
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body(&format!("list_spawns {npc_id} 1")),
        ]
        .concat(),
    );

    let pos = *world
        .objects
        .get_component::<crate::model::components::space::Position>(&7206)
        .expect("gm position");
    assert_eq!(
        (pos.x, pos.y),
        (54_321, 65_432),
        "goSpawn put the GM on the spawn"
    );
}

/// **`//spawnnight` / `//spawnday`** (GitHub #1) — the two buttons
/// `data/html/admin/spawn.htm` ships. Java registers no handler for either, so
/// they are dead there; here they force the `DayNightSpawns` phase.
#[test]
fn spawnnight_and_spawnday_force_the_phase() {
    const DAY_MOB: i32 = 24052;
    const NIGHT_MOB: i32 = 24055;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7204, 100);
    drain(&mut gm_rx);
    for npc_id in [DAY_MOB, NIGHT_MOB] {
        let mut t = crate::data::npc_data::default_template(npc_id);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    let line = |npc_id: i32| crate::data::spawn_data::NpcSpawnDef {
        npc_id,
        count: 1,
        loc: Some(crate::data::spawn_data::FixedLoc {
            x: 100,
            y: 100,
            z: 0,
            heading: 0,
        }),
        respawn_secs: 60,
        respawn_random_secs: 0,
        chase_range: 0,
        db_save: false,
    };
    world
        .data
        .spawn_data
        .spawns
        .push(crate::data::spawn_data::SpawnTemplate {
            file: "test/admin-day-night.xml".to_string(),
            name: Some("test-admin-day-night".to_string()),
            ai: Some("DayNightSpawns".to_string()),
            parameters: Default::default(),
            territories: Vec::new(),
            groups: vec![
                crate::data::spawn_data::SpawnGroup {
                    name: Some("dayTime".to_string()),
                    spawn_by_default: false,
                    territories: Vec::new(),
                    npcs: vec![line(DAY_MOB)],
                },
                crate::data::spawn_data::SpawnGroup {
                    name: Some("nightTime".to_string()),
                    spawn_by_default: false,
                    territories: Vec::new(),
                    npcs: vec![line(NIGHT_MOB)],
                },
            ],
        });

    let count_of = |world: &mut World, npc_id: i32| {
        let mut n = 0;
        world
            .objects
            .for_each_mut::<&crate::model::npc::Npc>(|npc| {
                if npc.npc_id == npc_id {
                    n += 1;
                }
            });
        n
    };

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("spawnnight"),
        ]
        .concat(),
    );
    assert_eq!(count_of(&mut world, NIGHT_MOB), 1, "night half is up");
    assert_eq!(count_of(&mut world, DAY_MOB), 0, "day half is not");

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("spawnday")].concat(),
    );
    assert_eq!(count_of(&mut world, DAY_MOB), 1, "they traded places");
    assert_eq!(count_of(&mut world, NIGHT_MOB), 0);
}

/// **`//respawnall` has to make the NPCs visible** (GitHub #2). The boot spawn
/// pass places NPCs without announcing them — at boot there is nobody to
/// announce to — so a GM running it on a live world got a field that looked
/// empty until they walked out of the region and back.
#[test]
fn respawnall_shows_the_new_npcs_to_a_player_standing_there() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7203, 100);
    drain(&mut gm_rx);

    let gm_pos = *world
        .objects
        .get_component::<crate::model::components::space::Position>(&7203)
        .expect("gm position");
    let npc_id = 90201;
    let mut template = crate::data::npc_data::default_template(npc_id);
    template.type_name = "Monster".into();
    world.data.npc_data.insert_for_test(template);
    world
        .data
        .spawn_data
        .spawns
        .push(crate::data::spawn_data::SpawnTemplate {
            file: "test/respawnall.xml".to_string(),
            name: Some("test-respawnall".to_string()),
            ai: None,
            parameters: Default::default(),
            territories: Vec::new(),
            groups: vec![crate::data::spawn_data::SpawnGroup {
                name: None,
                spawn_by_default: true,
                territories: Vec::new(),
                npcs: vec![crate::data::spawn_data::NpcSpawnDef {
                    npc_id,
                    count: 1,
                    loc: Some(crate::data::spawn_data::FixedLoc {
                        x: gm_pos.x,
                        y: gm_pos.y,
                        z: gm_pos.z,
                        heading: 0,
                    }),
                    respawn_secs: 60,
                    respawn_random_secs: 0,
                    chase_range: 0,
                    db_save: false,
                }],
            }],
        });

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("respawnall"),
        ]
        .concat(),
    );

    let npc_infos = drain(&mut gm_rx)
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::NPC_INFO)
        .count();
    assert!(
        npc_infos >= 1,
        "the respawned NPC announced itself to the GM standing on top of it"
    );
}

/// `//delete` despawns the targeted NPC and broadcasts DeleteObject.
#[test]
fn admin_delete_despawns_targeted_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7601, 100);
    drain(&mut gm_rx);

    let npc_oid = game_loop::npc::FIRST_NPC_OBJECT_ID + 1;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 1, 2, 3, 100, 50);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    world
        .objects
        .add_components(&7601, TargetRef(Some(npc_oid)));

    on_packet(&mut world, 1, build_admin("delete"));
    assert!(
        world
            .objects
            .get_component::<model::npc::Npc>(&npc_oid)
            .is_none(),
        "npc despawned by //delete"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "GM got DeleteObject"
    );
}

/// `//delete` with a non-NPC target (or none) warns and deletes nothing.
#[test]
fn admin_delete_without_npc_target_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7603, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("delete"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "select-an-npc line"
    );
}

/// `//spawn` with an unknown NPC id is refused.
#[test]
fn admin_spawn_rejects_unknown_npc() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7602, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("spawn 99999"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "does-not-exist line"
    );
}

/// `//spawn <npcId>` creates the NPC at the GM's location and shows it to them.
#[test]
fn admin_spawn_creates_npc_at_gm() {
    let (mut world, ..) = admin_world();
    world.data.npc_data = dist::npcs_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7604, 100);
    drain(&mut gm_rx);
    set_position(&mut world, 7604, (100, 200, 300));

    let npc_oid = world.next_npc_object_id;
    on_packet(&mut world, 1, build_admin("spawn 30001")); // Lector, a Merchant (non-monster)
    assert_eq!(world.next_npc_object_id, npc_oid + 1, "one NPC spawned");
    let npc = world
        .objects
        .get_component::<model::npc::Npc>(&npc_oid)
        .expect("npc entity exists");
    assert_eq!(npc.npc_id, 30001);
    let pos = world.objects.get_component::<Position>(&npc_oid).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (100, 200, 300), "spawned at the GM");
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "GM was shown the NPC"
    );
}

/// `//spawnat <id> <x> <y> <z>` spawns an NPC at explicit coordinates.
#[test]
fn admin_spawnat_creates_npc_at_coords() {
    let (mut world, ..) = admin_world();
    world.data.npc_data = dist::npcs_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8912, 100);
    drain(&mut gm_rx);
    let npc_oid = world.next_npc_object_id;
    on_packet(
        &mut world,
        1,
        build_admin("spawnat 30001 -84000 244000 -3700"),
    );
    assert_eq!(world.next_npc_object_id, npc_oid + 1, "one NPC spawned");
    let pos = world.objects.get_component::<Position>(&npc_oid).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (-84000, 244000, -3700),
        "spawned at the coords"
    );
}

/// `//mobgroup` lifecycle: create → spawn (members tagged Controllable) →
/// set a state → invul → kill → remove.
#[test]
fn admin_mobgroup_lifecycle() {
    let (mut world, ..) = admin_world();
    world.data.npc_data = dist::npcs_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8940, 100);
    drain(&mut gm_rx);

    // create (no spawn yet)
    on_packet(&mut world, 1, build_admin("mobgroup_create 1 20001 3"));
    assert_eq!(
        world.mob_groups.get(&1).map(|g| g.max_count),
        Some(3),
        "group registered"
    );
    assert!(
        world.mob_groups.get(&1).unwrap().members.is_empty(),
        "not spawned yet"
    );

    // spawn at the GM → 3 Controllable NPCs
    on_packet(&mut world, 1, build_admin("mobgroup_spawn 1"));
    let members: Vec<i32> = world.mob_groups.get(&1).unwrap().members.clone();
    assert_eq!(members.len(), 3, "three mobs spawned");
    for &m in &members {
        assert_eq!(
            world
                .objects
                .get_component::<model::mob_group::Controllable>(&m)
                .map(|c| c.group_id),
            Some(1),
            "member tagged Controllable"
        );
    }

    // state: follow the GM
    on_packet(&mut world, 1, build_admin("mobgroup_follow 1"));
    assert!(matches!(
        world.mob_groups.get(&1).unwrap().state,
        model::mob_group::MobGroupState::Follow(8940)
    ));

    // invul on → each member gets the invul flag
    on_packet(&mut world, 1, build_admin("mobgroup_invul 1 on"));
    assert!(world.mob_groups.get(&1).unwrap().invul, "group invul set");
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&members[0])
            .is_some_and(|f| f.invul),
        "member invul"
    );

    // kill → members become corpses (dead)
    on_packet(&mut world, 1, build_admin("mobgroup_kill 1"));
    assert!(
        members.iter().all(|m| world
            .objects
            .get_component::<Vitals>(m)
            .is_some_and(|v| v.dead)),
        "all members killed"
    );

    // remove → group gone, members despawned
    on_packet(&mut world, 1, build_admin("mobgroup_remove 1"));
    assert!(!world.mob_groups.contains_key(&1), "group removed");
    assert!(
        members
            .iter()
            .all(|m| !world.objects.has_component::<model::npc::Npc>(m)),
        "members despawned"
    );
}

/// `//spawn` with no arguments must not panic (it used to index `args[0]` on an
/// empty token list, killing the game thread) — it answers with the spawns menu
/// and the "doesnt exist" message like an unknown id does.
#[test]
fn admin_spawn_without_args_does_not_panic() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("spawn"));
    let pkts = drain(&mut gm_rx);
    assert!(
        pkts.iter().any(|p| contains_utf16(p, "doesnt exist")),
        "GM is told the (missing) NPC doesnt exist instead of the server dying"
    );
}

// --- `//scan` (AdminScan) ---------------------------------------------------

/// Spawn a scan-target NPC with a real name at an offset from the GM.
fn scan_world() -> (
    World,
    db::CmdTx,
    db::CmdRx,
    UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, a, b, c) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    (world, a, b, c)
}

fn scan_html(pkts: &[Vec<u8>]) -> String {
    pkts.iter()
        .find_map(|p| decode_npc_html(p))
        .expect("scan html")
}

/// `//scan`'s range is a 3D sphere (Java `getVisibleObjectsInRange` measures
/// `calculateDistance3D`, default radius 1000): an NPC on a floor 2000 z away
/// is horizontally on top of the GM yet out of range — the Tower of Insolence
/// stairs case, where Java returns an empty list while the old Rust port
/// dumped every stacked floor into one client-crashing html.
#[test]
fn scan_range_is_a_3d_sphere() {
    let (mut world, ..) = scan_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    scan_npc(&mut world, NPC_OID, 5001, 300, 0, 0); // 3D 300: in
    scan_npc(&mut world, NPC_OID + 1, 5001, 200, 0, 2000); // 3D ~2010: out
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("scan")].concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(
        html.matches("admin_move_to").count(),
        1,
        "only the same-floor NPC is listed: {html}"
    );
    assert!(html.contains("Scan Target"), "{html}");
    assert!(
        html.contains(&format!("objectId={NPC_OID}")),
        "delete link carries the object id: {html}"
    );
}

/// With nothing in range the list is empty — no rows at all (what the Java
/// version shows on the ToI 13F stairs).
#[test]
fn scan_with_nothing_in_range_is_empty() {
    let (mut world, ..) = scan_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    scan_npc(&mut world, NPC_OID, 5001, 3000, 0, 0); // beyond default 1000
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("scan")].concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(html.matches("admin_move_to").count(), 0, "{html}");
}

/// The list pages at 15 rows (Java `PageBuilder`): 20 NPCs in range render 15
/// rows and a pager on the first page, and the remaining 5 on `page=1`. This
/// (with the radius) is what keeps the dialog under the client's html limit.
#[test]
fn scan_paginates_at_fifteen_rows() {
    let (mut world, ..) = scan_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 5001, 100);
    for i in 0..20 {
        scan_npc(&mut world, NPC_OID + i, 5001, 100 + i, 0, 0);
    }
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("scan")].concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(html.matches("admin_move_to").count(), 15, "{html}");
    assert!(html.contains("Page: 1/"), "pager rendered: {html}");
    assert!(
        html.contains("admin_scan page=1"),
        "next-page bypass: {html}"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("scan page=1"),
        ]
        .concat(),
    );
    let html = scan_html(&drain(&mut gm_rx));
    assert_eq!(
        html.matches("admin_move_to").count(),
        5,
        "second page holds the remainder: {html}"
    );
}

/// **Mobs don't notice an invisible GM** (Java `AttackableAI` drops invisible
/// targets; the aggro scan must skip them, with no raid exemption).
#[test]
fn npc_aggro_ignores_hidden_gm() {
    use model::components::player::AdminFlags;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7131, 100);
    drain(&mut gm_rx);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 10, 100, 0, 0);
    assert!(
        ai::notices_target(&world, NPC_OID, 7131),
        "a visible player is noticed"
    );
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&7131)
        .copied()
        .unwrap_or_default();
    flags.hidden = true;
    world.objects.add_components(&7131, flags);
    assert!(
        !ai::notices_target(&world, NPC_OID, 7131),
        "a hidden GM is never noticed"
    );
}

/// **`//unspawnall` clears every NPC and `//respawnall` puts the world
/// back** through the boot spawn pass.
#[test]
fn unspawnall_and_respawnall() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7701, 100);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 10, 100, 0, 0);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("unspawnall"));
    assert!(
        !world.objects.has_component::<model::npc::Npc>(&NPC_OID),
        "all NPCs despawned"
    );

    // The synthetic test world has an empty spawn table — respawnall reports 0.
    on_packet(&mut world, 1, build_admin("respawnall"));
    let msgs = drain(&mut gm_rx);
    assert!(!msgs.is_empty(), "respawnall answers");
}

// ---------------------------------------------------------------------------
// Server control, olympiad manual commands, quest admin
// ---------------------------------------------------------------------------

/// **`//show_quests` is `AdminQuest`'s NPC listing, not `//charquestmenu`.**
/// The two were aliased to the player quest-state editor, so the `Quests`
/// button on the shift-click admin NPC window answered `INVALID_TARGET`
/// instead of listing the scripts registered on the target NPC.
#[test]
fn show_quests_lists_the_target_npcs_scripts() {
    use crate::game_loop::quests;

    struct NpcQuestScript;
    impl quests::QuestScript for NpcQuestScript {
        fn id(&self) -> i32 {
            -42
        }
        fn name(&self) -> &'static str {
            "TestNpcQuest"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[30001]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[30001]
        }
        fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
            None
        }
    }

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world.quests = Arc::new(quests::QuestRegistry::new(vec![Arc::new(NpcQuestScript)]));
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut gm_rx = ingame_player_access(&mut world, 1, 7830, 100);
    // Select the NPC — Java reads `activeChar.getTarget()`, ignoring the
    // template id the html passes as an argument.
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("show_quests 30001"));

    let html = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("npc-quests.htm served");
    assert!(
        html.contains("TestNpcQuest") && html.contains("admin_quest_info TestNpcQuest"),
        "the NPC's script is listed and links into //quest_info, got: {html}"
    );
    // The player-menu columns must NOT be what this button opens.
    assert!(
        !html.contains("CREATED") && !html.contains("STARTED"),
        "this is the NPC listing, not the player quest-state editor"
    );
}

// ---------------------------------------------------------------------------
// Tail polish: tradeoff, cond overrides, reload
// ---------------------------------------------------------------------------

/// `//instance_spawns <id>` lists an instance's live NPCs with a Go link, caps
/// the table at 50 rows and counts the rest as skipped.
#[test]
fn admin_instance_spawns_lists_the_instances_npcs() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 7402, 100);
    drain(&mut rx);

    // Not an instance id at all → Java's "Invalid instance number."
    on_packet(&mut world, 1, build_admin("instance_spawns 0"));
    assert!(
        last_admin_html(&drain(&mut rx)).is_none(),
        "no page for a rejected id"
    );

    let instance_id = world.instances.create(136);
    {
        // `add_test_npc`'s 4th argument is the *type* name; the listing prints
        // the template's display name, so it is set here.
        let mut t = crate::data::npc_data::default_template(20001);
        t.name = "Imperial Guard".into();
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);
    world.instances.record_npc(instance_id, NPC_OID);
    on_packet(
        &mut world,
        1,
        build_admin(&format!("instance_spawns {instance_id}")),
    );
    let page = last_admin_html(&drain(&mut rx)).expect("spawn page");
    assert!(page.contains(&format!("Spawns for {instance_id}")));
    assert!(page.contains("Imperial Guard"), "the live NPC is listed");
    assert!(
        page.contains("bypass -h admin_move_to"),
        "with a Go link to its position"
    );
    assert!(page.contains("Skipped:</td><td>0"), "nothing was skipped");

    // An id with no instance behind it.
    drain(&mut rx);
    on_packet(&mut world, 1, build_admin("instance_spawns 999"));
    assert!(last_admin_html(&drain(&mut rx)).is_none());
}

/// **`//list_spawns` takes a name as well as an id** (GitHub #4). Java
/// concatenates the middle words of the command and, when they aren't a
/// number, resolves them through `NpcData.getTemplateByName` — so typing a
/// name into the menu's box is meant to work. It answered with the usage line
/// instead.
#[test]
fn list_spawns_accepts_an_npc_name() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7701, 100);
    drain(&mut gm_rx);

    let npc_oid = NPC_OID + 71;
    let npc_id = 90401;
    let mut template = crate::data::npc_data::default_template(npc_id);
    template.type_name = "Monster".into();
    template.name = "Rotting Tree".into();
    world.data.npc_data.insert_for_test(template);
    add_test_npc(
        &mut world, npc_oid, npc_id, "Monster", 1, 31_337, 42_042, 100,
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("list_spawns Rotting Tree"),
        ]
        .concat(),
    );

    let texts: Vec<String> = drain(&mut gm_rx)
        .iter()
        .filter_map(|p| system_message_text(p))
        .collect();
    assert!(
        texts.iter().all(|t| !t.contains("Command format")),
        "the name resolved rather than falling back to usage: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("31337")),
        "and the spawn is listed: {texts:?}"
    );
}
