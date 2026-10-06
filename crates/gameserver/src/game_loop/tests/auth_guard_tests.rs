//! The authentication deadline (`game_loop::net::auth_guard`): a connection
//! that has not authenticated in time is dropped, and an address that keeps
//! doing it lands on the IP ban list.

use super::*;

use crate::db::DbCommand;
use crate::game_loop::net::auth_guard::{arm, strike};
use crate::world::WaitingClient;

/// A test world with the shipped `Security.ini` deadline switched on.
fn guarded_world() -> (
    World,
    db::CmdTx,
    db::CmdRx,
    UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db_tx, db_rx, link_rx) = test_world();
    world.auth_guard.cfg = crate::config::SecurityConfig::default();
    (world, db_tx, db_rx, link_rx)
}

/// Connects client `id` the way `NetEvent::Connected` does: a session, and
/// its deadline armed.
fn connect_guarded(world: &mut World, id: u32) -> UnboundedReceiver<bytes::Bytes> {
    let out = connect(world, id);
    arm(world, id);
    out
}

fn ban_commands(db_rx: &mut db::CmdRx) -> Vec<(String, i64, String)> {
    let mut out = Vec::new();
    while let Ok(cmd) = db_rx.try_recv() {
        if let DbCommand::BanIp {
            ip,
            expires_at,
            reason,
        } = cmd
        {
            out.push((ip, expires_at, reason));
        }
    }
    out
}

#[test]
fn a_silent_connection_is_dropped_at_the_deadline_and_not_before() {
    let (mut world, _db_tx, _db_rx, _link_rx) = guarded_world();
    let _silent = connect_guarded(&mut world, 1);

    advance_ticks(&mut world, 49);
    assert!(world.clients.contains_key(&1), "4.9 s in: still connected");
    advance_ticks(&mut world, 1);
    assert!(!world.clients.contains_key(&1), "5 s in: dropped");
    let ip: std::net::IpAddr = "127.0.0.1".parse().unwrap();
    assert_eq!(world.auth_guard.strikes[&ip].len(), 1);
}

#[test]
fn an_authenticated_client_is_left_alone() {
    let (mut world, _db_tx, _db_rx, _link_rx) = guarded_world();
    let _lobby = connect_guarded(&mut world, 1);
    let ClientSession::Connecting(s) = world.clients.remove(&1).unwrap() else {
        unreachable!()
    };
    let s = s
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![]);
    world.clients.insert(1, ClientSession::InLobby(s));

    advance_ticks(&mut world, 60);
    assert!(world.clients.contains_key(&1));
    assert!(world.auth_guard.strikes.is_empty());
}

#[test]
fn waiting_on_the_login_server_drops_without_a_strike() {
    let (mut world, _db_tx, _db_rx, _link_rx) = guarded_world();
    let _waiting = connect_guarded(&mut world, 1);
    world.login.waiting.insert(
        "bob".into(),
        WaitingClient {
            client_id: 1,
            session_key: SessionKey::new(1, 2, 3, 4),
        },
    );

    advance_ticks(&mut world, 50);
    assert!(!world.clients.contains_key(&1), "still dropped");
    assert!(
        world.auth_guard.strikes.is_empty(),
        "a slow login server is not the client's fault"
    );
}

#[test]
fn the_fifth_strike_in_the_window_bans_the_address() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = guarded_world();
    let ip: std::net::IpAddr = "203.0.113.9".parse().unwrap();
    let t0 = 1_759_000_000_000;

    // One strike from long ago has aged out of the 10-minute window.
    strike(&mut world, ip, t0 - 11 * 60_000);
    for i in 0..4 {
        strike(&mut world, ip, t0 + i);
    }
    assert!(ban_commands(&mut db_rx).is_empty(), "four in the window");

    strike(&mut world, ip, t0 + 4);
    let bans = ban_commands(&mut db_rx);
    assert_eq!(bans.len(), 1);
    let (banned, expires_at, reason) = &bans[0];
    assert_eq!(banned, "203.0.113.9");
    assert_eq!(*expires_at, 0, "permanent by default");
    assert!(reason.starts_with("automatic: 5 connections"), "{reason}");
    assert!(
        !world.auth_guard.strikes.contains_key(&ip),
        "the count starts over"
    );
}

#[test]
fn a_timed_ban_and_the_switches() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = guarded_world();
    let ip: std::net::IpAddr = "203.0.113.9".parse().unwrap();
    world.auth_guard.cfg.unauthenticated_strikes_before_ban = 1;
    world.auth_guard.cfg.unauthenticated_ban_minutes = 60;
    strike(&mut world, ip, 1_000);
    assert_eq!(ban_commands(&mut db_rx)[0].1, 1_000 + 3_600_000);

    // 0 strikes: drop, never ban.
    world.auth_guard.cfg.unauthenticated_strikes_before_ban = 0;
    for t in 0..10 {
        strike(&mut world, ip, t);
    }
    assert!(ban_commands(&mut db_rx).is_empty());

    // 0 timeout: no deadline at all.
    world.auth_guard.cfg.unauthenticated_timeout_ms = 0;
    let _c = connect_guarded(&mut world, 9);
    advance_ticks(&mut world, 100);
    assert!(world.clients.contains_key(&9));
}
