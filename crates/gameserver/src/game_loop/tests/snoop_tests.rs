//! GM chat snoop (`//snoop`, G31 slice 5).

use super::*;

use crate::model::Player;

const SNOOP_OPCODE: u8 = 0xDB;

#[test]
fn snoop_mirrors_the_targets_chat_to_the_gm() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 3001, 100);
    let _victim = ingame_player(&mut world, 2, 3002, 0, 0, 0);

    on_packet(&mut world, 1, build_admin("snoop P3002"));
    assert!(
        world
            .objects
            .get_component::<Player>(&3002)
            .unwrap()
            .snoop_listeners
            .contains(&3001),
        "GM registered as a listener"
    );
    drain(&mut gm_rx);

    // The snooped player speaks → the GM gets a Snoop line.
    on_packet(
        &mut world,
        2,
        [vec![cop::SAY2], say2_body("secret plans", 0, None)].concat(),
    );
    assert!(
        has_opcode(&drain(&mut gm_rx), SNOOP_OPCODE),
        "GM sees the snooped chat"
    );
}

#[test]
fn a_non_snooped_players_chat_reaches_no_snooper() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 3001, 100);
    let _a = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let _b = ingame_player(&mut world, 3, 3003, 0, 0, 0);

    on_packet(&mut world, 1, build_admin("snoop P3002"));
    drain(&mut gm_rx);

    // 3003 is not snooped — its chat produces no Snoop packet.
    on_packet(
        &mut world,
        3,
        [vec![cop::SAY2], say2_body("hello", 0, None)].concat(),
    );
    assert!(
        !has_opcode(&drain(&mut gm_rx), SNOOP_OPCODE),
        "only the snooped player is mirrored"
    );
}

/// **`SnoopQuit` (0xB4) takes both halves of the link apart** — the GM stops
/// receiving lines *and* stops being recorded as snooping.
#[test]
fn snoop_quit_unlinks_both_sides() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 3001, 100);
    let _victim = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    on_packet(&mut world, 1, build_admin("snoop P3002"));
    drain(&mut gm_rx);

    let mut body = vec![cop::SNOOP_QUIT];
    body.extend_from_slice(&3002i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert!(
        world
            .objects
            .get_component::<Player>(&3002)
            .unwrap()
            .snoop_listeners
            .is_empty(),
        "the target no longer lists the GM"
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .snooped
            .is_empty(),
        "and the GM no longer lists the target"
    );

    // The chat that used to be mirrored now is not.
    on_packet(
        &mut world,
        2,
        [vec![cop::SAY2], say2_body("still secret", 0, None)].concat(),
    );
    assert!(
        !has_opcode(&drain(&mut gm_rx), SNOOP_OPCODE),
        "no more snoop lines"
    );
}

/// **A quit naming a player who is gone changes nothing.** Java looks the id
/// up in the world first and returns on a miss, so the GM's own entry survives
/// — asserted so the guard is not "simplified" into a blind retain.
#[test]
fn snoop_quit_for_an_absent_player_is_ignored() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 3001, 100);
    let _victim = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    on_packet(&mut world, 1, build_admin("snoop P3002"));
    drain(&mut gm_rx);

    let mut body = vec![cop::SNOOP_QUIT];
    body.extend_from_slice(&999_999i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .snooped,
        vec![3002],
        "the live link is untouched"
    );
}
