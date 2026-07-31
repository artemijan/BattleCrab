//! Wyvern riding & flight: the ride-speed substitution, the movement/
//! `ValidatePosition` flight exemptions, the dismount gates, and the
//! `WyvernManager` NPC script (`ai/others/WyvernManager`).

use super::*;

use crate::model::Player;
use crate::model::clan::Clan;
use crate::model::inventory::{Inventory, PaperdollSlot};
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

/// The hero-glow byte's offset in a captured packet, found by clearing the
/// flag and re-building the same packet: exactly one byte moves, and that is
/// the glow (Java `CharInfo`/`UserInfo`'s `isHero() || (isGM() &&
/// GM_HERO_AURA)`). Located rather than hard-coded so the assertions survive
/// a layout change.
fn glow_offsets(world: &mut World, oid: i32) -> (usize, usize) {
    let build = |world: &World| {
        let v = crate::model::PlayerView::of(&world.objects, oid).unwrap();
        let relation = crate::game_loop::party::calculate_relation(world, v.p);
        (
            crate::network::user_info::user_info(&v, &world.data, &world.cfg.character, relation),
            server_packets::char_info(&v, &[], &[], &Default::default()),
        )
    };
    let was = world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .hero_aura;
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .hero_aura = true;
    let (ui_on, ci_on) = build(world);
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .hero_aura = false;
    let (ui_off, ci_off) = build(world);
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .hero_aura = was;
    let only_diff = |a: &[u8], b: &[u8]| {
        let d: Vec<usize> = a
            .iter()
            .zip(b)
            .enumerate()
            .filter(|(_, (x, y))| x != y)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(d.len(), 1, "the glow flag moves exactly one byte");
        d[0]
    };
    (only_diff(&ui_on, &ui_off), only_diff(&ci_on, &ci_off))
}

/// **The hero glow survives mounting.** Java writes the glow byte as
/// `isHero() || (isGM() && GM_HERO_AURA)` in *both* `UserInfo` (the rider's
/// own client) and `CharInfo` (everyone else's), and `Player.mount` neither
/// touches the flag nor suppresses the byte while mounted — so a hero (or a
/// GM with `GMHeroAura=True`) keeps the glow on a wyvern and after landing.
/// Regression test for a field report of the glow vanishing on a wyvern: the
/// flag is on the wire in every state, mounted included.
#[test]
fn hero_glow_survives_mount_for_gm_and_hero() {
    let (mut world, ..) = admin_world();
    world.data.pet_data = crate::data::pet_data::PetData::load_from(DIST);
    world.data.npc_data = crate::data::npc_data::NpcData::load_from(DIST);
    world.data.gm.hero_aura = true;

    // The GM (glow from GMHeroAura) and a plain player crowned hero, plus an
    // onlooker whose client receives their CharInfo.
    let mut gm_rx = ingame_player_access(&mut world, 1, 8940, 100);
    let mut hero_rx = ingame_player_access(&mut world, 2, 8941, 0);
    let mut ob_rx = ingame_player_access(&mut world, 3, 8942, 0);
    crate::game_loop::admin::hero::set_hero(&mut world, 8941, true);
    assert!(
        world
            .objects
            .get_component::<Player>(&8941)
            .unwrap()
            .hero_aura,
        "a crowned hero glows without being a GM"
    );
    drain(&mut gm_rx);
    drain(&mut hero_rx);
    drain(&mut ob_rx);

    // The GM rides via `//ride_wyvern`; the hero has no admin rights, so they
    // take the script path the WyvernManager uses (Java `Player.mount`).
    for (is_gm, oid, own_rx) in [(true, 8940i32, &mut gm_rx), (false, 8941, &mut hero_rx)] {
        let (ui_off, ci_off) = glow_offsets(&mut world, oid);
        // Unmounted baseline.
        crate::game_loop::party::broadcast_user_info(&world, oid);
        let (ui, ci) = (
            drain(own_rx).into_iter().find(|p| p[0] == 0x32).unwrap(),
            drain(&mut ob_rx)
                .into_iter()
                .find(|p| p[0] == server_packets::opcodes::CHAR_INFO)
                .unwrap(),
        );
        assert_eq!((ui[ui_off], ci[ci_off]), (1, 1), "{oid}: glows on foot");

        // On the wyvern: the mount swaps speeds/collision, the glow stays.
        if is_gm {
            on_packet(&mut world, 1, build_admin("ride_wyvern"));
        } else {
            crate::game_loop::admin::mounts::mount_player(&mut world, oid, 12621, 2);
        }
        assert!(
            world
                .objects
                .get_component::<Player>(&oid)
                .unwrap()
                .is_flying(),
            "{oid}: on the wyvern"
        );
        let (ui, ci) = (
            drain(own_rx)
                .into_iter()
                .rev()
                .find(|p| p[0] == 0x32)
                .unwrap(),
            drain(&mut ob_rx)
                .into_iter()
                .rev()
                .find(|p| p[0] == server_packets::opcodes::CHAR_INFO)
                .unwrap(),
        );
        assert_eq!(
            (ui[ui_off], ci[ci_off]),
            (1, 1),
            "{oid}: the glow byte is still set while mounted"
        );

        // And after landing.
        if is_gm {
            on_packet(&mut world, 1, build_admin("unride"));
        } else {
            crate::game_loop::admin::mounts::dismount(&mut world, oid);
        }
        assert_eq!(
            world
                .objects
                .get_component::<Player>(&oid)
                .unwrap()
                .mount_type,
            0,
            "{oid}: dismounted"
        );
        let (ui, ci) = (
            drain(own_rx)
                .into_iter()
                .rev()
                .find(|p| p[0] == 0x32)
                .unwrap(),
            drain(&mut ob_rx)
                .into_iter()
                .rev()
                .find(|p| p[0] == server_packets::opcodes::CHAR_INFO)
                .unwrap(),
        );
        assert_eq!(
            (ui[ui_off], ci[ci_off]),
            (1, 1),
            "{oid}: the glow byte survives the dismount"
        );
    }
}

