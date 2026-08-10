//! Sell buffs (`Custom/SellBuffs.ini`) — the player buff shop: listing, the
//! price/count gates, the transaction, and who pays what.

use super::*;
use crate::game_loop::helpers::count_of;

use crate::model::components::{SkillBook, Vitals};
use crate::model::inventory::Inventory;

const SELLER: i32 = 3001;
const BUYER: i32 = 3002;
const ADENA: i32 = 57;
/// A cheap buff the seller knows and the whitelist allows.
const BUFF: i32 = 1204;

/// A world with the feature on, both players in range, and one sellable buff.
fn sell_world() -> (
    World,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
) {
    let (mut world, ..) = test_world();
    world.id_pool = 0x4A00_0000..0x4A00_0100;
    world.cfg.sell_buffs.enabled = true;
    world.cfg.sell_buffs.min_price = 100;
    world.cfg.sell_buffs.max_price = 10_000;
    world.cfg.sell_buffs.max_buffs = 2;
    world.cfg.sell_buffs.payment_id = ADENA;
    world.cfg.sell_buffs.mp_multiplier = 1;
    world.data.sell_buff_data.insert_for_test(BUFF);

    let mut adena = crate::data::item_data::ItemTemplate::default();
    adena.item_id = ADENA;
    adena.name = "Adena".into();
    adena.is_stackable = true;
    world.data.item_data.insert_for_test(adena);

    let mut skill = crate::model::skill::Skill {
        self_continuous: false,
        id: BUFF,
        level: 1,
        name: "Wind Walk".into(),
        mp_consume: 50,
        ..Default::default()
    };
    skill.effects = vec![];
    world.data.skill_data.insert_for_test(skill);

    let seller_rx = ingame_player(&mut world, 1, SELLER, 0, 0, 0);
    let buyer_rx = ingame_player(&mut world, 2, BUYER, 50, 0, 0);
    world
        .objects
        .get_component_mut::<SkillBook>(&SELLER)
        .unwrap()
        .0
        .insert(BUFF, 1);
    // The seller needs a peace zone to open in and MP to spend.
    for oid in [SELLER, BUYER] {
        world
            .objects
            .get_component_mut::<crate::model::components::ZoneFlags>(&oid)
            .unwrap()
            .mask |= crate::data::zone_data::ZoneKind::Peace.bit();
    }
    let v = world.objects.get_component_mut::<Vitals>(&SELLER).unwrap();
    v.max_mp = 500;
    v.cur_mp = 500.0;
    (world, seller_rx, buyer_rx)
}

fn bypass(world: &mut World, client_id: u32, cmd: &str) {
    let (c, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
    let oid = if client_id == 1 { SELLER } else { BUYER };
    crate::game_loop::sell_buffs::handle_bypass(world, client_id, oid, c, rest);
}

fn listed(world: &World) -> Vec<(i32, i64)> {
    world
        .objects
        .get_component::<Player>(&SELLER)
        .unwrap()
        .sell_buff_list
        .clone()
}

/// The price bounds and the list cap, each with its own refusal.
#[test]
fn listing_a_buff_respects_the_price_and_count_limits() {
    let (mut world, mut rx, _b) = sell_world();
    drain(&mut rx);

    // Under the minimum.
    bypass(&mut world, 1, &format!("sellbuffaddskill {BUFF} 50"));
    assert!(listed(&world).is_empty(), "too cheap");
    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("Too small price")),
    );

    // Over the maximum.
    bypass(&mut world, 1, &format!("sellbuffaddskill {BUFF} 99999"));
    assert!(listed(&world).is_empty(), "too dear");

    // Just right.
    bypass(&mut world, 1, &format!("sellbuffaddskill {BUFF} 500"));
    assert_eq!(listed(&world), vec![(BUFF, 500)]);

    // A skill the seller doesn't know is refused outright.
    bypass(&mut world, 1, "sellbuffaddskill 9999 500");
    assert_eq!(listed(&world).len(), 1, "unknown skill ignored");

    // Re-pricing works and does *not* re-check the bounds (Java quirk).
    bypass(&mut world, 1, &format!("sellbuffchangeprice {BUFF} 1"));
    assert_eq!(
        listed(&world),
        vec![(BUFF, 1)],
        "Java skips the bounds here"
    );

    // Removing empties the list again.
    bypass(&mut world, 1, &format!("sellbuffremove {BUFF}"));
    assert!(listed(&world).is_empty());
}

