//! Soulshots and spiritshots: charging, the grade refusal, the damage they
//! double, and the auto-shot toggle.

use super::*;

/// Using a soulshot with a matching-grade weapon charges the shot, consumes
/// `weapon.soulShotCount` from the stack, and plays the shot's `<skills>`
/// visual (`SoulShots.useItem`).
#[test]
fn soulshot_charges_consumes_and_plays_visual() {
    use crate::data::item_data::kinds::{CrystalType, ItemHandler};
    use crate::model::inventory::Inventory;
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 2, 2);
    world.data.item_data.insert_for_test(shot_template(
        1463,
        CrystalType::D,
        ItemHandler::SoulShots,
        2150,
    ));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = inventory::add_inventory_item(&mut world, 3001, 1463, 10).unwrap()[0];
    drain(&mut a_rx);

    items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "soulshot charged"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(1463),
        8,
        "weapon.soulShotCount (2) consumed"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE),
        "enable message sent"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE
                && i32::from_le_bytes(p[13..17].try_into().unwrap()) == 2150),
        "shot visual (skill 2150) broadcast"
    );
}

/// A soulshot whose grade doesn't match the equipped weapon is refused.
#[test]
fn soulshot_wrong_grade_is_refused() {
    use crate::data::item_data::kinds::{CrystalType, ItemHandler};
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 2, 2);
    // A C-grade soulshot on a D-grade weapon.
    world.data.item_data.insert_for_test(shot_template(
        1464,
        CrystalType::C,
        ItemHandler::SoulShots,
        2151,
    ));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    let shot_oid = inventory::add_inventory_item(&mut world, 3001, 1464, 10).unwrap()[0];
    drain(&mut a_rx);

    items::use_equipable_item(&mut world, 1, 3001, shot_oid);

    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "wrong-grade shot not charged"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(1464),
        10,
        "nothing consumed"
    );
}

/// A charged soulshot is spent on the next non-miss melee swing, doubles its
/// damage, and sets the `SHOT_USED` flag (`generateHit`).
#[test]
fn soulshot_consumed_on_hit_doubles_melee_damage() {
    use crate::game_loop;
    use crate::model::{Player, ShotType};

    fn attack_damage_and_flags(packets: &[Vec<u8>]) -> (i32, i32) {
        let atk = packets
            .iter()
            .find(|p| p[0] == server_packets::opcodes::ATTACK)
            .expect("Attack broadcast");
        (
            i32::from_le_bytes(atk[13..17].try_into().unwrap()),
            i32::from_le_bytes(atk[17..21].try_into().unwrap()),
        )
    }

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // A non-miss swing consumes five rolls in order: miss(1000), shield-rate(100),
    // shield-perfect(100), crit(100), random-damage(2r+1). Force them so both
    // swings are identical: hit, no shield (the NPC has none anyway), no crit,
    // and rand roll 10 → `rand_roll = 10 - 10 = 0` → random multiplier 1.0.
    const SWING_ROLLS: [i32; 5] = [0, 0, 0, 99, 10];

    // Control swing (no shot): plain hit, no crit.
    world.force_rolls(SWING_ROLLS);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (base_dmg, base_flags) = attack_damage_and_flags(&drain(&mut a_rx));
    assert_eq!(base_flags & 0x08, 0, "no soulshot flag without a charge");

    // Charged swing: identical rolls → exactly double, flag set, shot spent.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .charge_shot(ShotType::Soulshots);
    world.force_rolls(SWING_ROLLS);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let (ss_dmg, ss_flags) = attack_damage_and_flags(&drain(&mut a_rx));

    assert_eq!(ss_dmg, base_dmg * 2, "soulshot doubles the swing");
    assert_ne!(ss_flags & 0x08, 0, "SHOT_USED flag set");
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "shot consumed"
    );
}

/// A charged spiritshot doubles a magic attack's damage and is spent
/// (`calcMagicDam` `sps` bonus + `Skill` uncharge).
#[test]
fn spiritshot_doubles_magic_damage_and_is_consumed() {
    use crate::game_loop;
    use crate::model::components::stats::Vitals;
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // A nuke now carries Java's ±10 % `randomMod`; this test compares two
    // casts, so the spread is switched off rather than averaged out.
    zero_random_damage(&mut world, 3001);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    let skill = world
        .data
        .skill_data
        .get(1177, 1)
        .expect("Wind Strike")
        .clone();
    assert_eq!(skill.magic_type, 1, "test skill must be magic");
    drain(&mut a_rx);

    let start_hp = nvit(&world, npc_oid).cur_hp;
    // Control cast (no shot), non-crit. The trailing 0 pins the `MagicFailures`
    // success roll — unforced it resists ~3 % of the time against this mob, and
    // the halved damage reads as "the shot did nothing".
    world.force_rolls([999_999, 0]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let base = start_hp - nvit(&world, npc_oid).cur_hp;
    assert!(base > 0.0, "control nuke dealt damage");
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = start_hp;

    // Charged spiritshot cast, identical crit + success rolls.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .charge_shot(ShotType::Spiritshots);
    world.force_rolls([999_999, 0]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let ss = start_hp - nvit(&world, npc_oid).cur_hp;

    assert!(
        (ss - base * 2.0).abs() < 1e-6,
        "spiritshot doubles magic damage ({ss} vs {base})"
    );
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Spiritshots),
        "spiritshot consumed"
    );
}