/// **`//settruehero` drives its own packet byte, separate from `//sethero`.**
/// Java's `AdminAdmin` has two distinct commands: `sethero` flips `isHero()`
/// (skill tree + the SOCIAL glow byte) while `settruehero` flips `isTrueHero()`
/// — a second flag written as `100 : 0` at the tail of both `CharInfo` and
/// `UserInfo`. The port had aliased the two commands and hard-coded the true
/// hero byte to 0, so the flag could never reach the client.
#[test]
fn settruehero_is_a_separate_flag_from_sethero() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8950, 100);
    let mut ob_rx = ingame_player_access(&mut world, 2, 8951, 0);
    drain(&mut gm_rx);
    drain(&mut ob_rx);
    // Java's `settruehero` needs a target (INVALID_TARGET otherwise); self.
    world
        .objects
        .add_components(&8950, crate::model::components::TargetRef(Some(8950)));

    let true_hero_byte = |pk: &[u8]| pk[pk.len() - 3]; // …trueHero, hairAccessory, abilityPoints
    crate::game_loop::party::broadcast_user_info(&world, 8950);
    let before = drain(&mut ob_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::CHAR_INFO)
        .unwrap();
    assert_eq!(true_hero_byte(&before), 0, "off by default, as in Java");

    on_packet(&mut world, 1, build_admin("settruehero"));
    {
        let p = world.objects.get_component::<Player>(&8950).unwrap();
        assert!(p.true_hero, "the flag flipped");
        assert!(!p.is_hero, "and it did NOT touch isHero()");
    }
    let after = drain(&mut ob_rx)
        .into_iter()
        .rev()
        .find(|p| p[0] == server_packets::opcodes::CHAR_INFO)
        .unwrap();
    assert_eq!(true_hero_byte(&after), 100, "Java writes 100, not 1");

    // Toggling back clears it (Java `setTrueHero(!isTrueHero())`).
    on_packet(&mut world, 1, build_admin("settruehero"));
    assert!(
        !world
            .objects
            .get_component::<Player>(&8950)
            .unwrap()
            .true_hero
    );
}

/// **`CharInfo` carries the states an onlooker can only learn from it.** Java
/// fills sitting / in-combat / dead / pvp-flag / noble / cursed-weapon /
/// clan-crest / clan-reputation from live state inside the packet ctor; the
/// port had them hard-coded to 0, so a player who walked into view of someone
/// sitting, flagged or dead saw them standing, clean and alive.
#[test]
fn char_info_reflects_live_player_state() {
    let (mut world, ..) = admin_world();
    let _gm = ingame_player_access(&mut world, 1, 8960, 100);
    let mut ob_rx = ingame_player_access(&mut world, 2, 8961, 0);
    drain(&mut ob_rx);

    let snapshot = |world: &World, rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>| {
        crate::game_loop::party::broadcast_user_info(world, 8960);
        drain(rx)
            .into_iter()
            .rev()
            .find(|p| p[0] == server_packets::opcodes::CHAR_INFO)
            .expect("CharInfo")
    };
    let base = snapshot(&world, &mut ob_rx);

    // Sit down + flag for PvP: both bytes must move.
    {
        let p = world.objects.get_component_mut::<Player>(&8960).unwrap();
        p.sitting = true;
        p.is_noble = true;
    }
    world.objects.add_components(
        &8960,
        crate::model::components::PvpState {
            flag: 1,
            ..Default::default()
        },
    );
    let sitting = snapshot(&world, &mut ob_rx);
    let moved: Vec<usize> = base
        .iter()
        .zip(&sitting)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        moved.len(),
        3,
        "exactly the sitting, pvp-flag and noble bytes changed"
    );
    assert!(
        moved.iter().all(|&i| sitting[i] == 1 || base[i] == 1),
        "each flipped between 0 and 1"
    );

    // Dying flips the alike-dead byte (Java `isAlikeDead`).
    world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&8960)
        .unwrap()
        .dead = true;
    let dead = snapshot(&world, &mut ob_rx);
    let dead_moved: Vec<usize> = sitting
        .iter()
        .zip(&dead)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(dead_moved.len(), 1, "only the alike-dead byte changed");
    assert_eq!(dead[dead_moved[0]], 1);
}

