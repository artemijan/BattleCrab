//! `General.ini`'s ground-item persistence and item write-path keys —
//! `SaveDroppedItem` and its three companions, `MultipleItemDrop`,
//! `DestroyAllItems`, `UpdateItemsOnCharStore`.
//!
//! The four `*DroppedItem*` keys are all **off** on this dist, so the visible
//! behaviour is unchanged by wiring them. What the tests pin is the other
//! branch: an operator who turns `SaveDroppedItem` on gets ground items that
//! survive a restart, with the right amount of lifetime left.

use super::*;
use crate::db::GroundItemRow;
use crate::game_loop::ground_items;
use crate::model::components::{GroundItem, Position};

/// `Adena` — stackable, and in the real item table.
const STACKABLE: i32 = 57;
/// `Darin's Letter` — non-stackable.
const NON_STACKABLE: i32 = 687;

fn ground_world() -> (World, db::CmdRx) {
    let (mut world, _tx, db_rx, _link) = test_world();
    world.data.item_data = crate::data::dist::items_owned();
    world.cfg.general.save_dropped_item = true;
    // `add_inventory_item` allocates from the runtime id pool, which a fresh
    // test world leaves empty.
    world.id_pool = 0x6000_0000..0x6000_0100;
    (world, db_rx)
}

fn ground_ids(world: &mut World) -> Vec<i32> {
    let mut v = Vec::new();
    world
        .objects
        .for_each_mut::<(&GroundItem,)>(|(g,)| v.push(g.object_id));
    v.sort_unstable();
    v
}

/// A restart round-trip: what `store_all` gathers is what `restore_from_db`
/// puts back, in the same place with the same stack.
#[test]
fn a_stored_ground_item_comes_back_where_it_lay() {
    let (mut world, mut db_rx) = ground_world();
    let oid = ground_items::spawn_ground_item(
        &mut world,
        STACKABLE,
        1234,
        0,
        11_000,
        22_000,
        -3_000,
        0,
        ground_items::DropSource::Npc,
    );
    assert!(world.objects.has_component::<GroundItem>(&oid));

    ground_items::store_all(&mut world);
    let rows = match db_rx.try_recv() {
        Ok(crate::db::DbCommand::StoreGroundItems { items }) => items,
        _ => panic!("expected a StoreGroundItems command"),
    };
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].item_id, rows[0].count), (STACKABLE, 1234));
    assert_eq!((rows[0].x, rows[0].y, rows[0].z), (11_000, 22_000, -3_000));

    // A fresh world — the restart.
    let (mut world, _db_rx) = ground_world();
    assert_eq!(ground_items::restore_from_db(&mut world, &rows), 1);
    let restored = ground_ids(&mut world);
    assert_eq!(restored.len(), 1);
    let (item_id, count) = world
        .objects
        .get_component::<GroundItem>(&restored[0])
        .map(|g| (g.item_id, g.count))
        .unwrap();
    assert_eq!((item_id, count), (STACKABLE, 1234));
    let pos = world
        .objects
        .get_component::<Position>(&restored[0])
        .map(|p| (p.x, p.y, p.z))
        .unwrap();
    assert_eq!(pos, (11_000, 22_000, -3_000));
}

/// **The decay clock is resumed, not restarted.** `drop_time` is wall-clock
/// because ticks do not survive a restart; an item that lay for most of its
/// lifetime must decay almost at once, not get a fresh full term.
///
/// Asserted as an inequality against the full term rather than an exact tick,
/// because the fixture cannot control the millisecond the row is read.
#[test]
fn a_restored_item_keeps_only_the_lifetime_it_had_left() {
    let (mut world, _db_rx) = ground_world();
    world.cfg.general.autodestroy_item_after = 600;
    let now = commons::util::now_millis();

    let nearly_spent = GroundItemRow {
        object_id: 90_001,
        item_id: STACKABLE,
        count: 1,
        enchant_level: 0,
        x: 100,
        y: 100,
        z: 0,
        // 590 s of a 600 s life already gone.
        drop_time_ms: now - 590_000,
        equipable: false,
    };
    let fresh = GroundItemRow {
        object_id: 90_002,
        drop_time_ms: now,
        ..nearly_spent
    };
    assert_eq!(
        ground_items::restore_from_db(&mut world, &[nearly_spent, fresh]),
        2
    );

    let due = world.scheduler.pending_ticks_for_test();
    assert_eq!(due.len(), 2, "both rows armed a decay");
    let (soon, late) = (due.iter().min().unwrap(), due.iter().max().unwrap());
    assert!(
        *soon <= world.tick + 110,
        "the nearly-spent item decays within ~11 s, not 600 (tick {soon})"
    );
    assert!(
        *late > world.tick + 5_000,
        "the fresh one keeps most of its term (tick {late})"
    );
}

