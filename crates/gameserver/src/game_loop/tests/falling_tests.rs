//! Falling damage — `Player.isFalling` / `Formulas.calcFallDam` and the 1.5 s
//! `_fallingDamageTask`.
//!
//! Every test here drives the real `ValidatePosition` handler rather than
//! calling `falling::*` directly, because the branch that matters most is the
//! one that makes `handle_validate_position` **return early**: a unit test of
//! `is_falling` alone would pass against a server that never calls it.

use super::*;
use crate::game_loop::falling::{self, SAFE_FALL_HEIGHT};
use crate::game_loop::position::handle_validate_position;
use crate::model::components::{AdminFlags, FallingDamage, StatModifiers, Vitals};
use crate::model::stats::Stat;

/// Enough drop to be a fall, with room to spare over the 333 safe height.
const DROP: i32 = 1333;

/// A player standing at z = 0 inside the synthetic geodata region, with a
/// known max HP so the damage arithmetic is exact.
fn faller(world: &mut World, max_hp: i32) -> UnboundedReceiver<bytes::Bytes> {
    install_wall_region(world);
    let rx = ingame_player(world, 1, 4001, 1000, 1000, 0);
    let v = world.objects.get_component_mut::<Vitals>(&4001).unwrap();
    v.max_hp = max_hp;
    v.cur_hp = max_hp as f64;
    rx
}

/// Run the game-loop sweep `n` ticks forward.
fn advance(world: &mut World, ticks: u64) {
    for _ in 0..ticks {
        world.tick += 1;
        falling::falling_damage_tick(world);
    }
}

/// The whole chain, end to end: the client reports a Z far below the server's,
/// the server answers `ValidateLocation` to stop the client sinking, arms the
/// damage, and takes the HP **1.5 s later** — not on the report.
///
/// The delay is asserted on both sides (nothing at 14 ticks, everything at 15)
/// because applying the damage inline would pass a test that only checked the
/// end state, and would take the HP while the player is still in the air.
#[test]
fn a_fall_beyond_the_safe_height_costs_hp_after_the_delay() {
    let (mut world, ..) = test_world();
    let mut rx = faller(&mut world, 1000);
    zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, -DROP, 0));

    // "Prevent falling under ground."
    let pkt = rx.try_recv().expect("ValidateLocation sent");
    assert_eq!(pkt[0], server_packets::opcodes::VALIDATE_LOCATION);

    // `calcFallDam` = fallHeight × maxHp / 1000 = 1333 × 1000 / 1000.
    let armed = world
        .objects
        .get_component::<FallingDamage>(&4001)
        .expect("damage armed");
    assert_eq!(armed.damage, 1333);
    assert_eq!(
        world.objects.get_component::<Vitals>(&4001).unwrap().cur_hp,
        1000.0,
        "no HP comes off on the report itself"
    );

    advance(&mut world, 14);
    assert_eq!(
        world.objects.get_component::<Vitals>(&4001).unwrap().cur_hp,
        1000.0,
        "still airborne at 1.4 s"
    );

    advance(&mut world, 1);
    // Clamped to `getCurrentHp() - 1`: 1333 > 999, so the fall leaves 1 HP.
    assert_eq!(
        world.objects.get_component::<Vitals>(&4001).unwrap().cur_hp,
        1.0,
        "a fall never kills — `min(damage, currentHp - 1)`"
    );
    assert!(
        world
            .objects
            .get_component::<FallingDamage>(&4001)
            .is_none(),
        "the task clears itself"
    );

    // Java reports the **unclamped** `_fallingDamage` in the message even
    // though it applied the clamped 999. Ported as written, and asserted on
    // the parameter rather than just the id — reporting the clamped figure
    // would be the natural "fix" and would still send the right message.
    let sm = drain(&mut rx)
        .into_iter()
        .find(|p| {
            p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOU_RECEIVED_S1_FALLING_DAMAGE
        })
        .expect("falling-damage system message");
    assert_eq!(
        i32::from_le_bytes(sm[5..9].try_into().unwrap()),
        1333,
        "the message carries the full damage, not the clamped 999"
    );
}

