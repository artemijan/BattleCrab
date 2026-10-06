//! The monitor channel's `clients` answer (`docs/MONITORING.md` §10): one
//! record per session, staged like the `sessions_*` gauges.

use super::*;

use crate::game_loop::net::client_records;
use crate::network::client_packets::session::HardwareInfo;

#[test]
fn every_session_is_listed_with_its_stage_and_who_it_is() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _connecting = connect(&mut world, 1);

    let _lobby = connect(&mut world, 2);
    let ClientSession::Connecting(s) = world.clients.remove(&2).unwrap() else {
        unreachable!()
    };
    let s = s
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![dummy_char(555, "Hero")]);
    world.clients.insert(2, ClientSession::InLobby(s));

    let _in_game = ingame_player(&mut world, 3, 3003, 10, 20, 30);
    world.hwids.insert(
        3,
        HardwareInfo {
            mac_address: "AA:BB".into(),
            cpu_name: "Ryzen".into(),
            ..Default::default()
        },
    );
    world.protocol_versions.insert(3, 746);

    let records = client_records(&world);
    let summary: Vec<(u64, &str, Option<&str>, Option<&str>)> = records
        .iter()
        .map(|r| {
            (
                r.id,
                r.stage.as_str(),
                r.account.as_deref(),
                r.character.as_deref(),
            )
        })
        .collect();
    assert_eq!(summary[0], (1, "authenticating", None, None));
    assert_eq!(summary[1], (2, "lobby", Some("bob"), None));
    assert_eq!((summary[2].0, summary[2].1), (3, "in_game"));
    assert_eq!(summary[2].3, Some("P3003"));

    let lobby = &records[1].details["lobbyCharacters"];
    assert_eq!(lobby[0]["name"], "Hero");

    let in_game = &records[2];
    assert_eq!(in_game.hwid.as_deref(), Some("AA:BB"));
    assert_eq!(in_game.details["hardware"]["cpu"], "Ryzen");
    assert_eq!(in_game.details["protocolVersion"], 746);
    let character = &in_game.details["character"];
    assert_eq!(character["objectId"], 3003);
    assert_eq!(character["position"]["x"], 10);
    assert!(in_game.connected_ms > 0);
}
