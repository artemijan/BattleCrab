//! The PvP flag — when it starts, blinks and expires — and who is
//! auto-attackable.

use super::*;

/// PvP flag lifecycle (`Player.updatePvPStatus` + `PvpFlagTaskManager`): a
/// hostile action flags the player solid (1), the 1 s sweep blinks it (2) in
/// the final 20 s, then clears it (0) past expiry.
#[test]
fn pvp_flag_starts_blinks_and_expires() {
    use crate::game_loop::combat::pvp;
    use model::components::combat::PvpState;
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let start = world.tick;

    pvp::update_pvp_status(&mut world, 5001);
    let st = *world.objects.get_component::<PvpState>(&5001).unwrap();
    assert_eq!(st.flag, 1, "flagged solid");
    assert_eq!(
        st.expires_tick,
        start + 1200,
        "PVP_NORMAL_TIME = 120 s @ 100 ms ticks"
    );

    // Mid-life (before the last 20 s) stays solid.
    world.tick = start + 900;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(
        world.objects.get_component::<PvpState>(&5001).unwrap().flag,
        1
    );

    // Final 20 s (200 ticks) → blinking (2).
    world.tick = start + 1100;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(
        world.objects.get_component::<PvpState>(&5001).unwrap().flag,
        2,
        "blinks in the last 20 s"
    );

    // Past expiry → cleared.
    world.tick = start + 1200;
    pvp::pvp_flag_tick(&mut world);
    assert_eq!(
        world.objects.get_component::<PvpState>(&5001).unwrap().flag,
        0,
        "cleared past expiry"
    );
}

/// `updatePvPStatus(target)`: attacking a clean player flags for
/// `PVP_NORMAL_TIME`; attacking an already-flagged/PK player flags for the
/// shorter `PVP_PVP_TIME` (`checkIfPvP`). Attacking a PK doesn't flag at all.
#[test]
fn pvp_flag_duration_depends_on_target_state() {
    use crate::game_loop::combat::pvp;
    use model::components::combat::PvpState;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 50, 0, 0);
    let start = world.tick;

    // A attacks a clean B → 120 s.
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(
        world
            .objects
            .get_component::<PvpState>(&5001)
            .unwrap()
            .expires_tick,
        start + 1200
    );

    // B (clean) attacks the now-flagged A → 60 s (checkIfPvP true).
    world.tick = start + 10;
    pvp::update_pvp_status_target(&mut world, 5002, 5001);
    assert_eq!(
        world
            .objects
            .get_component::<PvpState>(&5002)
            .unwrap()
            .expires_tick,
        start + 10 + 600,
        "PVP time vs a flagged target"
    );

    // Attacking a PK doesn't flag the attacker (target freely attackable).
    world
        .objects
        .get_component_mut::<Player>(&5002)
        .unwrap()
        .reputation = -1;
    world
        .objects
        .get_component_mut::<PvpState>(&5001)
        .unwrap()
        .flag = 0;
    world
        .objects
        .get_component_mut::<PvpState>(&5001)
        .unwrap()
        .expires_tick = 0;
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(
        world.objects.get_component::<PvpState>(&5001).unwrap().flag,
        0,
        "no flag for attacking a PK"
    );
}

/// `isAutoAttackable` relation for players: a clean player needs Ctrl (not
/// auto-attackable), a flagged or PK one does not.
#[test]
fn flagged_or_pk_player_is_auto_attackable() {
    use crate::game_loop::combat::pvp;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 50, 0, 0);

    assert!(
        !pvp::is_player_auto_attackable(&world, 5001, 5002),
        "clean player needs force"
    );

    pvp::update_pvp_status(&mut world, 5002);
    assert!(
        pvp::is_player_auto_attackable(&world, 5001, 5002),
        "flagged player is attackable"
    );

    world
        .objects
        .get_component_mut::<model::components::combat::PvpState>(&5002)
        .unwrap()
        .flag = 0;
    world
        .objects
        .get_component_mut::<Player>(&5002)
        .unwrap()
        .reputation = -1;
    assert!(
        pvp::is_player_auto_attackable(&world, 5001, 5002),
        "PK is attackable"
    );
}

/// Arena (`ArenaZone`/`ZoneId.PVP`): both players in a PVP zone are freely
/// auto-attackable, and hostile actions there don't raise a flag.
#[test]
fn arena_players_attackable_without_flagging() {
    use crate::game_loop::combat::pvp;
    use model::components::combat::PvpState;
    use model::components::space::ZoneFlags;
    let (mut world, ..) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 0, 0);
    let pvp_bit = crate::data::zone_data::ZoneKind::Pvp.bit();
    world
        .objects
        .get_component_mut::<ZoneFlags>(&5001)
        .unwrap()
        .mask = pvp_bit;
    world
        .objects
        .get_component_mut::<ZoneFlags>(&5002)
        .unwrap()
        .mask = pvp_bit;

    // Freely attackable (no Ctrl) while both are in the arena.
    assert!(pvp::is_player_auto_attackable(&world, 5001, 5002));
    // Attacking there does not flag the attacker.
    pvp::update_pvp_status_target(&mut world, 5001, 5002);
    assert_eq!(
        world.objects.get_component::<PvpState>(&5001).unwrap().flag,
        0,
        "no flag inside an arena"
    );
}
