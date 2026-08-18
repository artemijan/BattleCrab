//! Multi-hit melee swings (G20): dual-weapon split hits and the polearm sweep.

use super::*;

use crate::data::item_data::{CrystalType, EtcItemType, ItemKind, ItemTemplate, WeaponType};
use crate::model::components::StatModifiers;
use crate::model::inventory::Inventory;
use crate::model::stats::Stat;

const ATTACKER: i32 = 2001;
const CID: u32 = 1;
const DUAL_ID: i32 = 8200;
const POLE_ID: i32 = 8201;
const SWORD_ID: i32 = 8202;

fn weapon_template(item_id: i32, name: &str, radius: i32, angle: i32) -> ItemTemplate {
    ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id,
        name: name.into(),
        kind: ItemKind::Weapon,
        crystal_type: CrystalType::None,
        crystal_count: 0,
        attack_radius: radius,
        attack_angle: angle,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        body_part: crate::data::item_data::SLOT_LR_HAND,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    }
}

fn melee_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = cast_test_world();
    // Real geometry: a polearm reaches 66 with a 120° arc; a sword 40/120.
    world
        .data
        .item_data
        .insert_for_test(weapon_template(DUAL_ID, "Test Duals", 40, 120));
    world
        .data
        .item_data
        .set_weapon_type_for_test(DUAL_ID, WeaponType::Dual);
    world
        .data
        .item_data
        .insert_for_test(weapon_template(POLE_ID, "Test Polearm", 66, 120));
    world
        .data
        .item_data
        .set_weapon_type_for_test(POLE_ID, WeaponType::Pole);
    world
        .data
        .item_data
        .insert_for_test(weapon_template(SWORD_ID, "Test Sword", 40, 120));
    world
        .data
        .item_data
        .set_weapon_type_for_test(SWORD_ID, WeaponType::Sword);
    (world, db, l)
}

fn equip(world: &mut World, item_id: i32) {
    let obj_id = 0x5100_0000 + item_id;
    let World { objects, data, .. } = world;
    let inv = objects
        .get_component_mut::<Inventory>(&ATTACKER)
        .expect("inventory");
    inv.add_item(&data.item_data, obj_id, item_id, 1);
    inv.equip_item(&data.item_data, obj_id);
}

/// Grant Polearm Mastery's `ATTACK_COUNT_MAX` bonus directly (skill 216 is
/// `HitNumber` amount 5 → 4 extra targets beyond the base 1).
fn grant_hit_number(world: &mut World, extra: f64) {
    world
        .objects
        .get_component_mut::<StatModifiers>(&ATTACKER)
        .expect("stat modifiers")
        .add
        .insert(Stat::AttackCountMax, extra);
}

/// The hits carried by the Attack packet: `(target_id, damage)`, first inline
/// then the additional-hit block.
fn attack_hits(pkts: &[Vec<u8>]) -> Vec<(i32, i32)> {
    let pkt = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::ATTACK)
        .expect("Attack packet");
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    let _attacker = r.read_i32().unwrap();
    let first_target = r.read_i32().unwrap();
    let _ss_substitute = r.read_i32().unwrap();
    let first_damage = r.read_i32().unwrap();
    let _flags = r.read_i32().unwrap();
    let _grade = r.read_i32().unwrap();
    let (_x, _y, _z) = (
        r.read_i32().unwrap(),
        r.read_i32().unwrap(),
        r.read_i32().unwrap(),
    );
    let extra = r.read_i16().unwrap();
    let mut out = vec![(first_target, first_damage)];
    for _ in 0..extra {
        let t = r.read_i32().unwrap();
        let d = r.read_i32().unwrap();
        let _f = r.read_i32().unwrap();
        let _g = r.read_i32().unwrap();
        out.push((t, d));
    }
    out
}

// ---------------------------------------------------------------------------

/// A **dual** weapon strikes the main target twice, each hit at half damage
/// and **independently rolled** (Java calls `generateHit` twice): pin one
/// crit and one plain hit through the forced-roll tape — under the old
/// shared-roll shape the two damages could never differ.
#[test]
fn dual_weapon_strikes_twice_with_independent_rolls() {
    let (mut world, _db, _l) = melee_world();
    let mut out = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    equip(&mut world, DUAL_ID);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);
    drain(&mut out);

    // Per hit: miss(1000), shield(100)×2, crit(100), random-damage. Hit 1
    // crits (roll 0), hit 2 doesn't (roll 99); everything else identical.
    world.force_rolls([0, 99, 99, 0, 0, 0, 99, 99, 99, 0]);
    combat::do_auto_attack(&mut world, ATTACKER, NPC_OID);
    world.clear_forced_rolls();
    let hits = attack_hits(&drain(&mut out));

    assert_eq!(hits.len(), 2, "a dual swing carries two hits: {hits:?}");
    assert!(
        hits.iter().all(|(t, _)| *t == NPC_OID),
        "both land on the main target"
    );
    assert!(
        hits[0].1 > hits[1].1,
        "hit 1 crit, hit 2 didn't — the rolls are independent: {hits:?}"
    );
    assert!(hits[1].1 > 0, "the plain half still lands");
}

/// A single-handed weapon still swings once.
#[test]
fn single_weapon_strikes_once() {
    let (mut world, _db, _l) = melee_world();
    let mut out = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    equip(&mut world, SWORD_ID);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);
    drain(&mut out);

    combat::do_auto_attack(&mut world, ATTACKER, NPC_OID);
    assert_eq!(attack_hits(&drain(&mut out)).len(), 1);
}

