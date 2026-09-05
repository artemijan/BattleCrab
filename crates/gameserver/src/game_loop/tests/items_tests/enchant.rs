//! Enchanting: the scroll's success and failure, support items, the
//! random-range roll, and the punishment for pressing too fast.

use super::*;

/// Full enchant flow with real data: use scroll → add scroll → put target →
/// enchant. Success bumps +1; a forced failure at +4 destroys the weapon and
/// returns crystals.
#[test]
fn enchant_scroll_success_and_failure() {
    use crate::model::components::commerce::EnchantRequest;
    use crate::model::inventory::Inventory;
    const DIST: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(DIST);
    world.id_pool = 0x4000_0000..0x4000_0200;

    // Scroll: Enchant Weapon (D-grade) 955; Bastard Sword 69 (D weapon, enchantable).
    let sword_cc = world.data.item_data.get(69).unwrap().crystal_count;
    let crystal_id = world
        .data
        .item_data
        .get(69)
        .unwrap()
        .crystal_type
        .crystal_item_id()
        .unwrap();

    let mut rx = ingame_player_access(&mut world, 1, 9800, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, 9800, 955, 5).unwrap();
    inventory::add_inventory_item(&mut world, 9800, 69, 1).unwrap();
    let scroll_oid = item_oid(&world, 9800, 955);
    let sword_oid = item_oid(&world, 9800, 69);

    // Use the scroll → opens the enchant request.
    let use_item = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll_oid);
        w.write_i32(0);
        w.into_bytes()
    };
    on_packet(&mut world, 1, use_item);
    assert!(
        world.objects.has_component::<EnchantRequest>(&9800),
        "enchant window opened"
    );

    let add_scroll = {
        let mut w = PacketWriter::new();
        w.write_i32(scroll_oid);
        w.write_i32(sword_oid);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
            &add_scroll,
        ),
    );
    let put_target = {
        let mut w = PacketWriter::new();
        w.write_i32(sword_oid);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
            &put_target,
        ),
    );

    // +0 weapon is a guaranteed (100%) success → +1.
    let do_enchant = |oid: i32| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(oid);
        w.write_i32(0);
        w.into_bytes()
    };
    // Java's anti-autoenchant guard punishes an Enchant pressed within 2 s of
    // the last window interaction, so the window has to age before the press.
    world.tick += 20;
    world.force_roll(0); // roll_f64 = 0.0 < 100
    on_packet(&mut world, 1, do_enchant(sword_oid));
    let level = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9800)
            .unwrap()
            .by_object_id(sword_oid)
            .map(|it| it.enchant_level)
    };
    assert_eq!(level(&world), Some(1), "success: +0 → +1");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9800)
            .unwrap()
            .count_of(955),
        4,
        "one scroll consumed"
    );

    // Bump to +4 (66.67% group chance), then force a failing roll (90%) →
    // weapon destroyed, crystals returned.
    world
        .objects
        .get_component_mut::<Inventory>(&9800)
        .unwrap()
        .set_item_enchant(sword_oid, 4);
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
            &add_scroll,
        ),
    );
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
            &put_target,
        ),
    );
    world.tick += 20;
    world.force_roll(900_000); // roll_f64 = 90.0 > 66.67 → fail
    on_packet(&mut world, 1, do_enchant(sword_oid));
    let inv = world.objects.get_component::<Inventory>(&9800).unwrap();
    assert_eq!(inv.count_of(69), 0, "failed enchant destroyed the sword");
    let expected_crystals = (sword_cc - (sword_cc + 1) / 2).max(0) as i64;
    assert_eq!(
        inv.count_of(crystal_id),
        expected_crystals,
        "crystals returned on break"
    );
    assert_eq!(inv.count_of(955), 3, "second scroll consumed");
}

