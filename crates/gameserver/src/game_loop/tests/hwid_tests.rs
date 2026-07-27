//! HWID subsystem (G31 slice 5): the `RequestHardWareInfo` parse, HWID
//! punishment matching, and the post-enter-world re-check.

use super::*;

use crate::game_loop::punishment;
use crate::model::punishment::{PunishmentAffect, PunishmentType};
use crate::network::client_packets::HardwareInfo;

/// A full 19-field `RequestHardWareInfo` body.
fn hwinfo_body(mac: &str, cpu: &str, vga: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(mac);
    for v in [6i32, 10, 0, 22000] {
        w.write_i32(v); // platform, major, minor, build
    }
    w.write_i32(9); // directxVersion
    w.write_i32(1); // directxRevision
    w.write_string(cpu);
    w.write_i32(3600); // cpuSpeed
    w.write_i32(8); // cpuCoreCount
    for v in [1i32, 0, 0, 0, 0, 8192, 0] {
        w.write_i32(v); // vgaCount, vgaPcxSpeed, mem1-3, videoMemory, vgaVersion
    }
    w.write_string(vga);
    w.write_string("31.0.15");
    w.into_bytes()
}

/// Register a hwid directly on a client (the ex-packet path's effect).
fn set_hwid(world: &mut World, client_id: u32, mac: &str) {
    world.hwids.insert(
        client_id,
        HardwareInfo {
            mac_address: mac.to_string(),
            ..Default::default()
        },
    );
}

#[test]
fn hardware_info_parses_every_kept_field() {
    let body = hwinfo_body("AA:BB:CC:DD:EE:FF", "Ryzen 9", "RTX 4090");
    let hw = HardwareInfo::read(&body).expect("parses");
    assert_eq!(hw.mac_address, "AA:BB:CC:DD:EE:FF");
    assert_eq!(hw.windows_major_version, 10);
    assert_eq!(hw.windows_build_number, 22000);
    assert_eq!(hw.cpu_name, "Ryzen 9");
    assert_eq!(hw.cpu_speed, 3600);
    assert_eq!(hw.cpu_core_count, 8);
    assert_eq!(hw.vga_name, "RTX 4090");
    assert_eq!(hw.vga_driver_version, "31.0.15");
}

#[test]
fn a_hwid_ban_disconnects_the_matching_client() {
    let (mut world, _tx, _rx, _link) = test_world();
    let _victim = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_hwid(&mut world, 2, "MAC-1");

    // Banning that HWID drives Java `BanHandler.onStart` over the matching player.
    punishment::start_punishment(
        &mut world,
        "MAC-1".to_string(),
        PunishmentAffect::Hwid,
        PunishmentType::Ban,
        0,
        "r".into(),
        "gm".into(),
    );
    assert!(
        world.clients.get(&2).is_none(),
        "the HWID-banned session was dropped"
    );
    // A different fingerprint is untouched.
    assert!(punishment::is_banned(
        &world,
        9,
        "acc",
        "1.2.3.4",
        Some("MAC-1")
    ));
    assert!(!punishment::is_banned(
        &world,
        9,
        "acc",
        "1.2.3.4",
        Some("MAC-2")
    ));
}

#[test]
fn on_hwid_received_kicks_a_fingerprint_banned_after_login() {
    let (mut world, _tx, _rx, _link) = test_world();
    let _p = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    // The ban exists first; the fingerprint arrives later (client-driven timing).
    punishment::start_punishment(
        &mut world,
        "MAC-9".to_string(),
        PunishmentAffect::Hwid,
        PunishmentType::Ban,
        0,
        "r".into(),
        "gm".into(),
    );
    assert!(
        world.clients.get(&2).is_some(),
        "still online (no hwid yet)"
    );

    set_hwid(&mut world, 2, "MAC-9");
    punishment::on_hwid_received(&mut world, 2);
    assert!(
        world.clients.get(&2).is_none(),
        "kicked once the fingerprint matched"
    );
}