/// Without Polearm Mastery a polearm hits only its target — the sweep is gated
/// on `ATTACK_COUNT_MAX`, not on the weapon type.
#[test]
fn polearm_without_mastery_hits_one_target() {
    let (mut world, _db, _l) = melee_world();
    let mut out = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    equip(&mut world, POLE_ID);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 20001, "Monster", 5, 50, 0, 0);
    drain(&mut out);

    combat::do_auto_attack(&mut world, ATTACKER, NPC_OID);
    assert_eq!(
        attack_hits(&drain(&mut out)).len(),
        1,
        "no mastery, no sweep"
    );
}

/// **With** mastery the polearm sweeps neighbours inside its radius and arc —
/// the G20 gate line "a polearm hits a line".
#[test]
fn polearm_with_mastery_sweeps_neighbours() {
    let (mut world, _db, _l) = melee_world();
    let mut out = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    equip(&mut world, POLE_ID);
    grant_hit_number(&mut world, 4.0); // 1 base + 4 extra = 5, like skill 216

    // Two mobs straight ahead inside the 66 radius, one far outside it.
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 20001, "Monster", 5, 55, 0, 0);
    add_test_npc(&mut world, NPC_OID + 2, 20001, "Monster", 5, 900, 0, 0);
    // Face them (heading 0 = +x).
    world
        .objects
        .get_component_mut::<Position>(&ATTACKER)
        .unwrap()
        .heading = 0;
    drain(&mut out);

    combat::do_auto_attack(&mut world, ATTACKER, NPC_OID);
    let hits = attack_hits(&drain(&mut out));
    let targets: Vec<i32> = hits.iter().map(|(t, _)| *t).collect();

    assert!(targets.contains(&NPC_OID), "the main target is hit");
    assert!(
        targets.contains(&(NPC_OID + 1)),
        "the neighbour in the arc is swept: {targets:?}"
    );
    assert!(
        !targets.contains(&(NPC_OID + 2)),
        "the distant mob is not: {targets:?}"
    );
}

/// The sweep is capped by `ATTACK_COUNT_MAX` — one extra target means two hits
/// total, however many neighbours are in reach.
#[test]
fn sweep_is_capped_by_attack_count() {
    let (mut world, _db, _l) = melee_world();
    let mut out = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    equip(&mut world, POLE_ID);
    grant_hit_number(&mut world, 1.0); // 1 base + 1 extra = 2

    for i in 0..4 {
        add_test_npc(
            &mut world,
            NPC_OID + i,
            20001,
            "Monster",
            5,
            30 + i * 5,
            0,
            0,
        );
    }
    world
        .objects
        .get_component_mut::<Position>(&ATTACKER)
        .unwrap()
        .heading = 0;
    drain(&mut out);

    combat::do_auto_attack(&mut world, ATTACKER, NPC_OID);
    assert_eq!(
        attack_hits(&drain(&mut out)).len(),
        2,
        "capped at the base + one extra"
    );
}

/// A creature behind the attacker is outside the arc and is not swept up.
#[test]
fn sweep_respects_the_attack_angle() {
    let (mut world, _db, _l) = melee_world();
    let mut out = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    equip(&mut world, POLE_ID);
    grant_hit_number(&mut world, 4.0);

    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0); // ahead (+x)
    add_test_npc(&mut world, NPC_OID + 1, 20001, "Monster", 5, -40, 0, 0); // directly behind
    world
        .objects
        .get_component_mut::<Position>(&ATTACKER)
        .unwrap()
        .heading = 0; // facing +x
    drain(&mut out);

    combat::do_auto_attack(&mut world, ATTACKER, NPC_OID);
    let targets: Vec<i32> = attack_hits(&drain(&mut out))
        .iter()
        .map(|(t, _)| *t)
        .collect();
    assert!(
        !targets.contains(&(NPC_OID + 1)),
        "a mob 180° behind is outside the 120° arc: {targets:?}"
    );
}

/// **Focus Attack (317)** is a *trade*: accuracy and crit damage in exchange
/// for giving up the sweep. Its two stat halves landed through the effect
/// registry long before the sweep gate did, so until G34 S4 the toggle was a
/// pure bonus with no cost at all — the thing it exists to trade away was
/// still happening.
#[test]
fn focus_attack_gives_up_the_polearm_sweep() {
    let (mut world, _db, _l) = melee_world();
    let mut out = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    equip(&mut world, POLE_ID);
    grant_hit_number(&mut world, 4.0);

    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 20001, "Monster", 5, 55, 0, 0);
    world
        .objects
        .get_component_mut::<Position>(&ATTACKER)
        .unwrap()
        .heading = 0;

    // Focus Attack up: `PHYSICAL_POLEARM_TARGET_SINGLE` above 0.
    world
        .objects
        .get_component_mut::<StatModifiers>(&ATTACKER)
        .expect("stat modifiers")
        .add
        .insert(Stat::PhysicalPolearmTargetSingle, 1.0);
    drain(&mut out);

    combat::do_auto_attack(&mut world, ATTACKER, NPC_OID);
    let hits = attack_hits(&drain(&mut out));
    let targets: Vec<i32> = hits.iter().map(|(t, _)| *t).collect();

    assert!(targets.contains(&NPC_OID), "the main target is still hit");
    assert!(
        !targets.contains(&(NPC_OID + 1)),
        "but the neighbour is not swept — that is the whole cost: {targets:?}"
    );
}
