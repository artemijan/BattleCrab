//! Wyvern riding & flight: the ride-speed substitution, the movement/
//! `ValidatePosition` flight exemptions, the dismount gates, and the
//! `WyvernManager` NPC script (`ai/others/WyvernManager`).

use super::*;

use crate::model::clan::Clan;
use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::model::Player;
use crate::network::server_packets::sm_ids;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
const CRYSTAL_B: i32 = 1460;

/// **`//ride_wyvern` produces a flying mount moving at the wyvern's
/// `speed_on_ride`, and dismounting mid-air is refused.** The speed is the
/// ride row's 250 plus the class `RunSpdBoost` 35 (Java `SpeedFinalizer
/// .getBaseSpeed`); hanging above z 10000 the dismount is blocked until the
/// rider lands (Java `Player.dismount`).
#[test]
fn admin_ride_wyvern_flies_at_ride_speed_and_gates_dismount() {
    let (mut world, ..) = admin_world();
    world.data.pet_data = crate::data::pet_data::PetData::load_from(DIST);
    let mut gm_rx = ingame_player_access(&mut world, 1, 8920, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("ride_wyvern"));
    {
        let p = world.objects.get_component::<Player>(&8920).unwrap();
        assert_eq!(p.mount_type, 2, "wyvern = MountType 2");
        assert_eq!(p.mount_npc_id, 12621);
        assert!(p.is_flying(), "a wyvern rider is flying");
    }
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&8920)
            .unwrap()
            .run_spd,
        285.0,
        "speed_on_ride 250 + RunSpdBoost 35 replaces the class run speed"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|pk| pk[0] == server_packets::opcodes::RIDE),
        "Ride broadcast sent"
    );

    // Hanging in the sky (z > 10000, no water below): dismount refused.
    world
        .objects
        .get_component_mut::<Position>(&8920)
        .unwrap()
        .z = 15000;
    on_packet(&mut world, 1, build_admin("unride"));
    assert!(
        world
            .objects
            .get_component::<Player>(&8920)
            .unwrap()
            .is_flying(),
        "mid-air dismount is refused"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|pk| pk[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && i16::from_le_bytes(pk[1..3].try_into().unwrap())
                    == sm_ids::YOU_ARE_NOT_ALLOWED_TO_DISMOUNT_IN_THIS_LOCATION),
        "the Java refusal SM is sent"
    );

    // Landed: the dismount goes through and the class speed comes back.
    world
        .objects
        .get_component_mut::<Position>(&8920)
        .unwrap()
        .z = 0;
    on_packet(&mut world, 1, build_admin("unride"));
    let p = world.objects.get_component::<Player>(&8920).unwrap();
    assert_eq!(p.mount_type, 0, "dismounted on the ground");
    assert_ne!(
        world
            .objects
            .get_component::<Speeds>(&8920)
            .unwrap()
            .run_spd,
        285.0,
        "class speed restored"
    );
}

/// **Mounting disarms the rider's hands** (Java `Player.mount` runs
/// `disarmWeapons()`/`disarmShield()` before the `Ride` broadcast). The
/// client renders a mounted paperdoll that still holds a weapon as a
/// ghostly, non-animated mount, so this is part of making the wyvern render
/// at all — and a cursed weapon refuses the whole mount.
#[test]
fn mounting_disarms_the_weapon_and_cursed_refuses() {
    let (mut world, ..) = admin_world();
    world.data.item_data = crate::data::ItemData::load_from(DIST);
    world.data.pet_data = crate::data::pet_data::PetData::load_from(DIST);
    let mut gm_rx = ingame_player_access(&mut world, 1, 8920, 100);

    // Give + equip a Squire's Sword (2369).
    world
        .objects
        .get_component_mut::<Inventory>(&8920)
        .unwrap()
        .add_item(&world.data.item_data, 991_001, 2369, 1);
    let changed = world
        .objects
        .get_component_mut::<Inventory>(&8920)
        .unwrap()
        .equip_item(&world.data.item_data, 991_001);
    assert!(!changed.is_empty(), "the sword equips");
    drain(&mut gm_rx);

    // A cursed weapon blocks the mount outright (Java `disarmWeapons`).
    world
        .objects
        .get_component_mut::<Player>(&8920)
        .unwrap()
        .cursed_weapon_equipped_id = 8190;
    on_packet(&mut world, 1, build_admin("ride_wyvern"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8920)
            .unwrap()
            .mount_type,
        0,
        "cursed weapon refuses the mount"
    );
    world
        .objects
        .get_component_mut::<Player>(&8920)
        .unwrap()
        .cursed_weapon_equipped_id = 0;
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("ride_wyvern"));
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&8920)
            .unwrap()
            .paperdoll_item_id(PaperdollSlot::RHand),
        0,
        "the weapon is disarmed by the mount"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8920)
            .unwrap()
            .mount_type,
        2,
        "mounted after the disarm"
    );
    let pkts = drain(&mut gm_rx);
    assert!(
        pkts.iter()
            .any(|pk| pk[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && i16::from_le_bytes(pk[1..3].try_into().unwrap())
                    == sm_ids::S1_HAS_BEEN_UNEQUIPPED),
        "the unequip SM reaches the rider"
    );
    assert!(
        pkts.iter().any(|pk| pk[0] == server_packets::opcodes::RIDE),
        "Ride still broadcast after the disarm"
    );
}

