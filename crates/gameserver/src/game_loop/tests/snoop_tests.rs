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
