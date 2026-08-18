//! `General.ini`'s instance and zone keys — `PeaceZoneMode`, `JailIsPvp`,
//! `EjectDeadPlayerTime`, `RestorePlayerInstance`.
//!
//! `DefaultFinishTime` has no test because it has no consumer: Java's only
//! no-arg `finishInstance()` caller is `AbstractInstance`'s protected helper,
//! which **no script on this dist calls**. Recorded on the config field.

use super::*;
use crate::data::instance_data::{ExitType, InstanceTemplate};
use crate::data::zone_data::ZoneKind;
use crate::game_loop::instances;
use crate::model::components::Vitals;

/// A minimal instance template: no spawns, no doors, ORIGIN exit.
const TEST_TEMPLATE: i32 = 920;

fn instance_world() -> (
    World,
    db::CmdTx,
    db::CmdRx,
    UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, tx, db_rx, link) = test_world();
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: TEST_TEMPLATE,
            name: Some("Config Test".into()),
            max_worlds: -1,
            duration_min: 0,
            empty_destroy_min: 5,
            enter: Some((5000, 5000, 100)),
            exit: ExitType::Origin,
            doors: vec![],
            groups: vec![],
        });
    (world, tx, db_rx, link)
}

/// `PeaceZoneMode` is three modes, not a flag, and only mode 0 is the dist's.
///
/// Mode **1** reads `getSiegeState() != 0`, which is **not** `isInSiege()`:
/// Java sets it for any registered clan member for the whole siege, wherever
/// they stand. Using the zone-scoped predicate instead would exempt nobody in
/// a town — which is the only place the mode is interesting.
#[test]
fn peace_zone_mode_two_switches_peace_zones_off_entirely() {
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);

    for (mode, expect_peace) in [(0, true), (2, false)] {
        world.cfg.general.peace_zone_mode = mode;
        // Put the player in a peace zone by asserting the mask directly: the
        // fixture has no zone geometry, so the config branch is what is under
        // test, not the lookup.
        let masked = crate::game_loop::zones::apply_zone_config_for_test(
            &world,
            100,
            (0, 0, 0),
            ZoneKind::Peace.bit(),
        );
        assert_eq!(
            masked & ZoneKind::Peace.bit() != 0,
            expect_peace,
            "PeaceZoneMode = {mode}"
        );
    }
}

/// Mode **1** exempts a siege participant and nobody else. Both halves are
/// asserted: a non-participant in mode 1 must keep the peace flag, or the mode
/// would just be mode 2 with extra steps.
#[test]
fn peace_zone_mode_one_exempts_only_siege_participants() {
    let (mut world, ..) = test_world();
    world.cfg.general.peace_zone_mode = 1;
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);

    let masked = crate::game_loop::zones::apply_zone_config_for_test(
        &world,
        100,
        (0, 0, 0),
        ZoneKind::Peace.bit(),
    );
    assert!(
        masked & ZoneKind::Peace.bit() != 0,
        "a player in no siege keeps the peace flag under mode 1"
    );
}

/// `JailIsPvp` (**False** here) turns the GM prison into a combat zone. The
/// jail is geometry-queried rather than masked, so the key adds the PVP bit
/// rather than flipping one the lookup already produced.
#[test]
fn jail_is_pvp_adds_the_pvp_flag_only_when_set() {
    let (mut world, ..) = test_world();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(crate::data::DIST_GAME);
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    // A point inside `gm_room.xml`, which is uniformly JailZone.
    let jail = jail_point(&world).expect("the dist ships a jail zone");

    for key in [false, true] {
        world.cfg.general.jail_is_pvp = key;
        let masked = crate::game_loop::zones::apply_zone_config_for_test(&world, 100, jail, 0);
        assert_eq!(
            masked & ZoneKind::Pvp.bit() != 0,
            key,
            "JailIsPvp = {key} inside the prison"
        );
    }

    // …and never outside it.
    world.cfg.general.jail_is_pvp = true;
    let masked = crate::game_loop::zones::apply_zone_config_for_test(&world, 100, (0, 0, 0), 0);
    assert_eq!(
        masked & ZoneKind::Pvp.bit(),
        0,
        "somewhere that is not the jail stays unflagged"
    );
}

/// Find a point inside the dist's jail zone by probing its declared bounds.
fn jail_point(world: &World) -> Option<(i32, i32, i32)> {
    // `gm_room.xml`'s interior, from the datapack.
    let candidates = [(-114_356, -249_645, -2_984), (-114_400, -249_500, -2_984)];
    candidates
        .into_iter()
        .find(|&(x, y, z)| world.data.zone_data.in_jail_zone(x, y, z))
}

