//! Death item drops (G20): the karma penalty for dying as a PK, and the
//! gentler drop a monster kill can cause.

use super::*;

use crate::model::Player;
use crate::model::inventory::Inventory;

const VICTIM: i32 = 2001;
const KILLER: i32 = 2002;
const VICTIM_CID: u32 = 1;
const KILLER_CID: u32 = 2;
const LOOT_ITEM: i32 = 8400;

/// Certain drops: every rate at 100 so the rolls can't hide a wiring bug.
fn drop_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cast_test_world();
    {
        let r = &mut world.cfg.rates;
        r.karma_rate_drop = 100;
        r.karma_rate_drop_item = 100;
        r.karma_rate_drop_equip = 100;
        r.karma_rate_drop_equip_weapon = 100;
        r.karma_drop_limit = 10;
        r.karma_pk_limit = 4;
        r.player_rate_drop = 100;
        r.player_rate_drop_item = 100;
        r.player_rate_drop_equip = 100;
        r.player_rate_drop_equip_weapon = 100;
        r.player_drop_limit = 3;
    }
    (world, db, l)
}

fn give(world: &mut World, oid: i32, item_id: i32, count: i64, obj_id: i32) {
    let World { objects, data, .. } = world;
    objects
        .get_component_mut::<Inventory>(&oid)
        .unwrap()
        .add_item(&data.item_data, obj_id, item_id, count);
}

/// A PK with `pk_kills` past the limit, holding `n` distinct loot stacks.
fn pk_victim(world: &mut World, stacks: i32) {
    {
        let p = world.objects.get_component_mut::<Player>(&VICTIM).unwrap();
        p.reputation = -1000;
        p.pk_kills = 10;
    }
    for i in 0..stacks {
        give(world, VICTIM, LOOT_ITEM + i, 1, 0x6000_0000 + i);
    }
}

fn inventory_len(world: &World, oid: i32) -> usize {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .map(|i| i.items().len())
        .unwrap_or(0)
}

fn ground_item_count(world: &World) -> usize {
    world.ground_item_regions.values().map(|v| v.len()).sum()
}

fn register_loot(world: &mut World, stacks: i32) {
    for i in 0..stacks {
        let mut t = crate::data::item_data::ItemTemplate {
            item_id: LOOT_ITEM + i,
            name: format!("Loot {i}"),
            ..items_tests_template()
        };
        t.type2 = 5; // not a weapon, not a quest item
        world.data.item_data.insert_for_test(t);
    }
}

/// Minimal etc-item template shared by the fixtures.
fn items_tests_template() -> crate::data::item_data::ItemTemplate {
    crate::data::item_data::ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id: 0,
        name: String::new(),
        kind: crate::data::item_data::ItemKind::Etc,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 0,
        type2: 5,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: crate::data::item_data::ItemHandler::None,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: crate::data::item_data::EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    }
}

fn kill_by_player(world: &mut World) {
    crate::game_loop::death::player_do_die(world, VICTIM, KILLER);
}

// ---------------------------------------------------------------------------

/// A repeat **PK** killed by another player scatters their inventory.
#[test]
fn pk_killed_by_player_drops_items() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);
    register_loot(&mut world, 2);
    pk_victim(&mut world, 2);
    assert_eq!(inventory_len(&world, VICTIM), 2);

    kill_by_player(&mut world);

    assert_eq!(
        inventory_len(&world, VICTIM),
        0,
        "the PK dropped their loot"
    );
    assert_eq!(ground_item_count(&world), 2, "and it is on the ground");
}

/// A **clean** player killed by a player drops nothing — this is a karma
/// penalty, not general looting.
#[test]
fn clean_player_killed_by_player_drops_nothing() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);
    register_loot(&mut world, 2);
    give(&mut world, VICTIM, LOOT_ITEM, 1, 0x6000_0000);

    kill_by_player(&mut world);

    assert_eq!(
        inventory_len(&world, VICTIM),
        1,
        "a clean victim keeps everything"
    );
    assert_eq!(ground_item_count(&world), 0);
}

