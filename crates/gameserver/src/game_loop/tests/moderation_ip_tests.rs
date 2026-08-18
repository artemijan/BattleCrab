//! G31 slice 4: the login-ban relay (Java `setAccountAccesslevel` →
//! `ChangeAccessLevel`) and the editchar IP tools (`find_ip`/`find_dualbox`/
//! `tracert`).

use super::*;

use crate::game_loop::admin::moderation::{characters_from_ip, dualbox_ips};
use crate::loginlink::LoginLinkCommand;

/// Drain the pending game→login commands.
fn drain_link(rx: &mut UnboundedReceiver<LoginLinkCommand>) -> Vec<LoginLinkCommand> {
    let mut out = Vec::new();
    while let Ok(c) = rx.try_recv() {
        out.push(c);
    }
    out
}

#[test]
fn login_ban_relays_access_level_and_kicks_the_account() {
    let (mut world, _db_tx, _db_rx, mut link_rx) = admin_world();
    // The session accounts are all "bob" (the test session fixture).
    let _gm = ingame_player_access(&mut world, 1, 7401, 100);
    let _victim = ingame_player_access(&mut world, 2, 7402, 0);
    drain_link(&mut link_rx);

    on_packet(&mut world, 1, build_admin("login_ban bob"));

    // A ChangeAccessLevel(-1) was relayed to the login server.
    let cmds = drain_link(&mut link_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            LoginLinkCommand::SetAccountAccessLevel { account, level }
                if account == "bob" && *level == -1
        )),
        "login ban relayed"
    );
    // The online character on that account was dropped.
    assert!(
        world.clients.get(&2).is_none(),
        "victim on the banned account disconnected"
    );
}

#[test]
fn login_unban_relays_a_zero_access_level() {
    let (mut world, _db_tx, _db_rx, mut link_rx) = admin_world();
    let _gm = ingame_player_access(&mut world, 1, 7401, 100);
    drain_link(&mut link_rx);

    on_packet(&mut world, 1, build_admin("login_unban bob"));

    assert!(drain_link(&mut link_rx).iter().any(|c| matches!(
        c,
        LoginLinkCommand::SetAccountAccessLevel { account, level }
            if account == "bob" && *level == 0
    )));
}

#[test]
fn find_ip_lists_only_matching_players() {
    // The test session fixture connects everyone from 127.0.0.1.
    let (mut world, ..) = admin_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);

    let mut hits = characters_from_ip(&world, "127.0.0.1");
    hits.sort_unstable();
    assert_eq!(hits, vec![3001, 3002], "both share the fixture IP");
    assert!(
        characters_from_ip(&world, "10.0.0.9").is_empty(),
        "a different IP matches nobody"
    );
}

#[test]
fn find_dualbox_reports_ips_at_or_above_the_threshold() {
    let (mut world, ..) = admin_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);

    // Two clients share 127.0.0.1 → a dualbox at threshold 2.
    assert_eq!(
        dualbox_ips(&world, 2),
        vec![("127.0.0.1".to_string(), 2)],
        "shared IP reported"
    );
    // Threshold 3 is above the pair → nothing.
    assert!(dualbox_ips(&world, 3).is_empty());
}

// --- AdminRepairChar (G33 tail) ---------------------------------------------

#[test]
fn repair_queues_a_db_command_only_for_an_offline_character() {
    use crate::db::DbCommand;
    let (mut world, _tx, mut db_rx, _link) = admin_world();
    let _gm = ingame_player_access(&mut world, 1, 7401, 100);
    drain_db(&mut db_rx);

    // Offline target → a RepairCharacter command is queued.
    on_packet(&mut world, 1, build_admin("repair Ghost"));
    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            DbCommand::RepairCharacter { char_name } if char_name == "Ghost"
        )),
        "offline repair queued"
    );

    // Online target → guarded, nothing queued (autosave would overwrite it).
    let _victim = ingame_player_access(&mut world, 2, 7402, 0); // name "P7402"
    drain_db(&mut db_rx);
    on_packet(&mut world, 1, build_admin("repair P7402"));
    assert!(
        !drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::RepairCharacter { .. })),
        "online character is not repaired"
    );
}