/// **A flying player's move click goes straight through the geodata wall.**
/// The same click that defers a walker to the path worker (see
/// `move_blocked_by_wall_defers_to_path_worker`) starts a direct move for a
/// wyvern rider — Java gates the whole geodata block on `!_isFlying`. A
/// purely vertical click (dx = dy = 0) also moves instead of being canceled
/// as degenerate (`verticalMovementOnly`).
#[test]
fn flying_move_ignores_geodata_and_allows_vertical_flight() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut rx = ingame_player(&mut world, 1, 4001, 8, 8, 0);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&4001).unwrap();
        speeds.run_spd = 100.0;
        speeds.running = true;
    }
    {
        let p = world.objects.get_component_mut::<Player>(&4001).unwrap();
        p.mount_type = 2;
        p.mount_npc_id = 12621;
    }
    drain(&mut rx);

    // Across the wall at cell 10 — a walker would get PathWait, no Movement.
    handle_move_backward_to_location(&mut world, 1, &move_body((328, 8, 0), (8, 8, 0), 1));
    let md = world
        .objects
        .get_component::<Movement>(&4001)
        .map(|m| m.0.clone())
        .expect("the flight starts immediately");
    assert_eq!(
        (md.dest_x, md.dest_y),
        (328, 8),
        "no geodata clamp while flying"
    );
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::PathWait>(&4001),
        "no path-worker handoff while flying"
    );
    assert_eq!(
        drain(&mut rx)
            .iter()
            .filter(|pk| pk[0] == server_packets::opcodes::MOVE_TO_LOCATION)
            .count(),
        1,
        "MoveToLocation broadcast to the mover"
    );

    // Straight up: dx = dy = 0, dz > 0 — Java's verticalMovementOnly.
    world.objects.remove_component::<Movement>(&4001);
    handle_move_backward_to_location(&mut world, 1, &move_body((8, 8, 500), (8, 8, 0), 1));
    let md = world
        .objects
        .get_component::<Movement>(&4001)
        .map(|m| m.0.clone())
        .expect("vertical flight starts");
    assert!(md.dest_z >= 500, "climbing to the requested z (head level)");
}

/// **`ValidatePosition` trusts a flying client's Z outright.** A +3000 climb
/// report — far outside the walker's ±1500 stairs window — is adopted
/// silently (Java `setXYZ(realX, realY, _z)` for `isFlying()`), where a
/// walker would have been pushed back with `ValidateLocation`.
#[test]
fn validate_position_trusts_flying_client_z() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 4001, 1000, 1000, 0);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&4001).unwrap();
        speeds.run_spd = 600.0;
        speeds.running = true;
    }
    {
        let p = world.objects.get_component_mut::<Player>(&4001).unwrap();
        p.mount_type = 2;
        p.mount_npc_id = 12621;
    }
    super::super::zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 3000, 0));
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (1000, 1000, 3000),
        "the flying client's z is adopted verbatim"
    );
    assert!(
        rx.try_recv().is_err(),
        "no ValidateLocation pushback for a climbing wyvern"
    );
}

// ---------------------------------------------------------------------------
// WyvernManager NPC script
// ---------------------------------------------------------------------------

/// Player 100 + the Gludio castle wyvern manager (35101) as npc oid 701.
fn wyvern_manager_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, _db, _link) = quest_test_world();
    add_test_npc(&mut world, 701, 35101, "Merchant", 75, 0, 0, 0);
    let rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    (world, rx)
}

