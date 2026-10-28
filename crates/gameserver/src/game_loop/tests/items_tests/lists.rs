//! The protocol-140 inventory lists: `ItemList` and `ExQuestItemList` go out
//! as a header phase and an items phase, and `InventoryUpdate` carries an int
//! count behind two new header fields.

use super::*;
use commons::network::PacketReader;

const ITEM_LIST: u8 = 0x11;
const INVENTORY_UPDATE: u8 = 0x21;
const EX_QUEST_ITEM_LIST: i16 = 0xC7;

/// A world whose player holds adena, a sword and one quest item.
fn bag() -> (
    World,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    i32,
) {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    let quest_item = world
        .data
        .item_data
        .all()
        .find(|t| t.is_quest_item)
        .map(|t| t.item_id)
        .expect("the dist ships quest items");
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9900, 0);
    inventory::add_inventory_item(&mut world, 9900, 57, 1000).unwrap();
    inventory::add_inventory_item(&mut world, 9900, 2551, 1).unwrap();
    inventory::add_inventory_item(&mut world, 9900, quest_item, 1).unwrap();
    drain(&mut rx);
    (world, rx, quest_item)
}

/// `RequestItemList` answers with Java's `sendItemList()`: `ItemList` 1 and 2,
/// `ExQuestItemList` 1 and 2, then the adena counter and the weight bar.
#[test]
fn request_item_list_sends_both_phases_of_both_lists() {
    let (mut world, mut rx, quest_item) = bag();
    crate::game_loop::items::handle_request_item_list(&mut world, 1);
    let out = drain(&mut rx);
    let lists: Vec<_> = out
        .iter()
        .filter(|p| p[0] == ITEM_LIST || is_ex(p, EX_QUEST_ITEM_LIST))
        .collect();
    assert_eq!(lists.len(), 4, "two phases of each list");

    // Phase 1: the header alone.
    let mut r = PacketReader::new(&lists[0][1..]);
    assert_eq!(r.read_u8(), Some(1));
    assert_eq!(r.read_i32(), Some(0));
    assert_eq!(r.read_i32(), Some(2), "adena and the sword");
    assert_eq!(r.read_i16(), Some(0), "no inventory block");
    assert_eq!(r.remaining(), 0);

    // Phase 2: the items, then the inventory block.
    let mut r = PacketReader::new(&lists[1][1..]);
    assert_eq!(r.read_u8(), Some(2));
    assert_eq!(r.read_i32(), Some(2));
    assert_eq!(r.read_i32(), Some(2));
    r.read_bytes(2 * 43);
    assert_eq!(r.read_i16(), Some(0));
    assert_eq!(r.remaining(), 0);

    // Both quest-list phases carry the quest item.
    for (phase, pkt) in [(1u8, lists[2]), (2, lists[3])] {
        let mut r = PacketReader::new(&pkt[3..]);
        assert_eq!(r.read_u8(), Some(phase));
        if phase == 2 {
            assert_eq!(r.read_i32(), Some(1));
        } else {
            assert_eq!(r.read_i16(), Some(0));
        }
        assert_eq!(r.read_i32(), Some(1));
        r.read_bytes(5);
        assert_eq!(r.read_i32(), Some(quest_item));
    }

    assert!(
        out.iter().any(|p| is_ex(p, 0x13E)),
        "the adena counter (ExAdenaInvenCount) follows"
    );
}

/// `InventoryUpdate` is `byte 0, int 0, int n`, and an augmented item sets the
/// augment mask bit and writes both options.
#[test]
fn inventory_update_header_and_augment_block() {
    let (mut world, mut rx, _) = bag();
    let sword = item_oid(&world, 9900, 2551);
    world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&9900)
        .unwrap()
        .set_augmentation(sword, 0, 111, 222);
    let changes = inventory::modified_changes(&world, 9900, &[sword]);
    inventory::send_inventory_update(&world, 9900, changes);
    let out = drain(&mut rx);
    let iu = out
        .iter()
        .find(|p| p[0] == INVENTORY_UPDATE)
        .expect("InventoryUpdate");

    let mut r = PacketReader::new(&iu[1..]);
    assert_eq!(r.read_u8(), Some(0));
    assert_eq!(r.read_i32(), Some(0));
    assert_eq!(r.read_i32(), Some(1));
    assert_eq!(r.read_i16(), Some(2), "modified");
    assert_eq!(r.read_u8(), Some(1), "AUGMENT_BONUS");
    r.read_bytes(42); // the fixed fields and the two protocol-140 bytes
    assert_eq!(r.read_i32(), Some(111));
    assert_eq!(r.read_i32(), Some(222));
    assert_eq!(r.remaining(), 0);
}