/// A PK below `MinimumPKRequiredToDrop` is still spared.
#[test]
fn pk_below_the_limit_drops_nothing() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);
    register_loot(&mut world, 1);
    pk_victim(&mut world, 1);
    world
        .objects
        .get_component_mut::<Player>(&VICTIM)
        .unwrap()
        .pk_kills = 1; // < 4

    kill_by_player(&mut world);

    assert_eq!(
        inventory_len(&world, VICTIM),
        1,
        "not enough PKs to be punished"
    );
}

/// Dying to a **monster** uses the player rates — a clean player can still
/// lose something.
#[test]
fn monster_kill_uses_the_player_rates() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);
    register_loot(&mut world, 2);
    give(&mut world, VICTIM, LOOT_ITEM, 1, 0x6000_0000);

    crate::game_loop::death::player_do_die(&mut world, VICTIM, NPC_OID);

    assert_eq!(
        inventory_len(&world, VICTIM),
        0,
        "a monster kill can cost items"
    );
    assert_eq!(ground_item_count(&world), 1);
}

/// The drop count is capped by the configured limit.
#[test]
fn drop_count_is_capped_by_the_limit() {
    let (mut world, _db, _l) = drop_world();
    world.cfg.rates.karma_drop_limit = 2;
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);
    register_loot(&mut world, 5);
    pk_victim(&mut world, 5);

    kill_by_player(&mut world);

    assert_eq!(ground_item_count(&world), 2, "capped at the drop limit");
    assert_eq!(inventory_len(&world, VICTIM), 3, "the rest is kept");
}

/// Adena and quest items never drop, however deep the karma.
#[test]
fn adena_and_quest_items_never_drop() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);

    // Adena plus a quest item.
    let mut adena = items_tests_template();
    adena.item_id = crate::data::item_data::ADENA_ID;
    adena.name = "Adena".into();
    world.data.item_data.insert_for_test(adena);
    let mut quest = items_tests_template();
    quest.item_id = LOOT_ITEM;
    quest.name = "Quest Item".into();
    quest.is_quest_item = true;
    world.data.item_data.insert_for_test(quest);

    {
        let p = world.objects.get_component_mut::<Player>(&VICTIM).unwrap();
        p.reputation = -1000;
        p.pk_kills = 10;
    }
    give(
        &mut world,
        VICTIM,
        crate::data::item_data::ADENA_ID,
        5000,
        0x6100_0000,
    );
    give(&mut world, VICTIM, LOOT_ITEM, 1, 0x6100_0001);

    kill_by_player(&mut world);

    assert_eq!(
        ground_item_count(&world),
        0,
        "neither adena nor quest items fall"
    );
    assert_eq!(inventory_len(&world, VICTIM), 2);
}

/// Items the datapack marks `is_dropable="false"` — the bound reward boxes such
/// as *Mage Class Equipment Set (10-day)* (15195) — survive a PK death, and so
/// do time-limited items. Java filters both in `Player.onDieDropItem`.
#[test]
fn bound_and_time_limited_items_never_drop() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);

    // A bound reward box: untradable / undroppable / unsellable, a time-limited
    // item, and a plain droppable item as the control.
    let mut bound = items_tests_template();
    bound.item_id = LOOT_ITEM;
    bound.name = "Mage Class Equipment Set".into();
    bound.trade_flags.dropable = false;
    bound.trade_flags.tradable = false;
    bound.is_sellable = false;
    world.data.item_data.insert_for_test(bound);
    let mut limited = items_tests_template();
    limited.item_id = LOOT_ITEM + 1;
    limited.name = "Time Limited".into();
    limited.time = 14400;
    world.data.item_data.insert_for_test(limited);
    let mut plain = items_tests_template();
    plain.item_id = LOOT_ITEM + 2;
    plain.name = "Plain Loot".into();
    world.data.item_data.insert_for_test(plain);

    pk_victim(&mut world, 0);
    give(&mut world, VICTIM, LOOT_ITEM, 1, 0x6200_0000);
    give(&mut world, VICTIM, LOOT_ITEM + 1, 1, 0x6200_0001);
    give(&mut world, VICTIM, LOOT_ITEM + 2, 1, 0x6200_0002);

    kill_by_player(&mut world);

    assert_eq!(
        ground_item_count(&world),
        1,
        "only the ordinary item reaches the ground"
    );
    let kept: Vec<i32> = world
        .objects
        .get_component::<Inventory>(&VICTIM)
        .unwrap()
        .items()
        .iter()
        .map(|it| it.item_id)
        .collect();
    assert_eq!(kept.len(), 2, "the bound box and the timed item are kept");
    assert!(kept.contains(&LOOT_ITEM), "the bound box stayed");
    assert!(kept.contains(&(LOOT_ITEM + 1)), "the timed item stayed");
}

