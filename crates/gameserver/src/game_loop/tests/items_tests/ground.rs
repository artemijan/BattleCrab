//! Dropping and picking up: where an item lands, the refusals (range, zone,
//! casting, bound, seated), and how long it survives on the ground.

use super::*;

/// `RequestDestroyItem` (0x60) removes `count` of a stackable item and sends an
/// `InventoryUpdate`; a bad object id is a no-op.
#[test]
fn destroy_item_removes_from_inventory() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9100, 0);
    drain(&mut rx);

    inventory::add_inventory_item(&mut world, 9100, 57, 1000).expect("adena added");
    let inv = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9100)
            .unwrap()
            .count_of(57)
    };
    let adena_oid = item_oid(&world, 9100, 57);

    let destroy = |oid: i32, count: i64| -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_DESTROY_ITEM);
        w.write_i32(oid);
        w.write_i64(count);
        w.into_bytes()
    };

    on_packet(&mut world, 1, destroy(adena_oid, 400));
    assert_eq!(inv(&world), 600, "400 adena destroyed");
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x21),
        "InventoryUpdate sent"
    );

    // A bogus object id changes nothing.
    on_packet(&mut world, 1, destroy(0x7fff_ffff, 1));
    assert_eq!(inv(&world), 600, "unchanged");

    // Destroy the rest.
    on_packet(&mut world, 1, destroy(adena_oid, 600));
    assert_eq!(inv(&world), 0, "all adena gone");
}

/// Giving adena (the item-creation menu's "Create Coin", quest rewards) sends
/// the adena counter (`ExAdenaInvenCount` 0x13E) and weight bar
/// (`ExUserInfoInvenWeight` 0x166) alongside the `InventoryUpdate`, matching
/// Java `Player.sendInventoryUpdate` — so the status-bar adena refreshes. The
/// bare-InventoryUpdate path left it stale.
#[test]
fn giving_adena_refreshes_the_adena_counter() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9100, 0);
    drain(&mut rx);

    inventory::give_item_with_earned_message(&mut world, 1, 9100, 57, 100_000);

    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| is_ex(p, 0x13E)),
        "ExAdenaInvenCount (status-bar adena) sent"
    );
    assert!(
        pkts.iter().any(|p| is_ex(p, 0x166)),
        "ExUserInfoInvenWeight sent"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9100)
            .unwrap()
            .adena(),
        100_000,
        "adena actually added"
    );
}

/// Drop → the item leaves the inventory and becomes a `GroundItem` world entity
/// (DropItem broadcast); a click (`Action`) picks it back up.
#[test]
fn drop_and_pickup_ground_item() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9200, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, 9200, 57, 1000).expect("adena");
    let count_of = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9200)
            .unwrap()
            .count_of(57)
    };
    let adena_oid = item_oid(&world, 9200, 57);

    // Drop 400 adena.
    let item_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(adena_oid);
    w.write_i64(400);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(&mut world, 1, w.into_bytes());

    assert_eq!(count_of(&world), 600, "400 left the inventory");
    let g = world
        .objects
        .get_component::<model::components::commerce::GroundItem>(&item_oid)
        .expect("ground item spawned");
    assert_eq!((g.item_id, g.count), (57, 400));
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DROP_ITEM),
        "DropItem broadcast"
    );

    // Click the ground item to pick it up (Action: objectId + origin xyz + action).
    let mut a = PacketWriter::new();
    a.write_u8(cop::ACTION);
    a.write_i32(item_oid);
    a.write_i32(0);
    a.write_i32(0);
    a.write_i32(0);
    a.write_u8(0);
    on_packet(&mut world, 1, a.into_bytes());
    // The stack landed where the drop asked (~90 units out), not underfoot, so
    // the click only starts the approach — `thinkPickUp` lifts it on arrival.
    advance_world(&mut world, 300);

    assert_eq!(count_of(&world), 1000, "adena back in the inventory");
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "ground item removed"
    );
    assert!(
        !world
            .ground_item_regions
            .values()
            .flatten()
            .any(|&id| id == item_oid),
        "ground item de-indexed"
    );
}

