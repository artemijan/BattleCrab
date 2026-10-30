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
