//! S10 — soul crystal options on an item: the load filter, the skills an
//! equipped ensouled weapon grants, and the item block's `SOUL_CRYSTAL` bytes.
//!
//! All against the dist: the Arcana Mace (6579, S grade) has one regular slot
//! and one special. Option 1 is Might (skill 13856 level 1, P. Atk.), option
//! 526 is Tyrr (13898 level 1), a special-slot option.

use super::*;

use crate::game_loop::items::ensoul;
use crate::model::components::skills::{Buffs, OptionSkills, SkillBook};
use crate::model::inventory::Inventory;

const PLAYER: i32 = 8101;
const CID: u32 = 1;
const ARCANA_MACE: i32 = 6579;
const MACE_OID: i32 = 9601;
const MIGHT: i32 = 1;
const MIGHT_SKILL: i32 = 13856;
const TYRR: i32 = 526;
const TYRR_SKILL: i32 = 13898;
/// Java `CommonSkill.EXPERTISE`; level 5 is S grade.
const EXPERTISE: i32 = 239;

fn ensoul_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data.item_data = crate::data::dist::items_owned();
    world.data.skill_data = crate::data::dist::skills_owned();
    world.data.ensoul = crate::data::EnsoulData::load_from(crate::data::DIST_GAME);
    world.id_pool = 0x4300_0000..0x4300_0100;
    let rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    (world, rx)
}

fn set_expertise(world: &mut World, level: i32) {
    world
        .objects
        .get_component_mut::<SkillBook>(&PLAYER)
        .unwrap()
        .0
        .insert(EXPERTISE, level);
}

/// The ensouled mace, in the bag and not yet worn.
fn give_ensouled_mace(world: &mut World) {
    let World { objects, data, .. } = world;
    let inv = objects.get_component_mut::<Inventory>(&PLAYER).unwrap();
    inv.add_item(&data.item_data, MACE_OID, ARCANA_MACE, 1);
    inv.set_ensoul_options(MACE_OID, [MIGHT, 0], TYRR);
}

fn option_skill(world: &World, skill_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<OptionSkills>(&PLAYER)
        .and_then(|o| o.0.get(&skill_id).copied())
}

/// The skill's passive pump buff is up — where its stats come from.
fn pumped(world: &World, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&PLAYER)
        .is_some_and(|b| b.0.iter().any(|x| x.passive && x.skill_id == skill_id))
}

/// Equipping grants both options' skills `store = false` (never in the
/// book) and pumps their stats; unequipping takes both away again.
#[test]
fn equipping_grants_the_soul_crystal_skills_and_unequipping_takes_them() {
    let (mut world, mut rx) = ensoul_world();
    set_expertise(&mut world, 5);
    give_ensouled_mace(&mut world);
    drain(&mut rx);

    items::handle_use_item(&mut world, CID, &use_item_body(MACE_OID));
    assert_eq!(option_skill(&world, MIGHT_SKILL), Some(1));
    assert_eq!(option_skill(&world, TYRR_SKILL), Some(1));
    assert!(pumped(&world, MIGHT_SKILL), "Might's P. Atk. is pumped");
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&PLAYER)
            .unwrap()
            .0
            .contains_key(&MIGHT_SKILL),
        "never filed in the persisted book"
    );

    items::handle_use_item(&mut world, CID, &use_item_body(MACE_OID));
    assert_eq!(option_skill(&world, MIGHT_SKILL), None);
    assert_eq!(option_skill(&world, TYRR_SKILL), None);
    assert!(!pumped(&world, MIGHT_SKILL), "the pump buff goes with it");
}

/// Java's equip listener returns before any grant when the weapon's grade is
/// above the wearer's expertise; the enter-world `applyItemSkills` makes no
/// such check, so the same weapon worn through a relog does get them.
#[test]
fn an_over_grade_weapon_grants_nothing_on_equip_but_does_at_login() {
    let (mut world, mut rx) = ensoul_world();
    set_expertise(&mut world, 4);
    give_ensouled_mace(&mut world);
    drain(&mut rx);

    items::handle_use_item(&mut world, CID, &use_item_body(MACE_OID));
    assert!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .paperdoll_slot_of(MACE_OID)
            .is_some(),
        "worn all the same"
    );
    assert_eq!(option_skill(&world, MIGHT_SKILL), None, "A grade wears S");

    ensoul::apply_equipped(&mut world, PLAYER);
    crate::game_loop::stats::passive_skills::refresh_conditioned_passives(&mut world, PLAYER);
    assert_eq!(option_skill(&world, MIGHT_SKILL), Some(1));
    assert!(pumped(&world, MIGHT_SKILL));
}