/// Enchant with a support item: its +20 bonus rate flips a roll that would miss
/// the bare 66.67% group chance at +3, and the support is consumed.
#[test]
fn enchant_support_item_bonus_and_consume() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9850, 0);
    drain(&mut rx);

    // Bastard Sword 69 (D weapon), Enchant Weapon D scroll 955, and the D-grade
    // weapon support "Lucky Enchant Stone" 12362 (+20 bonus, valid at +3..9).
    inventory::add_inventory_item(&mut world, 9850, 955, 1).unwrap();
    inventory::add_inventory_item(&mut world, 9850, 69, 1).unwrap();
    inventory::add_inventory_item(&mut world, 9850, 12362, 1).unwrap();
    let (scroll, sword, support) = (
        item_oid(&world, 9850, 955),
        item_oid(&world, 9850, 69),
        item_oid(&world, 9850, 12362),
    );
    // The support requires the target already at +3.
    world
        .objects
        .get_component_mut::<Inventory>(&9850)
        .unwrap()
        .set_item_enchant(sword, 3);

    let use_scroll = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll);
        w.write_i32(0);
        w.into_bytes()
    };
    on_packet(&mut world, 1, use_scroll);
    let add_scroll = {
        let mut w = PacketWriter::new();
        w.write_i32(scroll);
        w.write_i32(sword);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
            &add_scroll,
        ),
    );
    let put_target = {
        let mut w = PacketWriter::new();
        w.write_i32(sword);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
            &put_target,
        ),
    );
    // Support: body is (supportObjId, enchantObjId).
    let put_support = {
        let mut w = PacketWriter::new();
        w.write_i32(support);
        w.write_i32(sword);
        w.into_bytes()
    };
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_SUPPORT_ITEM,
            &put_support,
        ),
    );
    let put_out = drain(&mut rx);
    assert!(
        put_out.iter().any(|p| is_ex(
            p,
            server_packets::opcodes::EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT
        )),
        "support accepted"
    );

    // Roll 80%: bare chance 66.67 would fail, but +20 support → 86.67 succeeds.
    // Age the window past Java's 2 s anti-autoenchant guard first.
    world.tick += 20;
    world.force_roll(800_000);
    let enchant = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(sword);
        w.write_i32(support);
        w.into_bytes()
    };
    on_packet(&mut world, 1, enchant);

    let inv = world.objects.get_component::<Inventory>(&9850).unwrap();
    let level = inv.by_object_id(sword).unwrap().enchant_level;
    assert_eq!(level, 4, "support bonus carried the +3 → +4 enchant");
    assert_eq!(inv.count_of(12362), 0, "support consumed");
    assert_eq!(inv.count_of(955), 0, "scroll consumed");
}

/// `randomEnchantMin`/`Max` on the **scroll** — the success step is a roll over
/// an inclusive range, not a flat `+1`.
///
/// Java `RequestEnchantItem`'s SUCCESS arm is
/// `Rnd.get(randomEnchantMin, randomEnchantMax)` capped at `maxEnchant`. The
/// port had this on the support side only and hard-coded the scroll side to
/// `+1`, which is correct for every scroll that omits the attributes (Java
/// defaults min to 1 and max to min) and wrong for the 20 that carry them.
///
/// Driven with 33808 "Giant's Scroll: Enchant Weapon (B-grade)" — `min 1 max 3`
/// — because it is the one a player can actually obtain here: Q375 Whisper of
/// Dreams Part 2 rewards it, and this port ships that quest.
#[test]
fn a_scroll_with_a_random_range_rolls_its_enchant_step() {
    use crate::model::inventory::Inventory;
    const DIST: &str = crate::data::DIST_GAME;
    const SCROLL: i32 = 33808; // Giant's Scroll: Enchant Weapon (B-grade)
    const SWORD: i32 = 78; // Great Sword — B-grade weapon
    const PLAYER: i32 = 9801;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(DIST);
    world.id_pool = 0x4100_0000..0x4100_0200;

    // The range really is in the dist, and really is a range — a scroll whose
    // min == max would make every assertion below pass for the wrong reason.
    let tpl = world.data.enchant.scroll(SCROLL).expect("33808 loaded");
    assert_eq!((tpl.random_min, tpl.random_max), (1, 3));

    let mut rx = ingame_player_access(&mut world, 1, PLAYER, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, PLAYER, SCROLL, 5).unwrap();
    inventory::add_inventory_item(&mut world, PLAYER, SWORD, 1).unwrap();
    let scroll_oid = item_oid(&world, PLAYER, SCROLL);
    let sword_oid = item_oid(&world, PLAYER, SWORD);
    let level = |w: &World| {
        w.objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .by_object_id(sword_oid)
            .map(|it| it.enchant_level)
            .unwrap()
    };

    let arm = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
        let mut w = PacketWriter::new();
        w.write_i32(scroll_oid);
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
                &w.into_bytes(),
            ),
        );
        let mut w = PacketWriter::new();
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
                &w.into_bytes(),
            ),
        );
    };
    let do_enchant = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(sword_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
    };

    // Roll order per attempt: the success check (`roll_f64`) consumes one
    // forced value, then the step roll consumes the next. `roll(3)` returns an
    // index in 0..3, so the step is `min + index`.
    arm(&mut world);
    // Java's anti-autoenchant guard punishes an Enchant pressed within 2 s of
    // the last window interaction, so the window has to age before the press.
    world.tick += 20;
    world.force_roll(0); // success
    world.force_roll(2); // index 2 → step 1 + 2 = 3
    do_enchant(&mut world);
    assert_eq!(level(&world), 3, "the top of the range is +3, not +1");

    arm(&mut world);
    world.tick += 20;
    world.force_roll(0); // success
    world.force_roll(0); // index 0 → step 1
    do_enchant(&mut world);
    assert_eq!(level(&world), 4, "the bottom of the range is +1");

    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .count_of(SCROLL),
        3,
        "one scroll per attempt"
    );
}

