//! Water: the swim speeds the *client* is told about, the drowning clock
//! (`WaterTask`), and `moveToLocation`'s water branch.

use super::*;
use crate::data::zone_data::ZoneKind;
use crate::model::components::{Speeds, Vitals, WaterTask};

/// A `Speeds` with distinct land/swim values, so a slot mix-up is visible.
fn swim_speeds() -> Speeds {
    Speeds {
        swamp_multiplier: 1.0,
        run_spd: 120.0,
        walk_spd: 80.0,
        swim_run_spd: 50.0,
        swim_walk_spd: 30.0,
        move_multiplier: 1.0,
        base_run_spd: 120.0,
        base_walk_spd: 80.0,
        base_swim_run_spd: 50.0,
        base_swim_walk_spd: 50.0,
        running: true,
        swimming: false,
    }
}

/// **The run-speed slot of `UserInfo`/`CharInfo` follows you into the water.**
/// Java fills it from `getRunSpeed()`, which returns `SWIM_RUN_SPEED` while
/// `isInsideZone(WATER)` — the client predicts movement and paces the run
/// animation off that slot, so sending the land speed there is what made
/// entering water feel like no slowdown at all.
#[test]
fn client_speed_fields_switch_to_swim_speeds_underwater() {
    let mut s = swim_speeds();
    assert_eq!(
        s.client_speed_fields(),
        [120, 80, 50, 30],
        "on land: run/walk slots are the ground speeds"
    );

    s.swimming = true;
    // `getMovementSpeedMultiplier` now divides by the *swim* base (50), and
    // 50/50 = 1.0, so the wire values are the raw swim speeds.
    assert!((s.client_move_multiplier() - 1.0).abs() < 1e-9);
    assert_eq!(
        s.client_speed_fields(),
        [50, 30, 50, 30],
        "underwater the run/walk slots carry the swim speeds (Java's \
         getRunSpeed()/getWalkSpeed() are water-aware); slots 3/4 are the raw \
         swim stats, which is why they duplicate 1/2 here"
    );
}

/// The animation-rate divisor is picked by mode, not fixed to the run base:
/// Java's `getMovementSpeedMultiplier` chooses among all four template bases.
#[test]
fn move_multiplier_uses_the_base_for_the_current_mode() {
    let mut s = swim_speeds();
    s.base_swim_run_spd = 100.0; // make the swim base differ from swim speed
    s.swimming = true;
    // move_speed = swim_run 50, base = swim_run base 100 → half cadence.
    assert!((s.client_move_multiplier() - 0.5).abs() < 1e-9);
    s.swimming = false;
    assert!((s.client_move_multiplier() - 1.0).abs() < 1e-9);
}

/// **Drowning.** `checkWaterState` arms the breath gauge on entering water;
/// nothing happens for 60 s, then 1% of max HP a second with the
/// "unable to breathe" message. Leaving blanks the gauge and cancels.
#[test]
fn breath_runs_out_then_drowning_damage_ticks_every_second() {
    let (mut world, ..) = cast_test_world();
    world.cfg.general.allow_water = true;
    insert_zone(&mut world, ZoneKind::Water, 5000, 6000, -500, 500);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&3001).unwrap();
        v.max_hp = 500;
        v.cur_hp = 500.0;
    }
    super::zones::revalidate_zone(&mut world, 3001, true);
    drain(&mut rx);
    assert!(
        !world.objects.has_component::<WaterTask>(&3001),
        "dry land: no drowning clock"
    );

    // Wade in.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 5500;
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(world.objects.has_component::<WaterTask>(&3001));
    let gauge = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == 0x6b)
        .expect("SetupGauge for the breath bar");
    // objectId, color, currentTime, maxTime.
    assert_eq!(
        i32::from_le_bytes(gauge[5..9].try_into().unwrap()),
        2,
        "cyan"
    );
    assert_eq!(
        i32::from_le_bytes(gauge[9..13].try_into().unwrap()),
        60_000,
        "Stat.BREATH's 60 s default"
    );

    // 59 s under: still holding our breath.
    for _ in 0..590 {
        world.tick += 1;
        super::water::drown_tick(&mut world);
    }
    assert_eq!(
        world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp,
        500.0,
        "no damage before the gauge empties"
    );

    // The 60 s mark, then two more seconds.
    for _ in 0..30 {
        world.tick += 1;
        super::water::drown_tick(&mut world);
    }
    let v = *world.objects.get_component::<Vitals>(&3001).unwrap();
    assert_eq!(v.cur_hp, 485.0, "3 beats × (500/100) HP");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::PlayerVitals>(&3001)
            .unwrap()
            .cur_cp,
        100.0,
        "`directlyToHp` — CP does not soak drowning damage, or a full CP bar \
         would let you hold your breath forever"
    );
    let pkts = drain(&mut rx);
    let msgs = sm_ids_of(&pkts);
    assert_eq!(
        msgs.iter()
            .filter(|&&id| id
                == server_packets::sm_ids::YOU_HAVE_TAKEN_S1_DAMAGE_BECAUSE_YOU_WERE_UNABLE_TO_BREATHE)
            .count(),
        3
    );
    // ...and *only* that line. `PlayerStatus.reduceHp`'s damage message sits
    // inside `attacker != getActiveChar()`, and drowning names the victim as
    // its own attacker — so "Bob has received 5 damage from Bob" must not
    // appear alongside it.
    assert!(
        !msgs.contains(&server_packets::sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2),
        "drowning is announced once, by its own message"
    );

    // Surface: the clock stops and the bar is blanked.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 0;
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(!world.objects.has_component::<WaterTask>(&3001));
    let gauge = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == 0x6b)
        .expect("SetupGauge blanking the breath bar");
    assert_eq!(i32::from_le_bytes(gauge[9..13].try_into().unwrap()), 0);
    let hp = world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp;
    for _ in 0..30 {
        world.tick += 1;
        super::water::drown_tick(&mut world);
    }
    assert_eq!(
        world.objects.get_component::<Vitals>(&3001).unwrap().cur_hp,
        hp,
        "no beats after surfacing"
    );
}