/// An enchanted item keeps its `+N` across drop → pickup.
///
/// Java gets this for free: both sides move the same `Item` instance between
/// containers. This port mints a fresh instance on the give path, so the level
/// has to be carried across explicitly — and until it was, dropping a `+7`
/// weapon and picking it straight back up silently returned it at `+0`.
///
/// The assertion is on the *enchant of the instance in the bag*, not merely on
/// the item being back: the old behaviour returned the item too.
#[test]
fn a_dropped_item_keeps_its_enchant_when_picked_back_up() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9200, 0);
    drain(&mut rx);

    // Short Sword — non-stackable (a distinct instance) and genuinely
    // dropable. The starter Squire's Sword is `is_dropable="false"`, so a drop
    // of it is correctly refused and would make this test vacuous.
    const SWORD: i32 = 1;
    inventory::add_inventory_item(&mut world, 9200, SWORD, 1).expect("sword");
    let sword_oid = item_oid(&world, 9200, SWORD);
    world
        .objects
        .get_component_mut::<Inventory>(&9200)
        .unwrap()
        .set_enchant_level(sword_oid, 7);

    let ground_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(sword_oid);
    w.write_i64(1);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(&mut world, 1, w.into_bytes());

    assert_eq!(
        world
            .objects
            .get_component::<model::components::commerce::GroundItem>(&ground_oid)
            .expect("ground item spawned")
            .enchant,
        7,
        "the drop side records the enchant"
    );

    let mut a = PacketWriter::new();
    a.write_u8(cop::ACTION);
    a.write_i32(ground_oid);
    a.write_i32(0);
    a.write_i32(0);
    a.write_i32(0);
    a.write_u8(0);
    on_packet(&mut world, 1, a.into_bytes());
    advance_world(&mut world, 300);

    let picked_enchant = inv_item(&world, 9200, SWORD)
        .expect("sword back in the bag")
        .enchant_level;
    assert_eq!(
        picked_enchant, 7,
        "and the pickup restores it — not a fresh +0 instance"
    );
}

/// Where the drop tests aim: within `RequestDropItem`'s 150/50 box of the
/// dummy character's `(1, 2, 3)`, the way a real client's cursor position is.
const DROP_AT: (i32, i32, i32) = (61, 72, 13);

/// Give `count` adena to `player_oid` and drop it via `RequestDropItem` at a
/// fixed spot; returns the resulting ground-item object id.
fn drop_adena(world: &mut World, client_id: u32, player_oid: i32, count: i64) -> i32 {
    inventory::add_inventory_item(world, player_oid, 57, count).expect("adena");
    let adena_oid = item_oid(world, player_oid, 57);
    let item_oid = world.next_npc_object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(adena_oid);
    w.write_i64(count);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(world, client_id, w.into_bytes());
    item_oid
}

/// Build a `RequestDropItem` body for `item_oid` at an explicit location.
fn drop_item_packet(item_oid: i32, count: i64, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(item_oid);
    w.write_i64(count);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// Give the player adena and return its inventory object id.
fn give_adena(world: &mut World, player_oid: i32, count: i64) -> i32 {
    inventory::add_inventory_item(world, player_oid, 57, count).expect("adena");
    item_oid(world, player_oid, 57)
}

/// The dropped stack lands **where the client asked**, not at the player's
/// feet: Java reads `_x/_y/_z` off the packet and hands them to
/// `Player.dropItem` → `Item.dropMe`. Dropping at the character's own position
/// is what the port used to do, and it made every discarded stack pile up
/// under the character instead of scattering where it was dragged.
#[test]
fn dropped_item_lands_at_the_requested_location() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    let pos = world
        .objects
        .get_component::<Position>(&ground_oid)
        .expect("the stack reached the ground");
    assert_eq!(
        (pos.x, pos.y, pos.z),
        DROP_AT,
        "the ground item sits at the requested drop point"
    );
    let player_pos = *world.objects.get_component::<Position>(&9300).unwrap();
    assert_ne!(
        (pos.x, pos.y),
        (player_pos.x, player_pos.y),
        "sanity: the requested point is not the player's own position"
    );
}

/// `!player.isInsideRadius2D(_x, _y, 0, 150)` — a drop aimed further than 150
/// units away is refused with SM 151 and the stack stays in the inventory.
/// Without this a client could post items across the map from where it stands.
#[test]
fn drop_beyond_150_units_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    // (1, 2, 3) → (401, 2, 3) is 400 units out.
    on_packet(&mut world, 1, drop_item_packet(adena_oid, 100, 401, 2, 3));

    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&ground_oid),
        "nothing reached the ground"
    );
    assert_eq!(
        item_count(&world, 9300, 57),
        100,
        "the adena stays in the inventory"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_DISCARD_SOMETHING_THAT_FAR_AWAY_FROM_YOU),
        "the client is told the spot is too far away"
    );
}