/// Arena deaths cost nothing when another player did the killing.
#[test]
fn pvp_zone_death_drops_nothing() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);
    register_loot(&mut world, 1);
    pk_victim(&mut world, 1);
    world
        .objects
        .get_component_mut::<crate::model::components::ZoneFlags>(&VICTIM)
        .unwrap()
        .mask = crate::data::zone_data::ZoneKind::Pvp.bit();

    kill_by_player(&mut world);

    assert_eq!(inventory_len(&world, VICTIM), 1, "arena deaths are free");
}

/// **A shadow item never drops on death.** Java's filter is `isShadowItem() ||
/// isTimeLimitedItem() || !isDropable() || ADENA || TYPE2_QUEST`, and this port
/// implemented every leg *except* the first — so a Shadow weapon scattered on a
/// karma death where retail keeps it. 295 shadow items are reachable on this
/// chronicle.
///
/// The half that makes this more than a one-line filter: `isShadowItem()` is
/// `_mana >= 0` on the **item instance**, not a template property. Two copies
/// of the same item id can differ, which is why the test gives the victim two
/// stacks of one id and expects exactly the un-manaed one to fall.
#[test]
fn a_shadow_item_is_never_dropped_on_death() {
    let (mut world, _db, _l) = drop_world();
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 0, 0);
    let _k = ingame_caster(&mut world, KILLER_CID, KILLER, 50, 0);
    pk_victim(&mut world, 0);
    // A **non-stackable** template, like every real shadow item (they are
    // weapons and armour): two `give`s must yield two instances, not one
    // stack of two, or the instance-vs-template distinction this test exists
    // to prove cannot be set up at all.
    {
        let mut t = crate::data::item_data::ItemTemplate {
            item_id: LOOT_ITEM,
            name: "Shadow Weapon".into(),
            ..items_tests_template()
        };
        t.type2 = 5;
        t.is_stackable = false;
        world.data.item_data.insert_for_test(t);
    }

    // Two instances of the *same* template: one ordinary, one shadow.
    let plain_oid = 0x6100_0000;
    let shadow_oid = 0x6100_0001;
    give(&mut world, VICTIM, LOOT_ITEM, 1, plain_oid);
    give(&mut world, VICTIM, LOOT_ITEM, 1, shadow_oid);
    // `Item._mana >= 0` is what makes an instance shadow.
    world
        .objects
        .get_component_mut::<Inventory>(&VICTIM)
        .unwrap()
        .set_mana_left(shadow_oid, 20);
    // Drop everything that is eligible.
    world.cfg.rates.karma_drop_limit = 10;
    world.forced_rolls.clear();
    world.forced_rolls.extend([0; 32]);

    kill_by_player(&mut world);

    let still_held: Vec<i32> = world
        .objects
        .get_component::<Inventory>(&VICTIM)
        .map(|i| i.items().iter().map(|x| x.object_id).collect())
        .unwrap_or_default();
    assert!(
        still_held.contains(&shadow_oid),
        "the shadow instance is kept, like retail: {still_held:x?}"
    );
    assert!(
        !still_held.contains(&plain_oid),
        "while the ordinary copy of the same item id drops: {still_held:x?}"
    );
}