/// `Player.doDie` calls `stopWaterTask()` — a corpse on the seabed does not
/// keep drowning, and the breath bar leaves the screen with the death.
#[test]
fn death_stops_the_drowning_clock() {
    let (mut world, ..) = cast_test_world();
    world.cfg.general.allow_water = true;
    insert_zone(&mut world, ZoneKind::Water, -1000, 1000, -1000, 1000);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(world.objects.has_component::<WaterTask>(&3001));
    drain(&mut rx);

    super::death::player_do_die(&mut world, 3001, 3001);
    assert!(
        !world.objects.has_component::<WaterTask>(&3001),
        "the drowning clock is cancelled on death"
    );
    // And a dead player standing in water is not re-armed by a revalidate
    // (`startWaterTask`'s `!isDead()` guard).
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(!world.objects.has_component::<WaterTask>(&3001));
}

/// `AllowWater = False` leaves water *slow* but harmless: Java gates only
/// `checkWaterState()` on the flag, never `WaterZone.onEnter`'s speed switch.
#[test]
fn allow_water_off_still_slows_you_down() {
    let (mut world, ..) = cast_test_world();
    world.cfg.general.allow_water = false;
    insert_zone(&mut world, ZoneKind::Water, -1000, 1000, -1000, 1000);
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        let s = world.objects.get_component_mut::<Speeds>(&3001).unwrap();
        s.run_spd = 120.0;
        s.swim_run_spd = 50.0;
    }
    super::zones::revalidate_zone(&mut world, 3001, true);
    let speeds = *world.objects.get_component::<Speeds>(&3001).unwrap();
    assert!(speeds.swimming);
    assert_eq!(speeds.move_speed(), 50.0);
    assert!(
        !world.objects.has_component::<WaterTask>(&3001),
        "no drowning with AllowWater off"
    );
}

/// `Creature.moveToLocation`'s `isInWater` is `WATER && !CASTLE` — a castle
/// moat still swims (that's the plain WATER bit driving the speeds) but keeps
/// geodata and the 700-unit click clamp off.
#[test]
fn castle_zone_is_the_exception_to_the_water_movement_branch() {
    let (mut world, ..) = cast_test_world();
    insert_zone(&mut world, ZoneKind::Water, -1000, 1000, -1000, 1000);
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    super::zones::revalidate_zone(&mut world, 3001, true);
    assert!(
        super::position::is_in_water(&world, 3001),
        "open water: the movement branch is on"
    );

    insert_zone(&mut world, ZoneKind::Castle, -1000, 1000, -1000, 1000);
    assert!(
        world
            .objects
            .get_component::<Speeds>(&3001)
            .unwrap()
            .swimming,
        "the speed branch has no castle exception"
    );
    assert!(
        !super::position::is_in_water(&world, 3001),
        "inside a castle the moat does not disable geodata"
    );
}

/// "Make water move short": a swim click past 700 units is cut back to the
/// first 700 of the ray, all three axes scaled together.
#[test]
fn a_long_swim_click_is_clamped_to_700_units() {
    let (mut world, ..) = cast_test_world();
    insert_zone(
        &mut world,
        ZoneKind::Water,
        -10_000,
        10_000,
        -10_000,
        10_000,
    );
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    super::zones::revalidate_zone(&mut world, 3001, true);

    handle_move_backward_to_location(&mut world, 1, &move_body((3500, 0, -700), (0, 0, 0), 1));
    let mv = world
        .objects
        .get_component::<crate::model::components::Movement>(&3001)
        .expect("the swim move started");
    assert_eq!(
        (mv.0.dest_x, mv.0.dest_y, mv.0.dest_z),
        (700, 0, -140),
        "clamped to 700/3500 of the requested vector, z included"
    );
}
