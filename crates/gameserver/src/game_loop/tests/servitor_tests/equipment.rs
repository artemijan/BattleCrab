//! Pet gear: wearing armour, the species gate on what it may equip, and
//! fetching a ground item into its own bag.

use super::*;

const WOLF_ARMOR: i32 = 3891;

/// Register Wolf's Hide Armor — a real chest-slot pet armour with defence.
fn register_pet_armor(world: &mut World) {
    let mut t = crate::data::item_data::template::ItemTemplate::default();
    t.item_id = WOLF_ARMOR;
    t.name = "Wolf's Hide Armor".into();
    t.kind = crate::data::item_data::kinds::ItemKind::Armor;
    t.body_part = crate::data::item_data::SLOT_CHEST;
    // As the real 3891 declares it: the pet window refuses anything else.
    t.for_npc = true;
    world.data.item_data.insert_for_test(t);
    world
        .data
        .item_data
        .insert_stats_for_test(WOLF_ARMOR, vec![(Stat::PhysicalDefence, 31.0)]);
}

fn give_pet_armor(world: &mut World) -> i32 {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .add_item(&data.item_data, 7_600_001, WOLF_ARMOR, 1)
}

/// A pet's armour goes on its **own** paperdoll, and its defence counts.
#[test]
fn a_pet_can_wear_armour_and_gains_its_defence() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_pet_armor(&mut world);
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);

    let before = world
        .objects
        .get_component::<CombatStats>(&pet_oid)
        .unwrap()
        .p_def;
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);

    assert!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .paperdoll_slot_of(armor)
            .is_some(),
        "the armour is worn"
    );
    let after = world
        .objects
        .get_component::<CombatStats>(&pet_oid)
        .unwrap()
        .p_def;
    assert!(
        after > before,
        "and its defence counts ({before} → {after})"
    );
}

/// Clicking a worn item takes it off again (Java `useEquippableItem` toggles),
/// and the defence goes with it.
#[test]
fn clicking_worn_pet_armour_takes_it_off() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_pet_armor(&mut world);
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);

    let naked = world
        .objects
        .get_component::<CombatStats>(&pet_oid)
        .unwrap()
        .p_def;
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);

    assert!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .paperdoll_slot_of(armor)
            .is_none(),
        "taken off"
    );
    assert_eq!(
        world
            .objects
            .get_component::<CombatStats>(&pet_oid)
            .unwrap()
            .p_def,
        naked,
        "and the defence went with it"
    );
}

/// Worn pet armour persists as `PET_EQUIP`, carried items as `PET` — and the
/// slot survives the round trip, so a pet's armour comes back **on** rather
/// than loose in its bag. This closes the deferral slice 8 left behind.
#[test]
fn pet_equipment_round_trips_through_its_own_location() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_pet_armor(&mut world);
    register_food(&mut world, 100);
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);
    put_food_in_pet(&mut world, 3);
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);

    let rows = world
        .objects
        .get_component::<PetInventory>(&OWNER)
        .unwrap()
        .to_rows();
    let worn = rows
        .iter()
        .find(|r| r.item_id == WOLF_ARMOR)
        .expect("armour row");
    let carried = rows
        .iter()
        .find(|r| r.item_id == WOLF_FOOD)
        .expect("food row");
    assert_eq!(worn.loc, "PET_EQUIP", "worn gear gets its own location");
    assert_ne!(worn.loc_data, 0, "and keeps the slot it was in");
    assert_eq!(carried.loc, "PET", "carried items stay in the bag");

    // Back again.
    let restored = PetInventory::from_rows(&rows);
    assert!(
        restored.0.paperdoll_slot_of(worn.object_id).is_some(),
        "the pet's armour comes back on, not loose in its bag"
    );
}

// ---------------------------------------------------------------------------
// Reconnect resummon (slice 26)
// ---------------------------------------------------------------------------