/// **A model swap must not eat the character's visual effects.** Field report:
/// a hidden GM who mounts a strider and dismounts is still invisible, but the
/// STEALTH glow is gone from their own view — and Java does the same, because
/// `Player.dismount()` sends `Ride` + `broadcastUserInfo()` and never calls
/// `updateAbnormalVisualEffects`. The client rebuilds the actor around the new
/// model and starts it with no visuals, so the list has to be *re-sent, and
/// late* — Java schedules its own refresh 50 ms out rather than inline.
#[test]
fn mounting_and_dismounting_resend_the_visual_effects() {
    use crate::model::skill::STEALTH_CLIENT_ID;
    let (mut world, ..) = admin_world();
    world.data.pet_data = crate::data::pet_data::PetData::load_from(DIST);
    world.data.npc_data = crate::data::npc_data::NpcData::load_from(DIST);
    let mut gm_rx = ingame_player_access(&mut world, 1, 8980, 100);

    // Hide: the GM is invisible and their client knows to draw STEALTH.
    on_packet(&mut world, 1, build_admin("hide"));
    drain(&mut gm_rx);

    let stealth_packet = |pkts: &[Vec<u8>]| {
        pkts.iter().any(|p| {
            p[0] == server_packets::opcodes::EX
                && i16::from_le_bytes(p[1..3].try_into().unwrap())
                    == server_packets::opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT
                && p.windows(2).any(|w| w == STEALTH_CLIENT_ID.to_le_bytes())
        })
    };

    for step in ["ride_strider", "unride"] {
        on_packet(&mut world, 1, build_admin(step));
        // Nothing in the same batch as the model swap — that packet would be
        // applied to the actor the client is tearing down.
        let immediate = drain(&mut gm_rx);
        assert!(
            !stealth_packet(&immediate),
            "{step}: the visual list must not ride along with the swap"
        );
        // One tick later (Java's 50 ms) it arrives.
        advance_ticks(&mut world, 1);
        assert!(
            stealth_packet(&drain(&mut gm_rx)),
            "{step}: the visual list is re-sent after the actor is rebuilt"
        );
        assert!(
            world
                .objects
                .get_component::<crate::model::components::AdminFlags>(&8980)
                .is_some_and(|f| f.hidden),
            "{step}: still invisible throughout"
        );
    }
}

/// `AllowRideMountsDuringSiege = False` (this dist) has three consumers, two of
/// which are reachable here: `Player.mount` refuses inside a live siege zone,
/// and `SiegeZone.onEnter` dismounts a rider who walks in. Both are silent.
#[test]
fn a_siege_zone_refuses_and_strips_mounts() {
    use crate::model::components::ZoneFlags;

    let (mut world, _tx, _db, _l) = test_world();
    world.cfg.feature.allow_ride_mounts_during_siege = false;
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // Give the strider a template so the mount's collision swap resolves.
    let mut t = crate::data::npc_data::default_template(12526);
    t.type_name = "Npc".into();
    t.level = 55;
    world.data.npc_data.insert_for_test(t);

    // Inside a live siege zone the mount is simply refused.
    world
        .objects
        .get_component_mut::<ZoneFlags>(&3001)
        .unwrap()
        .in_active_siege = true;
    assert!(
        !crate::game_loop::admin::mounts::mount_player(&mut world, 3001, 12526, 1),
        "no mounting inside a siege"
    );
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_mounted(),
        "and nothing was applied"
    );

    // Outside it, the same mount works…
    world
        .objects
        .get_component_mut::<ZoneFlags>(&3001)
        .unwrap()
        .in_active_siege = false;
    assert!(crate::game_loop::admin::mounts::mount_player(
        &mut world, 3001, 12526, 1
    ));
    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_mounted()
    );

    // …until the siege catches up with the rider, which dismounts them
    // (`SiegeZone.onEnter`, reached from `refresh_siege_zone_flag`).
    crate::game_loop::zones::dismount_for_siege(&mut world, 3001);
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_mounted(),
        "SiegeZone.onEnter dismounts"
    );
}
