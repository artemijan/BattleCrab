//! `admin/items.rs` — creating, deleting and enchanting items.

use super::*;

/// `//create_item 57 1000` puts 1000 adena in the GM's inventory.
#[test]
fn admin_create_item_adds_to_gm_inventory() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7201, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 57 1000"));
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&7201)
            .unwrap()
            .count_of(57),
        1000,
        "1000 adena created"
    );
}

/// `//delete_item <objectId> [count]` trims a stack by the item's object id,
/// and a count of 0 destroys the whole stack (Java's `numval == 0`).
#[test]
fn admin_delete_item_trims_a_stack_by_object_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0200..0x4000_0300;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7211, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 57 1000"));
    fn inv(w: &World) -> &Inventory {
        w.objects.get_component::<Inventory>(&7211).unwrap()
    }
    let adena_oid = item_oid(&world, 7211, 57);

    // Partial: 400 off the 1000.
    on_packet(
        &mut world,
        1,
        build_admin(&format!("delete_item {adena_oid} 400")),
    );
    assert_eq!(inv(&world).count_of(57), 600, "400 adena destroyed");

    // Count 0 means the whole remaining stack.
    on_packet(
        &mut world,
        1,
        build_admin(&format!("delete_item {adena_oid} 0")),
    );
    assert_eq!(inv(&world).count_of(57), 0, "stack destroyed outright");
}

/// `//delete_item` on an object id nobody online owns reports it and changes
/// nothing (Java's "Item doesn't have owner." / "Player is not online.").
#[test]
fn admin_delete_item_rejects_unowned_object_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0300..0x4000_0400;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7212, 100);
    on_packet(&mut world, 1, build_admin("create_item 57 50"));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("delete_item 123456789 1"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "one message, no destruction"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&7212)
            .unwrap()
            .count_of(57),
        50,
        "inventory untouched"
    );
}

/// `//delete_quest_item <itemId> [count] [charName]`: no count clears the lot,
/// a count trims, and a trailing name overrides the target.
#[test]
fn admin_delete_quest_item_by_template_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0400..0x4000_0500;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7213, 100);
    let _p_rx = ingame_player_access(&mut world, 2, 7214, 0);
    let pname = world
        .objects
        .get_component::<Player>(&7214)
        .unwrap()
        .name
        .clone();
    world.objects.add_components(&7213, TargetRef(Some(7214)));
    inventory::add_inventory_item(&mut world, 7214, 57, 10);
    drain(&mut gm_rx);

    let held = |w: &World, oid: i32| {
        w.objects
            .get_component::<Inventory>(&oid)
            .map(|i| i.count_of(57))
            .unwrap_or(0)
    };
    assert_eq!(held(&world, 7214), 10, "target stocked");

    // A count trims the target's stack.
    on_packet(&mut world, 1, build_admin("delete_quest_item 57 4"));
    assert_eq!(held(&world, 7214), 6, "4 destroyed off the target");

    // No count clears whatever is left.
    on_packet(&mut world, 1, build_admin("delete_quest_item 57"));
    assert_eq!(held(&world, 7214), 0, "no count = all of it");

    // A trailing name wins over the target: stock the GM, aim at the player.
    inventory::add_inventory_item(&mut world, 7213, 57, 8);
    assert_eq!(held(&world, 7213), 8, "GM stocked");
    on_packet(
        &mut world,
        1,
        build_admin(&format!("delete_quest_item 57 3 {pname}")),
    );
    assert_eq!(held(&world, 7213), 8, "named player, not the GM");
    on_packet(&mut world, 1, build_admin("delete_quest_item 57 3"));
    assert_eq!(held(&world, 7213), 8, "still the target, not the GM");

    // An unheld id reports and destroys nothing.
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("delete_quest_item 2716"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "one message");
    assert_eq!(held(&world, 7213), 8, "nothing destroyed");
}

/// `//create_item` with a bogus id answers "does not exist" and adds nothing.
#[test]
fn admin_create_item_rejects_unknown_id() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7204, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("create_item 99999999 5"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "does-not-exist line"
    );
}

/// `//setew <n>` sets the enchant level of the equipped weapon.
#[test]
fn admin_setew_enchants_equipped_weapon() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8101, 100);
    drain(&mut gm_rx);
    // Equip a weapon (item 1, the starter gloves aside — any weapon id) in RHand.
    let weapon = crate::db::ItemRow {
        object_id: 50000,
        item_id: 1,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: model::inventory::PaperdollSlot::RHand as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    world
        .objects
        .add_components(&8101, Inventory::from_rows(&[weapon]));

    on_packet(&mut world, 1, build_admin("setew 10"));
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&8101)
            .unwrap()
            .paperdoll_enchant_level(model::inventory::PaperdollSlot::RHand),
        10,
        "weapon enchanted to +10"
    );
}

/// `//setew` with no weapon equipped warns.
#[test]
fn admin_setew_without_weapon_warns() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8102, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("setew 10"));
    assert_eq!(
        count_system_messages(&drain(&mut gm_rx)),
        1,
        "no-item-in-slot line"
    );
}

/// `//create_coin adena <n>` gives adena (item 57) to the GM.
#[test]
fn admin_create_coin_gives_adena() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut gm_rx = ingame_player_access(&mut world, 1, 8911, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("create_coin adena 100"));
    let inv = world.objects.get_component::<Inventory>(&8911).unwrap();
    assert_eq!(inv.count_of(57), 100, "adena added");
}