/// Java's anti-autoenchant guard: pressing Enchant within 2 s of the last
/// window interaction is treated as a bot — punished, and the attempt consumes
/// nothing.
///
/// The heuristic is coarse and deliberately so: `RequestEnchantItem` compares
/// against `AbstractRequest._timestamp`, which the four `RequestEx*Enchant*`
/// packets stamp on their success path, so it measures "time since the player
/// last touched the window" rather than anything about the enchant itself.
#[test]
fn pressing_enchant_within_two_seconds_is_punished_and_costs_nothing() {
    use crate::model::components::commerce::EnchantRequest;
    use crate::model::inventory::Inventory;
    const DIST: &str = crate::data::DIST_GAME;
    const PLAYER: i32 = 9805;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.data.enchant = crate::data::EnchantData::load_from(DIST);
    world.id_pool = 0x4200_0000..0x4200_0200;
    // Start well clear of tick 0 so "stamped at tick 0" and "never stamped"
    // cannot be confused — the bug this test caught in the first place.
    world.tick = 500;

    let mut rx = ingame_player_access(&mut world, 1, PLAYER, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, PLAYER, 955, 3).unwrap();
    inventory::add_inventory_item(&mut world, PLAYER, 69, 1).unwrap();
    let scroll_oid = item_oid(&world, PLAYER, 955);
    let sword_oid = item_oid(&world, PLAYER, 69);
    let level = |w: &World| {
        w.objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .by_object_id(sword_oid)
            .map(|it| it.enchant_level)
            .unwrap()
    };
    let scrolls_left = |w: &World| {
        w.objects
            .get_component::<Inventory>(&PLAYER)
            .unwrap()
            .count_of(955)
    };

    let arm = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::USE_ITEM);
        w.write_i32(scroll_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
        let mut w = PacketWriter::new();
        w.write_i32(scroll_oid);
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_ADD_ENCHANT_SCROLL_ITEM,
                &w.into_bytes(),
            ),
        );
        let mut w = PacketWriter::new();
        w.write_i32(sword_oid);
        on_packet(
            world,
            1,
            ex_packet(
                cp::ex_opcodes::REQUEST_EX_TRY_TO_PUT_ENCHANT_TARGET_ITEM,
                &w.into_bytes(),
            ),
        );
    };
    let press = |world: &mut World| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_ENCHANT_ITEM);
        w.write_i32(sword_oid);
        w.write_i32(0);
        on_packet(world, 1, w.into_bytes());
    };

    // Straight from arming the window to pressing Enchant: 0 ticks elapsed.
    arm(&mut world);
    drain(&mut rx);
    world.force_roll(0); // would be a guaranteed success
    press(&mut world);

    assert_eq!(level(&world), 0, "the enchant never happened");
    assert_eq!(scrolls_left(&world), 3, "and cost no scroll");
    assert!(
        !world.objects.has_component::<EnchantRequest>(&PLAYER),
        "Java drops the request on this branch, unlike a plain validation error"
    );
    assert!(
        !drain(&mut rx).is_empty(),
        "the punishment's warning line goes out"
    );
    // The forced roll was never reached — the guard returns before the roll.
    assert_eq!(
        world.forced_rolls_len(),
        1,
        "the guard bails before the success roll is drawn"
    );
    world.clear_forced_rolls();

    // One tick short of the window is still a bot. This is what pins the
    // threshold at 2 s rather than "some delay": with only the 0-tick and
    // 20-tick cases below, a guard that fired at 100 ms would pass too.
    arm(&mut world);
    world.tick += 19;
    press(&mut world);
    assert_eq!(level(&world), 0, "19 ticks (1.9 s) is inside the window");
    assert_eq!(scrolls_left(&world), 3, "still no scroll spent");

    // Wait the window out and the identical sequence succeeds, which is what
    // makes the assertions above about the guard rather than about the setup.
    arm(&mut world);
    world.tick += 20;
    world.force_roll(0);
    press(&mut world);
    assert_eq!(level(&world), 1, "past the 2 s window it enchants normally");
    assert_eq!(scrolls_left(&world), 2, "and now a scroll is consumed");
}

// ---------------------------------------------------------------------------
// Item handlers restored with row 6 (`Book`, `RollingDice`, `PetFood`)
// ---------------------------------------------------------------------------