/// `drop_time == -1` is Java's protected flag — the item was never on the
/// auto-destroy list, and reloading it must not put it there.
#[test]
fn a_protected_row_is_restored_without_a_decay() {
    let (mut world, _db_rx) = ground_world();
    world.cfg.general.autodestroy_item_after = 600;
    let row = GroundItemRow {
        object_id: 90_003,
        item_id: STACKABLE,
        count: 1,
        enchant_level: 0,
        x: 100,
        y: 100,
        z: 0,
        drop_time_ms: -1,
        equipable: false,
    };
    assert_eq!(ground_items::restore_from_db(&mut world, &[row]), 1);
    assert!(
        world.scheduler.pending_ticks_for_test().is_empty(),
        "a protected item lies there forever"
    );
}

/// A row whose template has left the datapack is dropped rather than spawned —
/// Java's `getTemplate() == null` guard on the same path. Without it the item
/// becomes a ground entity nobody can pick up or describe.
#[test]
fn a_row_for_an_unknown_item_is_discarded() {
    let (mut world, _db_rx) = ground_world();
    let row = GroundItemRow {
        object_id: 90_004,
        item_id: 999_001,
        count: 1,
        enchant_level: 0,
        x: 100,
        y: 100,
        z: 0,
        drop_time_ms: commons::util::now_millis(),
        equipable: false,
    };
    assert_eq!(ground_items::restore_from_db(&mut world, &[row]), 0);
    assert!(ground_ids(&mut world).is_empty());
}

/// `SaveDroppedItem = False` (the dist) — the gather is a no-op, so no command
/// is queued at all. Java's `run()` returns before touching the table, and its
/// `save()`/`removeObject()` never populate the set either.
#[test]
fn nothing_is_stored_while_the_key_is_off() {
    let (mut world, mut db_rx) = ground_world();
    world.cfg.general.save_dropped_item = false;
    ground_items::spawn_ground_item(
        &mut world,
        STACKABLE,
        1,
        0,
        100,
        100,
        0,
        0,
        ground_items::DropSource::Npc,
    );
    ground_items::store_all(&mut world);
    assert!(
        db_rx.try_recv().is_err(),
        "no DB command while SaveDroppedItem is off"
    );
}

/// `MultipleItemDrop` (**True** here) splits a non-stackable into one instance
/// per unit. Its off branch is **lossy**, not merging: Java breaks out of the
/// loop on the first pass, creating one unit and discarding the rest.
///
/// Both branches are asserted because the lossy one is the surprising half, and
/// because a stackable must be unaffected either way.
#[test]
fn multiple_item_drop_splits_non_stackables_and_its_off_branch_loses_the_rest() {
    for (key, expect_units) in [(true, 5), (false, 1)] {
        let (mut world, _db_rx) = ground_world();
        world.cfg.general.multiple_item_drop = key;
        let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);

        let created = items::add_inventory_item(&mut world, 100, NON_STACKABLE, 5).expect("added");
        assert_eq!(
            created.len(),
            expect_units,
            "MultipleItemDrop = {key}: instance count"
        );
        assert_eq!(
            inv_count(&world, NON_STACKABLE),
            expect_units as i64,
            "MultipleItemDrop = {key}: total units held"
        );

        // A stackable is one instance of N regardless.
        let created = items::add_inventory_item(&mut world, 100, STACKABLE, 5).expect("added");
        assert_eq!(created.len(), 1, "a stackable never splits");
        assert_eq!(inv_count(&world, STACKABLE), 5);
    }
}

/// `UpdateItemsOnCharStore` selects whether the periodic save writes the item
/// half. It has to be a flag on the payload rather than an empty item list,
/// because the write is delete-then-reinsert: an empty list would delete
/// everything the character owns.
///
/// **Scope of this test:** it pins the flag the game thread computes, not the
/// DB thread's use of it. Sabotaging `store_player`'s `if s.store_items` leaves
/// this green — the write lives behind a channel and a real database, which the
/// unit fixtures do not reach. `char_persistence` is where that side is
/// exercised, and it stores unconditionally.
#[test]
fn update_items_on_char_store_selects_whether_the_save_carries_items() {
    for key in [true, false] {
        let (mut world, mut db_rx) = ground_world();
        world.cfg.general.update_items_on_char_store = key;
        let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
        items::add_inventory_item(&mut world, 100, STACKABLE, 10);

        crate::game_loop::net::store_player_now(&mut world, 100);
        let save = loop {
            match db_rx.try_recv() {
                Ok(crate::db::DbCommand::StorePlayer { save }) => break save,
                Ok(_) => continue,
                Err(_) => panic!("no StorePlayer queued"),
            }
        };
        assert_eq!(save.store_items, key, "UpdateItemsOnCharStore = {key}");
        assert!(
            !save.items.is_empty(),
            "the item list is gathered either way — the flag, not an empty \
             list, is what suppresses the write"
        );
    }
}