/// A worn ensouled weapon destroyed out from under its wearer takes its
/// skills along — the ids come off the snapshot, not the (gone) instance.
#[test]
fn destroying_the_worn_weapon_clears_its_skills() {
    let (mut world, mut rx) = ensoul_world();
    set_expertise(&mut world, 5);
    give_ensouled_mace(&mut world);
    drain(&mut rx);
    items::handle_use_item(&mut world, CID, &use_item_body(MACE_OID));
    assert_eq!(option_skill(&world, MIGHT_SKILL), Some(1));

    items::destroy_item_by_object_id(&mut world, PLAYER, MACE_OID, 1);
    assert_eq!(option_skill(&world, MIGHT_SKILL), None);
    assert_eq!(option_skill(&world, TYRR_SKILL), None);
    assert!(!pumped(&world, MIGHT_SKILL));
}

/// `restoreSpecialAbilities` files a row only for an equipable item, a known
/// option, and a position under the template's slot count.
#[test]
fn the_load_drops_what_java_would_not_restore() {
    let (world, _rx) = ensoul_world();

    let mut row = item_row_at(MACE_OID, ARCANA_MACE, 1, "INVENTORY", 0);
    // The mace has one regular slot, so position 1 is past it.
    row.ensoul_options = [MIGHT, MIGHT];
    row.ensoul_special_option = TYRR;
    ensoul::restore(&world.data, &mut row);
    assert_eq!(row.ensoul_options, [MIGHT, 0]);
    assert_eq!(row.ensoul_special_option, TYRR);

    // Not an option at all.
    row.ensoul_options = [99_999, 0];
    ensoul::restore(&world.data, &mut row);
    assert_eq!(row.ensoul_options, [0, 0], "unknown option");

    // Adena has no body part.
    let mut adena = item_row_at(MACE_OID + 1, 57, 1, "INVENTORY", 0);
    adena.ensoul_options = [MIGHT, 0];
    adena.ensoul_special_option = TYRR;
    ensoul::restore(&world.data, &mut adena);
    assert_eq!(
        (adena.ensoul_options, adena.ensoul_special_option),
        ([0, 0], 0)
    );
}

/// The item block sets `SOUL_CRYSTAL` (0x10) and appends the two lists, each
/// a count byte then the ids — empty slots skipped, not written as 0.
#[test]
fn the_item_block_carries_the_soul_crystal_lists() {
    let (mut world, _rx) = ensoul_world();
    give_ensouled_mace(&mut world);
    let inv = world.objects.get_component::<Inventory>(&PLAYER).unwrap();
    let item = inv.by_object_id(MACE_OID).unwrap();
    let template = world.data.item_data.get(ARCANA_MACE).unwrap();

    let mut w = PacketWriter::new();
    crate::network::enter_world::write_item_entry(&mut w, item, template, false);
    let bytes = w.into_bytes();
    assert_eq!(bytes[0], 0x10, "mask");
    let mut tail = vec![1u8];
    tail.extend_from_slice(&MIGHT.to_le_bytes());
    tail.push(1);
    tail.extend_from_slice(&TYRR.to_le_bytes());
    assert!(bytes.ends_with(&tail), "regular then special list");

    let mut plain = *item;
    plain.ensoul_options = [0; 2];
    plain.ensoul_special_option = 0;
    let mut w = PacketWriter::new();
    crate::network::enter_world::write_item_entry(&mut w, &plain, template, false);
    let bytes = w.into_bytes();
    assert_eq!(bytes[0], 0, "no mask bit and no trailing lists");
    assert_eq!(
        bytes.len(),
        1 + 4 + 4 + 1 + 8 + 1 + 1 + 2 + 8 + 1 + 1 + 4 + 4 + 1 + 1 + 1
    );
}

// ---------------------------------------------------------------------------
// RequestItemEnsoul / RequestTryEnSoulExtraction
// ---------------------------------------------------------------------------

/// Soul crystal stone (slot type 1) listing Might.
const MIGHT_STONE: i32 = 46444;
/// Mystic stone (slot type 2) listing Tyrr.
const TYRR_STONE: i32 = 46553;
const S_GEMSTONE: i32 = 2134;
const STONE_OID: i32 = 9602;
const GEMS_OID: i32 = 9603;
const MYSTIC_OID: i32 = 9604;
/// Helios Shaper — R99, two regular slots and a special one.
const HELIOS_SHAPER: i32 = 17416;