/// The same guard's second half: `Math.abs(_z - player.getZ()) > 50`. The 2D
/// distance is fine here — only the height differs — so a port that checked
/// distance in 3D, or skipped z entirely, would let this through.
#[test]
fn drop_more_than_50_units_below_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, 11, 12, -300),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&ground_oid),
        "nothing reached the ground"
    );
    assert_eq!(item_count(&world, 9300, 57), 100, "adena kept");
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_DISCARD_SOMETHING_THAT_FAR_AWAY_FROM_YOU),
        "the client is told the spot is too far away"
    );
    // …while the same request at the player's own height is accepted, so the
    // refusal is the z test and not something else in the chain.
    let ground_oid = world.next_npc_object_id;
    on_packet(&mut world, 1, drop_item_packet(adena_oid, 100, 11, 12, 13));
    assert!(
        world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&ground_oid),
        "the in-range drop goes through"
    );
}

/// `player.isInsideZone(ZoneId.NO_ITEM_DROP)` — inside a `ConditionZone` that
/// declares `NoItemDrop` (`no_drop_item.xml`: the bascule bridge, the
/// Underground Coliseum floors) nothing may be discarded at all.
#[test]
fn drop_inside_a_no_item_drop_zone_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        id: 0,
        name: "test_no_drop".into(),
        kind: crate::data::zone_data::ZoneKind::Condition,
        territory: Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -1000,
                x2: 1000,
                y1: -1000,
                y2: 1000,
            },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: Some(crate::data::zone_data::ConditionZoneParams {
            no_item_drop: true,
            no_bookmark: false,
        }),
        mother_tree: None,
    });
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&ground_oid),
        "nothing reached the ground inside the zone"
    );
    assert_eq!(item_count(&world, 9300, 57), 100, "adena kept");
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED),
        "the client is told the item cannot be discarded"
    );
}

/// "Do not drop items when casting known skills to avoid exploits." Java
/// refuses mid-cast with `"You cannot drop an item while casting " +
/// skill.getName() + "."` — the **named** skill, so the player can tell which
/// cast is holding their inventory. `SkillData` now keeps `<skill name="…">`
/// per id to say it.
#[test]
fn drop_while_casting_a_known_skill_is_refused_by_name() {
    const WIND_STRIKE: i32 = 1177;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    world
        .data
        .skill_data
        .insert_name_for_test(WIND_STRIKE, "Wind Strike");
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    // The character knows the skill *and* is casting it.
    world
        .objects
        .get_component_mut::<SkillBook>(&9300)
        .unwrap()
        .0
        .insert(WIND_STRIKE, 1);
    world.objects.add_components(
        &9300,
        Casting(model::CastState {
            skill_id: WIND_STRIKE,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: 0,
            seq: 0,
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 100, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&ground_oid),
        "nothing reached the ground mid-cast"
    );
    assert_eq!(item_count(&world, 9300, 57), 100, "adena kept");
    let pkts = drain(&mut rx);
    let needle: Vec<u8> = "You cannot drop an item while casting Wind Strike."
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert!(
        pkts.iter()
            .any(|p| p.windows(needle.len()).any(|w| w == needle)),
        "the refusal names the skill being cast"
    );
}

/// `_count > item.getCount()` refuses outright (Java sends
/// `THAT_ITEM_CANNOT_BE_DISCARDED`) rather than clamping — a forged count must
/// not walk away with the whole stack under a partial-drop request.
#[test]
fn drop_of_more_than_is_held_is_refused() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let adena_oid = give_adena(&mut world, 9300, 100);
    let ground_oid = world.next_npc_object_id;

    on_packet(
        &mut world,
        1,
        drop_item_packet(adena_oid, 500, DROP_AT.0, DROP_AT.1, DROP_AT.2),
    );

    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&ground_oid),
        "nothing reached the ground"
    );
    assert_eq!(
        item_count(&world, 9300, 57),
        100,
        "the whole stack is still held"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED),
        "the client is told the item cannot be discarded"
    );
}

/// Datapack parity: *Mage Class Equipment Set (10-day)* (15195) declares
/// `is_dropable="false"`, so `RequestDropItem` must refuse it with
/// `THAT_ITEM_CANNOT_BE_DISCARDED` and leave it in the inventory — Java's
/// first guard in `RequestDropItem.runImpl`.
#[test]
fn bound_item_cannot_be_discarded() {
    const BOUND_BOX: i32 = 15195;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);

    inventory::add_inventory_item(&mut world, 9300, BOUND_BOX, 1).expect("bound box");
    let box_oid = item_oid(&world, 9300, BOUND_BOX);
    let would_be_ground_oid = world.next_npc_object_id;

    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(box_oid);
    w.write_i64(1);
    w.write_i32(DROP_AT.0);
    w.write_i32(DROP_AT.1);
    w.write_i32(DROP_AT.2);
    on_packet(&mut world, 1, w.into_bytes());

    assert!(
        world
            .objects
            .get_component::<Inventory>(&9300)
            .unwrap()
            .items()
            .iter()
            .any(|it| it.item_id == BOUND_BOX),
        "the bound box stays in the inventory"
    );
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&would_be_ground_oid),
        "nothing reached the ground"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THAT_ITEM_CANNOT_BE_DISCARDED),
        "the client is told the item cannot be discarded"
    );
}