/// Make player 100 the leader of clan 500 owning `castle_id` (Java
/// `isOwnerClan`: leader of the residence's owner clan).
fn own_castle(world: &mut World, castle_id: i32) {
    let clan_id = 500;
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "Owners".into(),
            leader_id: 100,
            level: 5,
            reputation_score: 0,
            castle_id,
            members: Vec::new(),
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    let p = world.objects.get_component_mut::<Player>(&100).unwrap();
    p.clan_id = clan_id;
}

fn served_html(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Option<String> {
    drain(rx).iter().find_map(|p| decode_npc_html(p))
}

/// **Only the owning clan's leader is served, and the default config keeps
/// castle wyverns behind the Seal of Strife block.** A clanless player gets
/// the turn-away page; the castle lord on this dist's config
/// (`AllowRideWyvernAlways = False`) gets the Dusk page — the same dead end
/// Java serves — never the ride console.
#[test]
fn wyvern_manager_gates_on_ownership_and_seal_of_strife() {
    let (mut world, mut rx) = wyvern_manager_world();

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest WyvernManager Return"),
    );
    let html = served_html(&mut rx).expect("a page for the non-owner");
    assert!(
        html.contains("Only you, my lord"),
        "non-owner sees wyvernmanager-02, got: {html}"
    );

    own_castle(&mut world, 1); // Gludio — 35101's castle
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest WyvernManager Return"),
    );
    let html = served_html(&mut rx).expect("a page for the lord");
    assert!(
        html.contains("Seal of Strife"),
        "castle lord on default config sees the Dusk block, got: {html}"
    );

    // The lord of a *different* castle is still a non-owner here.
    world.clans.get_mut(&500).unwrap().castle_id = 2;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest WyvernManager Return"),
    );
    let html = served_html(&mut rx).expect("a page for the wrong lord");
    assert!(
        html.contains("Only you, my lord"),
        "Dion's lord is refused at Gludio, got: {html}"
    );
}

/// **The full exchange: strider + 25 B-crystals → wyvern.** With the config
/// gate open, RideWyvern refuses a lord who isn't riding a level-55 strider,
/// then — once riding one with the fee in the bags — takes the crystals,
/// swaps the strider for the wyvern (flying, ride speed), and serves the
/// success page.
#[test]
fn wyvern_manager_exchanges_strider_and_crystals_for_wyvern() {
    let (mut world, mut rx) = wyvern_manager_world();
    world.cfg.feature.allow_ride_wyvern_always = true;
    world.data.pet_data = crate::data::pet_data::PetData::load_from(DIST);
    own_castle(&mut world, 1);

    // Not riding a strider → wyvernmanager-05 with the placeholders filled.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest WyvernManager RideWyvern"),
    );
    let html = served_html(&mut rx).expect("the strider-requirement page");
    assert!(
        html.contains("Strider of at least level 55") && html.contains("25 Crystals"),
        "wyvernmanager-05 with %strider_level%/%wyvern_fee% replaced, got: {html}"
    );

    // Riding a level-55 strider but without the fee → wyvernmanager-06.
    {
        let p = world.objects.get_component_mut::<Player>(&100).unwrap();
        p.mount_type = 1;
        p.mount_npc_id = 12526;
        p.mount_level = 55;
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest WyvernManager RideWyvern"),
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&100)
            .unwrap()
            .mount_type
            == 1,
        "no crystals, still on the strider"
    );
    drain(&mut rx);

    // With the 25 crystals: the exchange goes through.
    world
        .objects
        .get_component_mut::<Inventory>(&100)
        .unwrap()
        .add_item(&world.data.item_data, 999_001, CRYSTAL_B, 25);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest WyvernManager RideWyvern"),
    );
    {
        let p = world.objects.get_component::<Player>(&100).unwrap();
        assert_eq!(p.mount_type, 2, "now on the wyvern");
        assert_eq!(p.mount_npc_id, 12621);
        assert!(p.is_flying());
    }
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&100)
            .unwrap()
            .count_of(CRYSTAL_B),
        0,
        "the 25-crystal fee was taken"
    );
    assert_eq!(
        world.objects.get_component::<Speeds>(&100).unwrap().run_spd,
        285.0,
        "wyvern ride speed applied (250 + 35 boost, under the 300 cap)"
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|pk| pk[0] == server_packets::opcodes::RIDE),
        "Ride broadcast"
    );
    assert!(
        pkts.iter()
            .filter_map(|p| decode_npc_html(p))
            .any(|h| h.contains("ready to go")),
        "wyvernmanager-04 (success) served"
    );
}