/// `RequestItemEnsoul` body: item, count, then `(type, position, crystal,
/// option)` per entry.
fn ensoul_body(item_oid: i32, entries: &[(u8, u8, i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(item_oid);
    w.write_u8(entries.len() as u8);
    for &(slot_type, position, crystal, option) in entries {
        w.write_u8(slot_type);
        w.write_u8(position);
        w.write_i32(crystal);
        w.write_i32(option);
    }
    w.into_bytes()
}

fn extraction_body(item_oid: i32, slot_type: u8, position: u8) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(item_oid);
    w.write_u8(slot_type);
    w.write_u8(position);
    w.into_bytes()
}

fn give(world: &mut World, oid: i32, item_id: i32, count: i64) {
    let World { objects, data, .. } = world;
    objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .add_item(&data.item_data, oid, item_id, count);
}

fn slots(world: &World, oid: i32) -> ([i32; 2], i32) {
    world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .and_then(|inv| inv.by_object_id(oid))
        .map(|it| (it.ensoul_options, it.ensoul_special_option))
        .unwrap()
}

fn count(world: &World, item_id: i32) -> i64 {
    crate::game_loop::character::inventory::count_of(world, PLAYER, item_id)
}

fn ensoul_result(packets: &[Vec<u8>]) -> Option<Vec<u8>> {
    packets
        .iter()
        .find(|p| p.len() >= 3 && p[0] == 0xFE && i16::from_le_bytes([p[1], p[2]]) == 0x181)
        .map(|p| p[3..].to_vec())
}

/// Both slot types in one request: each stone and its fee leave the bag
/// (S grade: 19 gemstones for the regular slot, 7 for the mystic one), both
/// options are filed, and the result lists them.
#[test]
fn ensouling_fills_both_slots_and_takes_the_fees() {
    let (mut world, mut rx) = ensoul_world();
    give(&mut world, MACE_OID, ARCANA_MACE, 1);
    give(&mut world, STONE_OID, MIGHT_STONE, 1);
    give(&mut world, MYSTIC_OID, TYRR_STONE, 1);
    give(&mut world, GEMS_OID, S_GEMSTONE, 30);
    drain(&mut rx);

    crate::game_loop::items::ensoul::handle_request_item_ensoul(
        &mut world,
        CID,
        &ensoul_body(
            MACE_OID,
            &[(1, 1, STONE_OID, MIGHT), (2, 1, MYSTIC_OID, TYRR)],
        ),
    );
    assert_eq!(slots(&world, MACE_OID), ([MIGHT, 0], TYRR));
    assert_eq!(count(&world, MIGHT_STONE), 0);
    assert_eq!(count(&world, TYRR_STONE), 0);
    assert_eq!(count(&world, S_GEMSTONE), 30 - 19 - 7);
    let mut expect = vec![1u8, 1];
    expect.extend_from_slice(&MIGHT.to_le_bytes());
    expect.push(1);
    expect.extend_from_slice(&TYRR.to_le_bytes());
    assert_eq!(ensoul_result(&drain(&mut rx)), Some(expect));
    // Not worn, so nothing is granted.
    assert_eq!(option_skill(&world, MIGHT_SKILL), None);
}

/// On a worn weapon the new option's skill is granted at once, with no
/// expertise check, and re-ensouling a filled slot charges the re-ensoul fee
/// (10 for S) and swaps the skill.
#[test]
fn ensouling_a_worn_weapon_grants_and_replaces_the_skill() {
    let (mut world, mut rx) = ensoul_world();
    set_expertise(&mut world, 5);
    give(&mut world, MACE_OID, ARCANA_MACE, 1);
    items::handle_use_item(&mut world, CID, &use_item_body(MACE_OID));
    set_expertise(&mut world, 0);
    give(&mut world, STONE_OID, MIGHT_STONE, 2);
    give(&mut world, GEMS_OID, S_GEMSTONE, 29);
    drain(&mut rx);

    let body = ensoul_body(MACE_OID, &[(1, 1, STONE_OID, MIGHT)]);
    crate::game_loop::items::ensoul::handle_request_item_ensoul(&mut world, CID, &body);
    assert_eq!(option_skill(&world, MIGHT_SKILL), Some(1));
    assert!(pumped(&world, MIGHT_SKILL));
    assert_eq!(count(&world, S_GEMSTONE), 10);

    // Option 16, another option on the same stone, over the filled slot.
    let body = ensoul_body(MACE_OID, &[(1, 1, STONE_OID, 16)]);
    crate::game_loop::items::ensoul::handle_request_item_ensoul(&mut world, CID, &body);
    assert_eq!(slots(&world, MACE_OID).0, [16, 0]);
    assert_eq!(count(&world, S_GEMSTONE), 0, "the re-ensoul fee");
    let replacement = world.data.ensoul.option(16).unwrap();
    let (skill_id, level) = (replacement.skill_id, replacement.skill_level);
    assert_ne!(skill_id, MIGHT_SKILL);
    assert_eq!(
        option_skill(&world, MIGHT_SKILL),
        None,
        "the old skill is gone"
    );
    assert!(!pumped(&world, MIGHT_SKILL));
    assert_eq!(option_skill(&world, skill_id), Some(level));
}

/// The refusals send their message and take nothing; a frozen player gets the
/// message but Java does not return, so the ensoul goes through.
#[test]
fn seated_refuses_and_frozen_only_complains() {
    let (mut world, mut rx) = ensoul_world();
    give(&mut world, MACE_OID, ARCANA_MACE, 1);
    give(&mut world, STONE_OID, MIGHT_STONE, 1);
    give(&mut world, GEMS_OID, S_GEMSTONE, 19);
    drain(&mut rx);
    let body = ensoul_body(MACE_OID, &[(1, 1, STONE_OID, MIGHT)]);

    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .sitting = true;
    crate::game_loop::items::ensoul::handle_request_item_ensoul(&mut world, CID, &body);
    assert!(has_sm(
        &drain(&mut rx),
        server_packets::sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_SEATED
    ));
    assert_eq!(slots(&world, MACE_OID).0, [0, 0]);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .sitting = false;

    world
        .objects
        .get_component_mut::<Buffs>(&PLAYER)
        .unwrap()
        .0
        .push(crate::model::skill::active_buff::ActiveBuff {
            abnormal_type: "FREEZING".into(),
            ..crate::model::skill::active_buff::ActiveBuff::passive_pump(1, 1, Vec::new())
        });
    crate::game_loop::items::ensoul::handle_request_item_ensoul(&mut world, CID, &body);
    assert!(has_sm(
        &drain(&mut rx),
        server_packets::sm_ids::CANNOT_USE_THE_SOUL_CRYSTAL_SYSTEM_WHILE_FROZEN
    ));
    assert_eq!(slots(&world, MACE_OID).0, [MIGHT, 0], "and carries on");
}

/// An entry Java leaves `null` (position 3 here) throws in the loop: the
/// entry before it is already paid for and filed, and nothing is sent after.
#[test]
fn an_invalid_entry_aborts_after_the_earlier_ones_landed() {
    let (mut world, mut rx) = ensoul_world();
    give(&mut world, MACE_OID, ARCANA_MACE, 1);
    give(&mut world, STONE_OID, MIGHT_STONE, 1);
    give(&mut world, GEMS_OID, S_GEMSTONE, 19);
    drain(&mut rx);

    crate::game_loop::items::ensoul::handle_request_item_ensoul(
        &mut world,
        CID,
        &ensoul_body(MACE_OID, &[(1, 1, STONE_OID, MIGHT), (1, 3, 0, 0)]),
    );
    assert_eq!(
        slots(&world, MACE_OID).0,
        [MIGHT, 0],
        "the first entry landed"
    );
    assert_eq!(ensoul_result(&drain(&mut rx)), None, "no result");
}

/// No grade on the dist has a removal fee, so extraction is a no-op there.
#[test]
fn extraction_does_nothing_without_a_removal_fee() {
    let (mut world, mut rx) = ensoul_world();
    give(&mut world, MACE_OID, ARCANA_MACE, 1);
    world
        .objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .set_ensoul_options(MACE_OID, [MIGHT, 0], 0);
    drain(&mut rx);

    crate::game_loop::items::ensoul::handle_request_try_ensoul_extraction(
        &mut world,
        CID,
        &extraction_body(MACE_OID, 1, 1),
    );
    assert_eq!(slots(&world, MACE_OID).0, [MIGHT, 0]);
    assert!(drain(&mut rx).is_empty());
}

/// With a fee supplied: extracting regular slot 0 takes the fee, returns the
/// stone, and moves slot 1 down. On a worn weapon the moved option's skill is
/// removed and not re-granted (Java's quirk).
#[test]
fn extracting_slot_zero_returns_the_stone_and_moves_slot_one_down() {
    use crate::data::item_data::kinds::CrystalType;
    let (mut world, mut rx) = ensoul_world();
    world
        .data
        .ensoul
        .insert_removal_fee_for_test(CrystalType::R99, vec![(57, 100)]);
    set_expertise(&mut world, 10);
    give(&mut world, MACE_OID, HELIOS_SHAPER, 1);
    give(&mut world, GEMS_OID, 57, 100);
    world
        .objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .set_ensoul_options(MACE_OID, [MIGHT, FOCUS], 0);
    items::handle_use_item(&mut world, CID, &use_item_body(MACE_OID));
    let moved_skill = world.data.ensoul.option(FOCUS).unwrap().skill_id;
    assert_eq!(option_skill(&world, MIGHT_SKILL), Some(1));
    assert!(option_skill(&world, moved_skill).is_some());
    drain(&mut rx);

    crate::game_loop::items::ensoul::handle_request_try_ensoul_extraction(
        &mut world,
        CID,
        &extraction_body(MACE_OID, 1, 1),
    );
    assert_eq!(slots(&world, MACE_OID).0, [FOCUS, 0]);
    assert_eq!(count(&world, 57), 0);
    assert_eq!(
        count(&world, MIGHT_STONE),
        1,
        "the lowest stone listing Might"
    );
    assert_eq!(option_skill(&world, MIGHT_SKILL), None);
    assert_eq!(option_skill(&world, moved_skill), None, "not re-granted");
}

/// Focus (skill 13858) — a second regular option, on a different skill.
const FOCUS: i32 = 31;

/// `EnsoulWindow` bypass: both verbs open their window, matched as Java's
/// handler table matches, without regard to case.
#[test]
fn the_ensoul_bypasses_open_their_windows() {
    for (command, sub_opcode) in [
        ("show_ensoul_window", 0x180i16),
        ("SHOW_EXTRACT_ENSOUL_WINDOW", 0x1B4),
    ] {
        let (mut world, ..) = test_world();
        add_test_npc(&mut world, NPC_OID, 30678, "Folk", 40, 100, 0, 0);
        let mut rx = ingame_player(&mut world, CID, PLAYER, 60, 0, 0);
        drain(&mut rx);
        handle_request_bypass_to_server(
            &mut world,
            CID,
            &bypass_body(&format!("npc_{NPC_OID}_{command}")),
        );
        let sent = drain(&mut rx);
        assert!(
            sent.iter().any(|p| p.len() == 3
                && p[0] == 0xFE
                && i16::from_le_bytes([p[1], p[2]]) == sub_opcode),
            "{command}"
        );
    }
}

// ---------------------------------------------------------------------------
// Multisell: the display, the echo check and the product's options
// ---------------------------------------------------------------------------

const MS_LIST: i32 = 990_601;
const ENSOULED_SPECIAL: i32 = TYRR;

/// A `maintainEnchantment` list trading the Arcana Mace for `product`.
fn upgrade_list(world: &mut World, product: i32) {
    use crate::data::multisell_data::{Ingredient, MultisellEntry, MultisellList, Product};
    world.data.multisells.insert_for_test(MultisellList {
        list_id: MS_LIST,
        is_chance_multisell: false,
        apply_taxes: false,
        maintain_enchantment: true,
        ingredient_multiplier: 1.0,
        product_multiplier: 1.0,
        entries: vec![MultisellEntry {
            ingredients: vec![Ingredient {
                id: ARCANA_MACE,
                count: 1,
                enchant_level: 0,
                maintain: false,
            }],
            products: vec![Product {
                id: product,
                count: 1,
                chance: None,
                enchant_level: 0,
            }],
            stackable: false,
        }],
        npcs_allowed: None,
    });
}

/// `MultiSellChoose` echoing an unenchanted, unaugmented item with the given
/// soul crystal options.
fn choose_with_options(regular: &[i32], special: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(MS_LIST);
    w.write_i32(1);
    w.write_i64(1);
    w.write_i16(0);
    w.write_i32(0);
    w.write_i32(0);
    for value in server_packets::NO_ATTRIBUTES {
        w.write_i16(value);
    }
    for list in [regular, special] {
        w.write_u8(list.len() as u8);
        for &id in list {
            w.write_i32(id);
        }
    }
    w.into_bytes()
}

/// The ensouled mace in the bag and an inventory-only window open on it.
fn open_upgrade(world: &mut World, rx: &mut UnboundedReceiver<bytes::Bytes>, product: i32) {
    upgrade_list(world, product);
    give_ensouled_mace(world);
    drain(rx);
    crate::game_loop::commerce::multisell::separate_and_send(
        world, CID, PLAYER, None, MS_LIST, true,
    );
}

fn product_slots(world: &World, item_id: i32) -> Option<([i32; 2], i32)> {
    world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .and_then(|inv| inv.first_of_item(item_id))
        .map(|it| (it.ensoul_options, it.ensoul_special_option))
}

/// The window shows the paired item's options on the row, and the product
/// inherits what the client echoes back.
#[test]
fn a_maintain_enchantment_upgrade_carries_the_soul_crystals() {
    const TALLUM_DLE: i32 = 6580;
    let (mut world, mut rx) = ensoul_world();
    open_upgrade(&mut world, &mut rx, TALLUM_DLE);
    let list = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST)
        .expect("the window");
    let mut options = vec![1u8];
    options.extend_from_slice(&MIGHT.to_le_bytes());
    options.push(1);
    options.extend_from_slice(&ENSOULED_SPECIAL.to_le_bytes());
    assert!(
        list.windows(options.len()).any(|w| w == options.as_slice()),
        "the row shows the mace's options"
    );

    crate::game_loop::commerce::multisell::handle_multi_sell_choose(
        &mut world,
        CID,
        &choose_with_options(&[MIGHT], &[ENSOULED_SPECIAL]),
    );
    assert_eq!(count(&world, ARCANA_MACE), 0, "the mace was traded in");
    assert_eq!(
        product_slots(&world, TALLUM_DLE),
        Some(([MIGHT, 0], ENSOULED_SPECIAL))
    );
}