/// Java never lifts a ground item on the click itself: `ItemAction` only sets
/// `AI_INTENTION_PICK_UP`, `CreatureAI.onIntentionPickUp` fires `moveToPawn`,
/// and `Player.doPickupItem` runs later from `PlayerAI.thinkPickUp` — once
/// `maybeMoveToPawn(target, 36)` reports the walk has arrived. So clicking loot
/// across the field must walk the character over, not teleport the item into
/// the bag.
#[test]
fn distant_ground_item_is_walked_to_before_pickup() {
    use crate::game_loop::items::ground_items::{DropSource, spawn_ground_item};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9400, 0);
    let start = *world.objects.get_component::<Position>(&9400).unwrap();
    drain(&mut rx);

    // 500 units away — far outside `maybeMoveToPawn`'s 36 + collision radius.
    let item_oid = spawn_ground_item(
        &mut world,
        57,
        400,
        0,
        start.x + 500,
        start.y,
        start.z,
        0,
        DropSource::Npc,
    );
    let held = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9400)
            .unwrap()
            .count_of(57)
    };
    assert_eq!(held(&world), 0, "sanity: no adena to start with");
    drain(&mut rx);

    // The click starts the approach and nothing else.
    handle_action(&mut world, 1, &action_body(item_oid, 0));
    assert_eq!(held(&world), 0, "the click alone must not pick it up");
    assert!(
        world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "the item is still on the ground"
    );
    assert!(
        matches!(
            world.objects.get_component::<Intent>(&9400).copied(),
            Some(Intent(model::PlayerIntent::PickUp { item_object_id })) if item_object_id == item_oid
        ),
        "AI_INTENTION_PICK_UP is set"
    );
    assert!(
        world.objects.has_component::<Movement>(&9400),
        "and the character is walking to it (moveToPawn)"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_PAWN),
        "MoveToPawn broadcast"
    );

    // Walk it out: `thinkPickUp` lifts the item on the tick it arrives.
    advance_world(&mut world, 300);
    assert_eq!(held(&world), 400, "picked up on arrival");
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "ground item removed"
    );
    assert!(
        !world.objects.has_component::<Intent>(&9400),
        "thinkPickUp's setIntention(AI_INTENTION_IDLE) clears the intention"
    );
    let end = *world.objects.get_component::<Position>(&9400).unwrap();
    assert!(
        ((end.x - start.x) as f64).hypot((end.y - start.y) as f64) > 400.0,
        "the character actually travelled to the loot"
    );
}

/// `CreatureAI.onIntentionPickUp`'s REST branch: a seated player's click on
/// loot is refused outright with a bare `ActionFailed` — no walk is started
/// and the item stays put.
#[test]
fn seated_player_cannot_pick_up() {
    use crate::game_loop::items::ground_items::{DropSource, spawn_ground_item};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9401, 0);
    let start = *world.objects.get_component::<Position>(&9401).unwrap();
    world
        .objects
        .get_component_mut::<Player>(&9401)
        .unwrap()
        .sitting = true;
    // At the player's feet, so only the REST gate can refuse it.
    let item_oid = spawn_ground_item(
        &mut world,
        57,
        400,
        0,
        start.x,
        start.y,
        start.z,
        0,
        DropSource::Npc,
    );
    drain(&mut rx);

    handle_action(&mut world, 1, &action_body(item_oid, 0));
    assert!(
        world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "loot stays on the floor while seated"
    );
    assert!(
        !world.objects.has_component::<Intent>(&9401),
        "no pick-up intention is started"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "clientActionFailed"
    );
}