/// **`RequestPetGetItem` (0x98) walks the pet to the item, then puts it in the
/// pet's own bag.**
///
/// The two-stage shape is the point: the order only sets the errand, and the
/// lift happens on a later think once the pet is within 36 units. A test that
/// dropped the item under the pet's feet would pass without the walk ever
/// working.
#[test]
fn a_pet_fetches_a_ground_item_into_its_own_bag() {
    use crate::game_loop::items::ground_items::{DropSource, spawn_ground_item};
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();

    // Adena on the floor, well out of reach.
    let item = spawn_ground_item(&mut world, 57, 100, 0, 600, 0, 0, 0, DropSource::Npc);

    let mut body = vec![cop::REQUEST_PET_GET_ITEM];
    body.extend_from_slice(&item.to_le_bytes());
    on_packet(&mut world, CID, body);

    // The errand is running and the pet stopped trailing its owner.
    assert!(
        world
            .objects
            .has_component::<model::components::summons::SummonPickup>(&pet),
        "the fetch order is pending"
    );
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&pet)
            .unwrap()
            .following,
        "and the pet stopped following"
    );
    assert!(
        world.objects.get_component::<Movement>(&pet).is_some(),
        "it is walking to the item"
    );

    // Put the pet on top of the item and think again — now it lifts.
    {
        let p = world.objects.get_component_mut::<Position>(&pet).unwrap();
        p.x = 600;
        p.y = 0;
    }
    crate::game_loop::servitor::pet_pickup_think(&mut world, pet);

    assert!(
        !world
            .objects
            .has_component::<model::components::summons::SummonPickup>(&pet),
        "the errand is over"
    );
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&pet)
            .unwrap()
            .following,
        "and the pet trails its owner again"
    );
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(57),
        100,
        "the adena is in the pet's bag, not the owner's"
    );
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item),
        "and off the floor"
    );
}

/// **A starving pet refuses the errand and says why.** The one guard in this
/// packet that sends a message rather than a bare `ActionFailed`.
#[test]
fn a_starving_pet_will_not_fetch() {
    use crate::game_loop::items::ground_items::{DropSource, spawn_ground_item};
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();
    // `isUncontrollable()` — the hunger gauge at zero.
    world.objects.get_component_mut::<PetOf>(&pet).unwrap().fed = 0;
    let item = spawn_ground_item(&mut world, 57, 100, 0, 10, 0, 0, 0, DropSource::Npc);
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_PET_GET_ITEM];
    body.extend_from_slice(&item.to_le_bytes());
    on_packet(&mut world, CID, body);

    assert!(
        !world
            .objects
            .has_component::<model::components::summons::SummonPickup>(&pet),
        "no errand was taken"
    );
    assert!(
        has_sm(
            &drain(&mut rx),
            server_packets::sm_ids::WHEN_YOUR_PETS_HUNGER_GAUGE_IS_AT_0_YOU_CANNOT_USE_YOUR_PET
        ),
        "and the owner is told why"
    );
}

// ---------------------------------------------------------------------------
// Item conditions on the pet window (`ItemTemplate.checkCondition` with the
// **pet** as the effector).
// ---------------------------------------------------------------------------

/// A pet armour gated on a category, the way every real one is.
fn register_gated_pet_armor(world: &mut World, category: &str) {
    use crate::data::item_cond::{Cond, CondMessage, ItemCondition};
    let mut t = crate::data::item_data::template::ItemTemplate::default();
    t.item_id = WOLF_ARMOR;
    t.name = "Gated Hide Armor".into();
    t.kind = crate::data::item_data::kinds::ItemKind::Armor;
    t.body_part = crate::data::item_data::SLOT_CHEST;
    t.for_npc = true;
    t.pre_conditions = vec![ItemCondition {
        node: Cond::CategoryType(vec![category.to_string()]),
        message: CondMessage::Sm {
            id: 1518,
            add_name: false,
        },
    }];
    world.data.item_data.insert_for_test(t);
}

fn pet_use_item(world: &mut World, item_object_id: i32) {
    crate::game_loop::servitor::handle_pet_use_item(world, CID, &item_object_id.to_le_bytes());
}