/// The echo must include every option the item has; leaving one out refuses.
#[test]
fn an_echo_missing_an_option_is_refused() {
    let (mut world, mut rx) = ensoul_world();
    open_upgrade(&mut world, &mut rx, 6580);
    crate::game_loop::commerce::multisell::handle_multi_sell_choose(
        &mut world,
        CID,
        &choose_with_options(&[], &[ENSOULED_SPECIAL]),
    );
    assert_eq!(count(&world, ARCANA_MACE), 1, "nothing traded");
}

/// **Java's one-way match, kept:** an echo with an extra option passes, and
/// the product gets the extra too. The Helios Shaper has two regular slots,
/// so Focus lands even though the mace never had it.
#[test]
fn an_echo_with_an_extra_option_passes_and_the_product_gets_it() {
    let (mut world, mut rx) = ensoul_world();
    open_upgrade(&mut world, &mut rx, HELIOS_SHAPER);
    crate::game_loop::commerce::multisell::handle_multi_sell_choose(
        &mut world,
        CID,
        &choose_with_options(&[MIGHT, FOCUS], &[ENSOULED_SPECIAL]),
    );
    assert_eq!(count(&world, ARCANA_MACE), 0);
    assert_eq!(
        product_slots(&world, HELIOS_SHAPER),
        Some(([MIGHT, FOCUS], ENSOULED_SPECIAL))
    );
}