/// A ground item left un-picked-up auto-destroys after its lifetime
/// (`ItemsOnGroundManager` cleanup) — when General.ini enables it.
#[test]
fn ground_item_decays_after_lifetime() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    // Enable player-drop auto-destroy (General.ini `AutoDestroyDroppedItemAfter`
    // + `DestroyPlayerDroppedItem`); the dist default keeps player drops.
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = true;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let item_oid = drop_adena(&mut world, 1, 9300, 100);
    assert!(
        world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "dropped"
    );

    // Jump past the 600 s lifetime and fire the scheduled decay.
    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "decayed"
    );
    assert!(
        !world
            .ground_item_regions
            .values()
            .flatten()
            .any(|&id| id == item_oid),
        "de-indexed"
    );
}

/// General.ini parity: with `DestroyPlayerDroppedItem = False` (the dist
/// value), a player's drop is **never** auto-destroyed even when
/// `AutoDestroyDroppedItemAfter > 0` — it persists until pickup/restart.
#[test]
fn player_ground_item_persists_when_destroy_player_dropped_off() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = false; // dist default
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9300, 0);
    drain(&mut rx);
    let item_oid = drop_adena(&mut world, 1, 9300, 100);

    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "player drop persists"
    );
}

/// An NPC drop auto-destroys whenever `AutoDestroyDroppedItemAfter > 0`,
/// independent of the player-drop flag (Java `Npc.dropItem`).
#[test]
fn npc_ground_item_decays_regardless_of_player_flag() {
    use crate::game_loop::items::ground_items::{DropSource, spawn_ground_item};
    let (mut world, ..) = admin_world();
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.destroy_dropped_player_item = false;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let item_oid = spawn_ground_item(&mut world, 57, 100, 0, 100, 200, -3000, 0, DropSource::Npc);
    assert!(
        world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "npc drop on ground"
    );

    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::GroundItem>(&item_oid),
        "npc drop decays"
    );
}

/// **Herbs run their own auto-destroy clock.** Java's gate is an *either/or*:
/// `(AUTODESTROY_ITEM_AFTER > 0 && !hasExImmediateEffect()) ||
/// (HERB_AUTO_DESTROY_TIME > 0 && hasExImmediateEffect())`. So a herb vanishes
/// on `AutoDestroyHerbTime` (60 s) rather than the ordinary 600 s — and it is
/// scheduled even when the ordinary destroyer is switched off entirely.
#[test]
fn herbs_decay_on_their_own_shorter_clock() {
    use crate::game_loop::items::ground_items::{DropSource, spawn_ground_item};
    use crate::model::components::commerce::GroundItem;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.cfg.general.autodestroy_item_after = 600;
    world.cfg.general.herb_auto_destroy_time = 60;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let _rx = ingame_player_access(&mut world, 1, 9300, 0);

    // 8600 "Herb of Life" carries `ex_immediate_effect`; 57 adena does not.
    let herb = spawn_ground_item(&mut world, 8600, 1, 0, 100, 200, 0, 0, DropSource::Npc);
    let coin = spawn_ground_item(&mut world, 57, 100, 0, 100, 200, 0, 0, DropSource::Npc);

    // Past 60 s: the herb is gone, the coin is not.
    world.tick += 60 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world.objects.has_component::<GroundItem>(&herb),
        "the herb is swept on the 60 s herb clock"
    );
    assert!(
        world.objects.has_component::<GroundItem>(&coin),
        "…while an ordinary drop still has its 600 s"
    );

    // Past 600 s the coin goes too.
    world.tick += 600 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(!world.objects.has_component::<GroundItem>(&coin));
}

/// The herb clock is gated **independently** of the ordinary one: with
/// `AutoDestroyDroppedItemAfter = 0` a herb is still swept, because Java's two
/// conditions are alternatives rather than nested.
#[test]
fn herbs_decay_even_with_the_ordinary_destroyer_off() {
    use crate::game_loop::items::ground_items::{DropSource, spawn_ground_item};
    use crate::model::components::commerce::GroundItem;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.cfg.general.autodestroy_item_after = 0; // ordinary destroyer off
    world.cfg.general.herb_auto_destroy_time = 60;
    world.id_pool = 0x4000_0000..0x4000_0100;
    let _rx = ingame_player_access(&mut world, 1, 9300, 0);

    let herb = spawn_ground_item(&mut world, 8600, 1, 0, 100, 200, 0, 0, DropSource::Npc);
    let coin = spawn_ground_item(&mut world, 57, 100, 0, 100, 200, 0, 0, DropSource::Npc);
    world.tick += 60 * 10 + 1;
    apply_due_tasks(&mut world);
    assert!(
        !world.objects.has_component::<GroundItem>(&herb),
        "the herb clock stands on its own"
    );
    assert!(
        world.objects.has_component::<GroundItem>(&coin),
        "and the coin is never scheduled at all"
    );
}