/// The shop opens only with a non-empty list, and opening seats the seller and
/// sets the package-sell store type other clients render.
#[test]
fn starting_the_shop_seats_the_seller() {
    let (mut world, mut rx, _b) = sell_world();

    // Empty list → refusal, no shop.
    bypass(&mut world, 1, "sellbuffstart My Shop");
    assert!(!crate::game_loop::sell_buffs::is_selling(&world, SELLER));
    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("list of buffs is empty")),
    );

    bypass(&mut world, 1, &format!("sellbuffaddskill {BUFF} 500"));
    bypass(&mut world, 1, "sellbuffstart My Shop");
    assert!(crate::game_loop::sell_buffs::is_selling(&world, SELLER));
    let p = world.objects.get_component::<Player>(&SELLER).unwrap();
    assert!(p.sitting, "a seller sits");
    assert_eq!(p.store_type, 8, "PACKAGE_SELL");

    // …and stopping puts it all back.
    bypass(&mut world, 1, "sellbuffstop");
    let p = world.objects.get_component::<Player>(&SELLER).unwrap();
    assert!(!p.selling_buffs && p.store_type == 0);
    assert_eq!(listed(&world).len(), 1, "the list survives a stop");
}

/// The transaction: the **buyer** pays the price, the **seller** pays the MP,
/// and the buff lands on the buyer.
#[test]
fn buying_a_buff_moves_money_mana_and_the_skill() {
    let (mut world, _s, mut buyer_rx) = sell_world();
    bypass(&mut world, 1, &format!("sellbuffaddskill {BUFF} 500"));
    bypass(&mut world, 1, "sellbuffstart Shop");
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&BUYER)
            .unwrap()
            .add_item(&data.item_data, 0x4A00_0050, ADENA, 2000);
    }
    let mp_of = |w: &World| w.objects.get_component::<Vitals>(&SELLER).unwrap().cur_mp;

    drain(&mut buyer_rx);
    bypass(
        &mut world,
        2,
        &format!("sellbuffbuyskill {SELLER} {BUFF} 0"),
    );
    assert_eq!(count_of(&world, BUYER, ADENA), 1500, "the buyer paid");
    assert_eq!(count_of(&world, SELLER, ADENA), 500, "the seller was paid");
    assert_eq!(mp_of(&world), 450.0, "the seller paid the MP");

    // A seller out of mana refuses with a message rather than casting.
    world
        .objects
        .get_component_mut::<Vitals>(&SELLER)
        .unwrap()
        .cur_mp = 10.0;
    drain(&mut buyer_rx);
    bypass(
        &mut world,
        2,
        &format!("sellbuffbuyskill {SELLER} {BUFF} 0"),
    );
    assert_eq!(adena_of(&world, BUYER), 1500, "no charge when refused");
    assert!(
        drain(&mut buyer_rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("no enough mana")),
    );
}

/// Out of interaction range, the shop is not reachable at all — neither the
/// menu nor the purchase.
#[test]
fn a_distant_buyer_cannot_reach_the_shop() {
    let (mut world, _s, _b) = sell_world();
    bypass(&mut world, 1, &format!("sellbuffaddskill {BUFF} 500"));
    bypass(&mut world, 1, "sellbuffstart Shop");
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&BUYER)
            .unwrap()
            .add_item(&data.item_data, 0x4A00_0051, ADENA, 2000);
    }
    world
        .objects
        .get_component_mut::<crate::model::components::Position>(&BUYER)
        .unwrap()
        .x = 5000;

    bypass(
        &mut world,
        2,
        &format!("sellbuffbuyskill {SELLER} {BUFF} 0"),
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&BUYER)
            .map_or(0, |i| i.count_of(ADENA)),
        2000,
        "nothing changed hands across the map"
    );
}

/// A buff seller may not also open an ordinary private store — Java's
/// `canOpenPrivateStore` reads `!_isSellingBuffs`.
#[test]
fn a_buff_seller_cannot_open_a_normal_store() {
    let (mut world, ..) = sell_world();
    assert!(crate::game_loop::private_store::can_open_private_store(
        &world, 1, SELLER
    ));
    bypass(&mut world, 1, &format!("sellbuffaddskill {BUFF} 500"));
    bypass(&mut world, 1, "sellbuffstart Shop");
    assert!(
        !crate::game_loop::private_store::can_open_private_store(&world, 1, SELLER),
        "the buff shop blocks the ordinary one"
    );
}