/// The safe height is a **boundary**, and Java's test is `deltaZ <= safeFall`,
/// so a drop of exactly 333 is not a fall. One unit more is.
///
/// Pinned at both sides because an off-by-one here is invisible in play and
/// would make every staircase in Giran cost HP.
#[test]
fn the_safe_height_boundary_is_inclusive() {
    let (mut world, ..) = test_world();
    let mut rx = faller(&mut world, 1000);
    zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    handle_validate_position(
        &mut world,
        1,
        &validate_position_body(1000, 1000, -SAFE_FALL_HEIGHT, 0),
    );
    assert!(
        world
            .objects
            .get_component::<FallingDamage>(&4001)
            .is_none(),
        "a drop of exactly the safe height is not a fall"
    );

    // Put the server back on top before the second report — the first one
    // reconciled normally and adopted the client's z.
    world
        .objects
        .get_component_mut::<Position>(&4001)
        .unwrap()
        .z = 0;
    handle_validate_position(
        &mut world,
        1,
        &validate_position_body(1000, 1000, -SAFE_FALL_HEIGHT - 1, 0),
    );
    assert!(
        world
            .objects
            .get_component::<FallingDamage>(&4001)
            .is_some(),
        "one unit past it is"
    );
}

/// **The point of the whole `isFalling` return value.** For a second after a
/// falling report, `ValidatePosition` must bail *before* reconciliation —
/// Java's "Disable validations during fall to avoid jumping".
///
/// Without the bail the desync snap adopts the client's position mid-air and
/// the player visibly stutters down the drop. The test moves the client
/// sideways far enough that reconciliation would be unmissable, and asserts
/// the server did not budge.
#[test]
fn position_reports_are_swallowed_while_a_fall_is_in_progress() {
    let (mut world, ..) = test_world();
    let mut rx = faller(&mut world, 1000);
    zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, -DROP, 0));
    drain(&mut rx);

    // Mid-fall, 1 tick later: a 2000-unit jump that would normally snap the
    // server straight onto the client.
    world.tick += 1;
    handle_validate_position(&mut world, 1, &validate_position_body(3000, 1000, -DROP, 0));
    let pos = *world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (1000, 1000),
        "the report is swallowed, not reconciled"
    );
    assert!(
        world
            .objects
            .get_component::<ClientPos>(&4001)
            .is_some_and(|c| c.x != 3000),
        "and it does not even update the client-position mirror"
    );

    // The window is 1 s. Past it, the same report reconciles normally.
    world.tick += 10;
    handle_validate_position(&mut world, 1, &validate_position_body(3000, 1000, -DROP, 0));
    assert_eq!(
        world.objects.get_component::<Position>(&4001).unwrap().x,
        3000,
        "the latch expires after FALLING_VALIDATION_DELAY"
    );
}

/// `Stat.FALL` — Acrobatics (173), the one learnable source, and the reason
/// the effect had to be registered at all.
///
/// Two things are pinned. First that the stat is consulted; second, and more
/// easily got wrong, **the order of operations**: Java's
/// `Stat.defaultValue(creature, stat, base)` is `mul * base + add`, so a
/// multiplier applies to the base and the flat term lands afterwards. A
/// modifier pair is used precisely because `(base + add) * mul` gives a
/// different answer and a single-modifier test could not tell them apart.
#[test]
fn the_fall_stat_reduces_the_damage_and_the_add_lands_after_the_multiply() {
    let (mut world, ..) = test_world();
    let _rx = faller(&mut world, 1000);

    assert_eq!(
        falling::calc_fall_dam(&world, 4001, 1000),
        1000.0,
        "unmodified: fallHeight × maxHp / 1000"
    );

    // Acrobatics level 1 is `DIFF -60` — a flat 60 off the damage, despite
    // the effect being named for the safe *height*.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&4001)
        .cloned()
        .expect("stat modifiers");
    mods.add.insert(Stat::Fall, -60.0);
    world.objects.add_components(&4001, mods.clone());
    assert_eq!(falling::calc_fall_dam(&world, 4001, 1000), 940.0);

    mods.mul.insert(Stat::Fall, 2.0);
    world.objects.add_components(&4001, mods);
    assert_eq!(
        falling::calc_fall_dam(&world, 4001, 1000),
        2.0 * 1000.0 - 60.0,
        "mul * base + add — not (base + add) * mul, which would give 1880"
    );
}