/// An echoed id that names no option aborts before anything else.
#[test]
fn an_unknown_echoed_option_closes_the_window() {
    let (mut world, mut rx) = ensoul_world();
    open_upgrade(&mut world, &mut rx, 6580);
    crate::game_loop::commerce::multisell::handle_multi_sell_choose(
        &mut world,
        CID,
        &choose_with_options(&[MIGHT, 99_999], &[ENSOULED_SPECIAL]),
    );
    assert_eq!(count(&world, ARCANA_MACE), 1);
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::commerce::ActiveMultisell>(&PLAYER)
    );
}

/// On a normal window Java's display stand-in is the first unequipped
/// equipable ingredient: the row still shows the mace's options, though the
/// choose neither checks them nor carries them.
#[test]
fn a_plain_maintain_window_shows_the_stand_in_item() {
    let (mut world, mut rx) = ensoul_world();
    upgrade_list(&mut world, 6580);
    give_ensouled_mace(&mut world);
    drain(&mut rx);
    crate::game_loop::commerce::multisell::separate_and_send(
        &mut world, CID, PLAYER, None, MS_LIST, false,
    );
    let list = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST)
        .expect("the window");
    let mut options = vec![1u8];
    options.extend_from_slice(&MIGHT.to_le_bytes());
    assert!(list.windows(options.len()).any(|w| w == options.as_slice()));
}
