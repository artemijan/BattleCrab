//! Feeding: the food tick, eating from its own bag, transferring food in and
//! out, hand feeding, the starving gates, and //fullfood.

use super::*;

/// A food bar saved above the level's capacity is clamped, not carried —
/// otherwise a datapack nerf would leave pets permanently over-full.
#[test]
fn restored_fed_is_clamped_to_max_meal() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    put_saved(&mut world, saved_row(collar, 1, 0, 9_999, 42.0));

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.fed, 248, "clamped to level 1's max_meal");
}

/// A summoned pet burns food on every tick — the drain that makes feeding
/// necessary at all.
#[test]
fn the_feed_tick_burns_food() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    assert_eq!(fed(&world, pet_oid), 248, "starts full");

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    assert_eq!(fed(&world, pet_oid), 238, "one normal-rate helping burned");
}

/// The bar is not allowed below zero — Java's `fed > consume ? fed - consume : 0`.
#[test]
fn the_feed_tick_floors_at_zero() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 4;

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    assert_eq!(
        fed(&world, pet_oid),
        0,
        "cost exceeded the bar — floored, not negative"
    );
    assert!(
        crate::game_loop::servitor::is_uncontrollable(&world, pet_oid),
        "an empty bar means starving"
    );
}

/// A hungry pet with food in *its own* inventory eats without being told.
/// `hungry_limit` is 55%, so the bar must be under 136 for this to fire.
#[test]
fn a_hungry_pet_eats_from_its_own_inventory() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 2);
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 100;

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    // 100 - 10 burned = 90, hungry (< 136), so it eats one 100-point helping.
    assert_eq!(fed(&world, pet_oid), 190, "burned 10, then ate 100");
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        1,
        "exactly one helping consumed"
    );
}

/// A pet that is not hungry leaves its food alone — otherwise a full bar would
/// eat through the whole stack.
#[test]
fn a_full_pet_does_not_eat() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 2);

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        2,
        "full pet leaves the stack alone"
    );
}

/// Feeding is capped at the level's `max_meal` — Java's `setCurrentFed` clamp.
/// Measured from a bar with room in it, so the clamp is what's under test
/// rather than an already-full bar.
#[test]
fn feeding_is_capped_at_max_meal() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 200;

    crate::game_loop::servitor::apply_feed(&mut world, pet_oid, 100);
    assert_eq!(
        fed(&world, pet_oid),
        248,
        "200 + 100 clamped to max_meal, not banked"
    );
}

/// Food reaches the pet by transfer from the owner — the client's only route,
/// since Java's `PetFood` handler refuses an unmounted player.
#[test]
fn food_transfers_to_the_pet_and_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let _ = summon_pet(&mut world, OWNER).unwrap();

    let food_oid = {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&OWNER)
            .unwrap()
            .add_item(&data.item_data, 7_300_001, WOLF_FOOD, 5)
    };

    let mut body = Vec::new();
    body.extend_from_slice(&food_oid.to_le_bytes());
    body.extend_from_slice(&3i64.to_le_bytes());
    crate::game_loop::servitor::handle_give_item_to_pet(&mut world, CID, &body);

    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        3
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&OWNER)
            .unwrap()
            .count_of(WOLF_FOOD),
        2,
        "the owner keeps the remainder"
    );

    // And back again.
    let pet_food_oid = world
        .objects
        .get_component::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .items()[0]
        .object_id;
    let mut body = Vec::new();
    body.extend_from_slice(&pet_food_oid.to_le_bytes());
    body.extend_from_slice(&3i64.to_le_bytes());
    crate::game_loop::servitor::handle_get_item_from_pet(&mut world, CID, &body);
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        0
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&OWNER)
            .unwrap()
            .count_of(WOLF_FOOD),
        5,
        "all five back with the owner"
    );
}

/// Manual feeding through the pet window, and the refusal for anything the
/// species does not eat.
#[test]
fn the_owner_can_feed_the_pet_by_hand() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 1);
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 50;

    let food_oid = world
        .objects
        .get_component::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .items()[0]
        .object_id;
    let body = food_oid.to_le_bytes().to_vec();
    crate::game_loop::servitor::handle_pet_use_item(&mut world, CID, &body);

    assert_eq!(fed(&world, pet_oid), 150, "hand-fed one helping");
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        0
    );
}

/// A pet only eats its own species' food (Java `canEatFoodId`).
#[test]
fn a_pet_refuses_food_it_does_not_eat() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 50;

    // A different item entirely, sitting in the pet's bag.
    insert_adena_template(&mut world);
    let oid = {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .add_item(&data.item_data, 7_400_001, 57, 1)
    };

    let body = oid.to_le_bytes().to_vec();
    crate::game_loop::servitor::handle_pet_use_item(&mut world, CID, &body);

    assert_eq!(fed(&world, pet_oid), 50, "bar untouched");
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(57),
        1,
        "and the item is not consumed"
    );
}