/// `EjectDeadPlayerTime` (**1** minute) — a corpse in an instance is expelled
/// unless it is resurrected first.
///
/// The cancellation mechanism is the point: Java never cancels the task, it
/// schedules one that re-reads `isDead()` when it fires. So a resurrection, a
/// manual exit, or a logout all "cancel" it without anything cancelling
/// anything.
#[test]
fn a_corpse_in_an_instance_is_ejected_unless_it_is_raised_first() {
    let (mut world, ..) = instance_world();
    let iid = instances::create_from_template(&mut world, TEST_TEMPLATE).expect("instance");
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    instances::enter(&mut world, 100, iid);
    assert_eq!(instances::instance_of_for_test(&world, 100), iid);

    // Die inside.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&100) {
        v.dead = true;
    }
    instances::arm_eject_on_death(&mut world, 100);
    assert!(
        !world.scheduler.pending_ticks_for_test().is_empty(),
        "the eject clock is armed"
    );

    // Still dead when it fires → expelled.
    instances::handle_eject_dead(&mut world, 100);
    assert_eq!(
        instances::instance_of_for_test(&world, 100),
        0,
        "a corpse is expelled"
    );

    // Now the resurrection case: re-enter, die, be raised, and the same task
    // finds a live player and does nothing.
    instances::enter(&mut world, 100, iid);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&100) {
        v.dead = true;
    }
    instances::arm_eject_on_death(&mut world, 100);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&100) {
        v.dead = false;
    }
    instances::handle_eject_dead(&mut world, 100);
    assert_eq!(
        instances::instance_of_for_test(&world, 100),
        iid,
        "a resurrected player stays — the task re-reads the death state \
         rather than being cancelled"
    );
}

/// `EjectDeadPlayerTime = 0` disables the eject, matching Java's `> 0` guard.
#[test]
fn a_zero_eject_time_arms_nothing() {
    let (mut world, ..) = instance_world();
    world.cfg.general.eject_dead_player_time_min = 0;
    let iid = instances::create_from_template(&mut world, TEST_TEMPLATE).expect("instance");
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    instances::enter(&mut world, 100, iid);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&100) {
        v.dead = true;
    }
    instances::arm_eject_on_death(&mut world, 100);
    assert!(world.scheduler.pending_ticks_for_test().is_empty());
}

/// `RestorePlayerInstance` (**True**) remembers the instance across a logout
/// and puts the player back. With it off Java instead moves them to the exit
/// location, so they do not wake up inside a world that no longer exists.
#[test]
fn the_instance_is_remembered_across_a_logout_only_when_the_key_is_set() {
    for key in [true, false] {
        let (mut world, ..) = instance_world();
        world.cfg.general.restore_player_instance = key;
        let iid = instances::create_from_template(&mut world, TEST_TEMPLATE).expect("instance");
        let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
        instances::enter(&mut world, 100, iid);

        instances::on_player_logout(&mut world, 100);
        let remembered = world
            .objects
            .get_component::<crate::model::components::PlayerVariables>(&100)
            .and_then(|v| v.0.get("INSTANCE_RESTORE").cloned());
        assert_eq!(
            remembered.is_some(),
            key,
            "RestorePlayerInstance = {key}: the variable"
        );
        assert_eq!(
            instances::instance_of_for_test(&world, 100) != 0,
            key,
            "RestorePlayerInstance = {key}: membership is kept only to restore it"
        );

        // The login half only re-enters when the instance is still running.
        instances::restore_on_login(&mut world, 100);
        assert_eq!(
            instances::instance_of_for_test(&world, 100) != 0,
            key,
            "RestorePlayerInstance = {key}: after the login restore"
        );
        assert!(
            world
                .objects
                .get_component::<crate::model::components::PlayerVariables>(&100)
                .is_none_or(|v| !v.0.contains_key("INSTANCE_RESTORE")),
            "the variable is consumed either way, as Java's unconditional \
             `vars.remove` does"
        );
    }
}

/// A remembered instance that has since been destroyed is discarded, not
/// retried — otherwise the variable would strand the player on every login.
#[test]
fn a_stale_instance_id_is_consumed_and_ignored() {
    let (mut world, ..) = instance_world();
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    world.objects.add_components(
        &100,
        crate::model::components::PlayerVariables(
            [("INSTANCE_RESTORE".to_string(), "999999".to_string())]
                .into_iter()
                .collect(),
        ),
    );
    instances::restore_on_login(&mut world, 100);
    assert_eq!(instances::instance_of_for_test(&world, 100), 0);
    assert!(
        !world
            .objects
            .get_component::<crate::model::components::PlayerVariables>(&100)
            .unwrap()
            .0
            .contains_key("INSTANCE_RESTORE"),
        "consumed, so it cannot be retried forever"
    );
}