/// A `PhysicalAttack` skill (Power Strike 3) deals damage end-to-end — the
/// regression guard for the whole family of physical skills that used to cast
/// but no-op — and a charged soulshot doubles it and is spent.
#[test]
fn physical_skill_damages_monster_and_soulshot_doubles() {
    use crate::game_loop;
    use crate::model::components::stats::{CombatStats, Vitals};
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 13;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    let skill = world
        .data
        .skill_data
        .get(3, 1)
        .expect("Power Strike")
        .clone();
    assert_eq!(skill.magic_type, 0, "test skill must be physical");
    // Zero the weapon random-damage spread so only the crit roll is consumed
    // and the damage is deterministic.
    world
        .objects
        .get_component_mut::<CombatStats>(&3001)
        .unwrap()
        .random_dmg = 0;
    drain(&mut a_rx);

    let start_hp = nvit(&world, npc_oid).cur_hp;
    // **Four** forced high rolls per cast, in the order the path draws them:
    // the unconditional top-of-cast magic-crit roll (unused for a physical
    // skill), the two `calcShldUse` rolls, then the physical-skill crit roll.
    // All fail, so damage is the non-crit, unblocked base.
    //
    // The shield pair arrived with the G20 shield slice and silently shifted
    // this queue: with only two values forced, the crit roll fell through to
    // the real RNG and the two casts could disagree — the test then failed
    // about two full-suite runs in three while still passing in isolation.
    // Control cast (no shot).
    world.force_rolls([999_999, 999_999, 999_999, 999_999]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let base = start_hp - nvit(&world, npc_oid).cur_hp;
    assert!(
        base > 0.0,
        "physical skill dealt damage (was a silent no-op before)"
    );
    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .cur_hp = start_hp;

    // Charged soulshot cast, identical (failed) crit rolls.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .charge_shot(ShotType::Soulshots);
    world.force_rolls([999_999, 999_999, 999_999, 999_999]);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &skill);
    let ss = start_hp - nvit(&world, npc_oid).cur_hp;

    assert!(
        (ss - base * 2.0).abs() < 1e-6,
        "soulshot doubles physical skill damage ({ss} vs {base})"
    );
    assert!(
        !world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "soulshot consumed"
    );
}

/// Toggling auto-use (`RequestAutoSoulShot`) with a matching weapon activates
/// the shot: `ExAutoSoulShot` ack, the auto-set records the item, and it's
/// charged immediately; a following attack keeps it topped up.
#[test]
fn auto_soulshot_toggle_activates_and_recharges() {
    use crate::data::item_data::kinds::{CrystalType, ItemHandler};
    use crate::game_loop;
    use crate::model::{Player, ShotType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    shot_weapon(&mut world, 9500, CrystalType::D, 1, 1);
    world.data.item_data.insert_for_test(shot_template(
        1463,
        CrystalType::D,
        ItemHandler::SoulShots,
        2150,
    ));
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    grant_and_equip(&mut world, 3001, 1, 9500);
    inventory::add_inventory_item(&mut world, 3001, 1463, 10);
    drain(&mut a_rx);

    // itemId=1463, enable=1, type=0.
    let mut body = Vec::new();
    body.extend_from_slice(&1463i32.to_le_bytes());
    body.extend_from_slice(&1i32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    items::handle_request_auto_soul_shot(&mut world, 1, &body);

    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .auto_shots
            .contains(&1463),
        "item recorded for auto-use"
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .is_charged_shot(ShotType::Soulshots),
        "charged on activation"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::EX
            && i16::from_le_bytes(p[1..3].try_into().unwrap())
                == server_packets::opcodes::EX_AUTO_SOUL_SHOT),
        "ExAutoSoulShot ack sent"
    );

    // The charge is spent on a hit, and the next attack auto-recharges it.
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Swing 1 spends the activation charge (no item, just the flag).
    world.force_rolls([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    drain(&mut a_rx);
    // Swing 2 finds no charge, auto-recharges (spends an item), then spends it:
    // the `SHOT_USED` flag on this swing proves the recharge fed it.
    world.force_rolls([0, 99, 10]);
    combat::do_auto_attack(&mut world, 3001, npc_oid);
    let atk = drain(&mut a_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::ATTACK)
        .expect("Attack");
    assert_ne!(
        i32::from_le_bytes(atk[17..21].try_into().unwrap()) & 0x08,
        0,
        "auto-shot re-charged and was spent on the 2nd swing"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(1463),
        8,
        "activation + one auto-recharge consumed two shots"
    );
}