/// The fixture above uses a hand-built skill, so it cannot catch a parse-arm
/// mistake. This reads the **real** Wolf Food skill out of the datapack: if
/// `<effect name="Feed"><normal>100</normal>` stops reaching `SkillEffect::Feed`,
/// every pet food in the game silently restores nothing.
#[test]
fn the_real_wolf_food_skill_parses_its_feed_value() {
    let skills = dist::skills();
    let skill = skills
        .get(2048, 1)
        .expect("Wolf Food skill 2048 exists in the datapack");
    let feed = skill
        .effects
        .iter()
        .find_map(|e| match e {
            SkillEffect::Feed { normal, .. } => Some(*normal),
            _ => None,
        })
        .expect("Wolf Food carries a Feed effect");
    assert_eq!(feed, 100, "the <normal> value from 2048");
}

// ---------------------------------------------------------------------------
// Client-visibility gaps (slice 10)
// ---------------------------------------------------------------------------

/// `//fullfood` fills the targeted pet's bar. Java gates on `isPet()`, which a
/// skill-summoned servitor fails: its `PetInfo` fed slot carries its remaining
/// lifetime, not food, so filling it would be meaningless.
#[test]
fn fullfood_fills_a_pet_and_refuses_a_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");

    // Drain the bar, then target the pet and fill it.
    {
        let p = world.objects.get_component_mut::<PetOf>(&pet).unwrap();
        p.fed = 1;
    }
    // `use_admin_command` returns silently for a non-GM (Java `if (!isGM())`),
    // and `is_gm` resolves the level through `AdminData` — which the synthetic
    // test world loads *empty*, so the real table is needed for level 70 to
    // mean anything.
    world.data.admin = crate::data::AdminData::load_from(DIST);
    world
        .objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        // `AdminCommands.xml` puts `admin_fullfood` at accessLevel **100**
        // ("Master"), not 70 — a level-70 GM is refused.
        .access_level = 100;
    world.objects.add_components(&OWNER, TargetRef(Some(pet)));
    crate::game_loop::admin::use_admin_command(&mut world, CID, "admin_fullfood", false);

    let p = world.objects.get_component::<PetOf>(&pet).unwrap();
    assert_eq!(p.fed, p.max_fed, "the bar is filled to max");

    // A servitor is not a pet: the command must not touch it.
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    world
        .objects
        .add_components(&OWNER, TargetRef(Some(servitor)));
    crate::game_loop::admin::use_admin_command(&mut world, CID, "admin_fullfood", false);
    assert!(
        !world.objects.has_component::<PetOf>(&servitor),
        "a servitor never grows a food bar from //fullfood"
    );
}

// ---------------------------------------------------------------------------
// G34 S4 sub-slice 15 — Betray
// ---------------------------------------------------------------------------

/// `storePetFood`: riding a pet drains the shared feed gauge, and the
/// dismount writes the drained value back onto the collar's `pets` row — a
/// rider who mounts at 100 and climbs off at 37 summons a pet at 37, not at
/// the value stored when the pet was unsummoned onto the saddle.
#[test]
fn dismount_stores_the_drained_feed_on_the_collar_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    world
        .data
        .categories
        .insert_for_test("WOLF_GROUP", &[WOLF_NPC]);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    world.objects.get_component_mut::<PetOf>(&pet).unwrap().fed = 100;

    crate::game_loop::client::user_commands::mount(&mut world, CID, OWNER);
    {
        let p = world.objects.get_component::<Player>(&OWNER).unwrap();
        assert!(p.is_mounted(), "the wolf was ridden");
        assert_eq!(
            p.mount_collar_object_id, collar,
            "the collar link rides along"
        );
        assert_eq!(p.mount_feed, 100, "the pet's food carried onto the gauge");
    }

    // The ride drains the gauge…
    world
        .objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .mount_feed = 37;
    crate::game_loop::admin::mounts::dismount(&mut world, OWNER);

    let p = world.objects.get_component::<Player>(&OWNER).unwrap();
    assert!(!p.is_mounted());
    assert_eq!(p.mount_collar_object_id, 0, "the link cleared");
    assert_eq!(
        world.objects.get_component::<PlayerPets>(&OWNER).unwrap().0[&collar].fed,
        37,
        "the drained gauge went back onto the pets row"
    );
}

// ---------------------------------------------------------------------------
// The pet/servitor window orders (`handlers/playeractions/*`)
// ---------------------------------------------------------------------------

/// `Pet.isUncontrollable()` — a pet whose hunger gauge has hit 0 ignores every
/// order and says so.
#[test]
fn a_starving_pet_ignores_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);
    world.objects.add_components(&OWNER, TargetRef(Some(FOE)));
    world.objects.get_component_mut::<PetOf>(&pet).unwrap().fed = 0;
    drain(&mut rx);

    handle_pet_action(&mut world, CID, OWNER, "PetAttack", 0);

    assert_eq!(hate_for(&world, pet, FOE), 0.0, "the order is refused");
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE),
        "and the owner is told why"
    );
}