fn pet_wears(world: &World, item_object_id: i32) -> bool {
    world
        .objects
        .get_component::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .paperdoll_slot_of(item_object_id)
        .is_some()
}

/// `ConditionCategoryType` reads `Creature.getId()`, which for a summon is its
/// **npc** id — not the owner's class id. That is the whole mechanism behind
/// `categoryType="STRIDER"` on a saddle: the wearer is the pet.
#[test]
fn pet_gear_is_gated_on_the_pets_own_species() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_gated_pet_armor(&mut world, "STRIDER_GROUP");
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);
    drain(&mut rx);

    // The Wolf is not a strider.
    pet_use_item(&mut world, armor);
    assert!(!pet_wears(&world, armor), "wrong species, refused");
    assert!(
        has_system_message(
            &drain(&mut rx),
            crate::network::server_packets::sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM
        ),
        "a summon gets its own line, not the block's message"
    );

    // Put the Wolf's npc id in the group and the same saddle goes on.
    world
        .data
        .categories
        .insert_for_test("STRIDER_GROUP", &[WOLF_NPC]);
    pet_use_item(&mut world, armor);
    assert!(pet_wears(&world, armor));
    let _ = pet_oid;
}

/// `RequestPetUseItem.useItem` refuses an equippable item that carries **no**
/// conditions at all: pet gear is defined by being gated. Without this leg the
/// pet window would happily wear a player's helmet.
#[test]
fn an_ungated_equippable_item_is_not_pet_gear() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_pet_armor(&mut world); // no `<cond>` on this one
    summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);
    drain(&mut rx);

    pet_use_item(&mut world, armor);
    assert!(!pet_wears(&world, armor));
    assert!(has_system_message(
        &drain(&mut rx),
        crate::network::server_packets::sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM
    ));
}

/// `PetInventory.restore`'s "check for equipped items from other pets": the
/// owner keeps one item store, so gear worn by the last pet is re-judged
/// against the one being summoned now.
#[test]
fn a_summoned_pet_sheds_gear_that_belonged_to_another_species() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_gated_pet_armor(&mut world, "STRIDER_GROUP");
    world
        .data
        .categories
        .insert_for_test("STRIDER_GROUP", &[WOLF_NPC]);
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);
    assert!(pet_wears(&world, armor), "worn while the species matched");

    // The wolf stops being a strider — as it would be by putting the saddle on
    // a wolf after a strider took it off — and is summoned again.
    unsummon_servitor(&mut world, OWNER);
    world.data.categories.insert_for_test("STRIDER_GROUP", &[]);
    let collar = crate::game_loop::servitor::active_pet_collar(&world, OWNER)
        .or_else(|| {
            world
                .objects
                .get_component::<Inventory>(&OWNER)
                .unwrap()
                .items()
                .first()
                .map(|i| i.object_id)
        })
        .expect("collar");
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).expect("re-summoned");

    assert!(
        !pet_wears(&world, armor),
        "the saddle came off at summon time"
    );
}

/// `for_npc` is Java's **first** gate on the pet window — 508 items declare it,
/// and anything else is refused before the conditions are even looked at.
#[test]
fn only_a_for_npc_item_reaches_the_pet_at_all() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_gated_pet_armor(&mut world, "STRIDER_GROUP");
    world
        .data
        .categories
        .insert_for_test("STRIDER_GROUP", &[WOLF_NPC]);
    // …but the item is not declared as pet gear.
    let mut t = world.data.item_data.get(WOLF_ARMOR).unwrap().clone();
    t.for_npc = false;
    world.data.item_data.insert_for_test(t);
    summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);
    drain(&mut rx);

    pet_use_item(&mut world, armor);
    assert!(
        !pet_wears(&world, armor),
        "the species matches, but the item is not for an npc"
    );
    assert!(has_system_message(
        &drain(&mut rx),
        crate::network::server_packets::sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM
    ));
}