/// `EnableFallingDamage = False` switches off **the damage only**. Java puts
/// the config check inside `Formulas.calcFallDam`, not around `isFalling`, so
/// the client re-grounding and the validation window still happen — turning
/// the key off must not reintroduce the mid-air stutter.
///
/// The obvious wrong implementation (gate the whole handler on the config)
/// passes an HP assertion and fails this one.
#[test]
fn disabling_the_config_stops_the_damage_but_not_the_position_handling() {
    let (mut world, ..) = test_world();
    world.cfg.general.enable_falling_damage = false;
    let mut rx = faller(&mut world, 1000);
    zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, -DROP, 0));

    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::VALIDATE_LOCATION),
        "the client is still pushed back above ground"
    );
    assert_eq!(
        world
            .objects
            .get_component::<FallingDamage>(&4001)
            .expect("the task is still armed")
            .damage,
        0,
        "…with nothing to apply"
    );

    // The suppression survives with the damage off: still swallowed one tick
    // in, while the validation window is open.
    world.tick += 1;
    handle_validate_position(&mut world, 1, &validate_position_body(3000, 1000, -DROP, 0));
    assert_eq!(
        world.objects.get_component::<Position>(&4001).unwrap().x,
        1000
    );

    advance(&mut world, 20);
    assert_eq!(
        world.objects.get_component::<Vitals>(&4001).unwrap().cur_hp,
        1000.0
    );
}

/// The damage is computed **once**, on the report that opens the fall
/// (`if (_fallingDamage == 0)`), and every later report only pushes the clock
/// out. A long fall is priced by its first measured drop, not by the deepest
/// one — which is Java's behaviour and is easy to "improve" into a
/// recomputing version that hits far harder.
#[test]
fn a_continuing_fall_reprices_nothing_and_only_defers_the_clock() {
    let (mut world, ..) = test_world();
    let mut rx = faller(&mut world, 10_000);
    zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, -400, 0));
    let first = *world
        .objects
        .get_component::<FallingDamage>(&4001)
        .expect("armed");
    assert_eq!(first.damage, 4000, "400 × 10000 / 1000");

    // A second report, past the validation window, reporting a much deeper Z.
    world.tick += 11;
    world
        .objects
        .get_component_mut::<Position>(&4001)
        .unwrap()
        .z = 0;
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, -5000, 0));
    let second = *world
        .objects
        .get_component::<FallingDamage>(&4001)
        .expect("still armed");
    assert_eq!(second.damage, 4000, "the price is not recomputed");
    assert_eq!(
        second.due_tick,
        world.tick + 15,
        "but the 1.5 s clock restarts from this report"
    );
}

/// The three states in which Java refuses to call a drop a fall at all:
/// dead, flying (wyvern), and inside a water zone. Each is a separate `||`
/// arm in `isFalling`, and each would otherwise fire on the *first* report:
/// a corpse sliding down a slope, a dismounting wyvern rider, and anyone
/// swimming down.
#[test]
fn death_flight_and_water_are_not_falls() {
    use crate::data::zone_data::ZoneKind;
    use crate::model::components::ZoneFlags;

    for case in ["dead", "flying", "water"] {
        let (mut world, ..) = test_world();
        let _rx = faller(&mut world, 1000);
        match case {
            "dead" => {
                world
                    .objects
                    .get_component_mut::<Vitals>(&4001)
                    .unwrap()
                    .dead = true
            }
            "flying" => {
                // `MountType.WYVERN` — `Player::is_flying`'s only source here.
                world
                    .objects
                    .get_component_mut::<Player>(&4001)
                    .unwrap()
                    .mount_type = 2;
            }
            _ => world.objects.add_components(
                &4001,
                ZoneFlags {
                    mask: ZoneKind::Water.bit(),
                    ..Default::default()
                },
            ),
        }

        assert!(
            !falling::is_falling(&mut world, 4001, -DROP),
            "{case}: not a fall"
        );
        assert!(
            world
                .objects
                .get_component::<FallingDamage>(&4001)
                .is_none(),
            "{case}: nothing armed"
        );
    }
}

/// `if ((_fallingDamage > 0) && !isInvul())` — an invulnerable GM takes no
/// fall damage, and Java clears the pending fall regardless, so it is
/// **discarded rather than deferred** to the moment invulnerability drops.
#[test]
fn an_invulnerable_player_discards_the_pending_fall() {
    let (mut world, ..) = test_world();
    let mut rx = faller(&mut world, 1000);
    zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, -DROP, 0));
    world.objects.add_components(
        &4001,
        AdminFlags {
            invul: true,
            ..Default::default()
        },
    );

    advance(&mut world, 15);
    assert_eq!(
        world.objects.get_component::<Vitals>(&4001).unwrap().cur_hp,
        1000.0,
        "invul takes nothing"
    );
    assert!(
        world
            .objects
            .get_component::<FallingDamage>(&4001)
            .is_none(),
        "and the fall is dropped, not held for later"
    );
}
