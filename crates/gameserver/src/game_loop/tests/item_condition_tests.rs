//! `ItemTemplate.checkCondition` — the `<cond>` gate on equipping and using an
//! item, and the `checkItemRestriction` sweep that takes off what stopped
//! qualifying (`game_loop::items::conditions`).

use super::*;
use crate::data::item_cond::{Cond, CondMessage, ItemCondition};
use crate::data::item_data::{ItemKind, ItemTemplate, SLOT_BACK, SLOT_HEAD, SLOT_R_HAND};
use crate::enums::Race;
use crate::network::server_packets::sm_ids;

const CID: u32 = 1;
const PLAYER: i32 = 3001;
/// The gated item's template id and the object id it is given as.
const ITEM: i32 = 8100;
const ITEM_OID: i32 = 9500;

/// A one-block gated item. `msg_id` is the block's own refusal line, exactly as
/// the datapack spells it.
fn gated_item(world: &mut World, item_id: i32, body_part: i32, node: Cond, msg_id: i16) {
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id,
        name: format!("Gated {item_id}"),
        kind: ItemKind::Armor,
        body_part,
        pre_conditions: vec![ItemCondition {
            node,
            message: CondMessage::Sm {
                id: msg_id,
                add_name: false,
            },
        }],
        ..ItemTemplate::for_test()
    });
}

/// A world with one in-game player holding `ITEM_OID`, plus that player's
/// packet queue drained of the login burst.
fn world_with_gated_item(
    node: Cond,
    msg_id: i16,
) -> (World, UnboundedReceiver<bytes::Bytes>, db::CmdRx) {
    let (mut world, db, _l) = cast_test_world();
    let mut rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    gated_item(&mut world, ITEM, SLOT_HEAD, node, 1518);
    let _ = msg_id;
    let World { objects, data, .. } = &mut world;
    objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .add_item(&data.item_data, ITEM_OID, ITEM, 1);
    drain(&mut rx);
    (world, rx, db)
}

/// A minimal clan the conditions can read: id, leader and level are all they
/// look at, plus the castle id one test moves.
fn test_clan(id: i32, leader_id: i32) -> Clan {
    Clan {
        id,
        name: format!("Clan{id}"),
        leader_id,
        level: 5,
        reputation_score: 0,
        castle_id: 0,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    }
}

fn is_equipped(world: &World, item_object_id: i32) -> bool {
    world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .unwrap()
        .paperdoll_slot_of(item_object_id)
        .is_some()
}

fn set_player(world: &mut World, f: impl FnOnce(&mut Player)) {
    f(world.objects.get_component_mut::<Player>(&PLAYER).unwrap());
}

/// The gate itself: a race-locked item is not worn, and the refusal is the
/// **block's own** message id rather than a generic one.
#[test]
fn a_race_gated_item_is_refused_with_the_blocks_own_message() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::Race(vec![Race::Kamael]), 1518);
    // `dummy_char` is a Human, and no Kamael can exist on this chronicle —
    // which is what makes the 698 Kamael-gated items unwearable here.
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));

    assert!(!is_equipped(&world, ITEM_OID), "the race gate held");
    assert!(
        has_system_message(&drain(&mut rx), 1518),
        "`<cond msgId=1518>` is what the player is told"
    );

    // The same item on a matching race goes on.
    set_player(&mut world, |p| p.race = Race::Kamael.ordinal());
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID));
}

/// Java gates on `!item.isEquipped()`: the check stops an item going **on**,
/// never coming off. Without that leg a player whose state changed while
/// wearing something would be locked into it.
#[test]
fn taking_a_gated_item_off_is_never_refused() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::Level(80), 1518);
    set_player(&mut world, |p| p.level = 80);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID), "qualified, so it went on");

    set_player(&mut world, |p| p.level = 1);
    drain(&mut rx);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(
        !is_equipped(&world, ITEM_OID),
        "no longer qualified, but still allowed to take it off"
    );
    assert!(
        !has_system_message(&drain(&mut rx), 1518),
        "and told nothing"
    );
}

/// An `<and>` of two `<player>` elements — the "Pledge Shield" shape — needs
/// both, which a per-leaf evaluation could pass on either.
#[test]
fn an_and_block_needs_every_leaf() {
    let (mut world, _rx, _db) =
        world_with_gated_item(Cond::And(vec![Cond::Level(40), Cond::Sex(1)]), 1518);
    set_player(&mut world, |p| {
        p.level = 40;
        p.is_female = false;
    });
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID), "level alone is not enough");

    set_player(&mut world, |p| p.is_female = true);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID));
}

/// `ConditionPlayerPkCount` is an inclusive **maximum** (`getPkKills() <=
/// value`) — read as a minimum it would gate the 16 items that use it exactly
/// backwards.
#[test]
fn pk_count_is_a_maximum_not_a_minimum() {
    let (mut world, _rx, _db) = world_with_gated_item(Cond::PkCount(0), 1685);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID), "0 kills passes `<= 0`");

    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID)); // take off
    set_player(&mut world, |p| p.pk_kills = 1);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID), "one kill is one too many");
}

/// A clan leader satisfies **any** `pledgeClass`, and `-1` means leaders only —
/// the two special cases in `ConditionPlayerPledgeClass`, either of which a
/// plain `>=` would get wrong.
#[test]
fn a_clan_leader_passes_any_pledge_class_and_minus_one_means_leaders_only() {
    let (mut world, _rx, _db) = world_with_gated_item(Cond::PledgeClass(8), 1518);
    // Someone else leads, so the rank has to stand on its own.
    world.clans.insert(77, test_clan(77, PLAYER + 1));
    set_player(&mut world, |p| {
        p.clan_id = 77;
        p.pledge_class = 4;
    });
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID), "rank 4 is below 8");

    world.clans.get_mut(&77).unwrap().leader_id = PLAYER;
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(
        is_equipped(&world, ITEM_OID),
        "the leader passes regardless"
    );

    // `-1`: a rank the leader has and nobody else can reach.
    gated_item(
        &mut world,
        ITEM + 1,
        SLOT_R_HAND,
        Cond::PledgeClass(-1),
        1518,
    );
    let World { objects, data, .. } = &mut world;
    objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .add_item(&data.item_data, ITEM_OID + 1, ITEM + 1, 1);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID + 1));
    assert!(is_equipped(&world, ITEM_OID + 1));

    world.clans.get_mut(&77).unwrap().leader_id = PLAYER + 1;
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID + 1)); // off
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID + 1));
    assert!(
        !is_equipped(&world, ITEM_OID + 1),
        "-1 with no crown fails even at the highest rank"
    );
}

/// `GMItemRestriction` is **True** on this dist, which is what makes the
/// override inert here — the whole reason the key had nothing to gate before
/// the conditions landed.
#[test]
fn the_gm_override_is_switched_off_by_gm_item_restriction() {
    let (mut world, _rx, _db) = world_with_gated_item(Cond::Race(vec![Race::Kamael]), 1518);
    set_player(&mut world, |p| {
        p.cond_overrides = 1u64 << crate::game_loop::admin::ITEM_CONDITIONS_ORDINAL;
    });
    // The shipped `General.ini` sets `GMItemRestriction = True`; the test
    // world builds a default config, so the dist value is set here rather
    // than assumed.
    world.cfg.general.gm_item_restriction = true;
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(
        !is_equipped(&world, ITEM_OID),
        "GMItemRestriction=True puts the override-holder back under the rules"
    );

    world.cfg.general.gm_item_restriction = false;
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID), "…and off, they bypass it");
}

/// The Olympiad arm sits *before* the `<cond>` loop and picks its message by
/// whether the item is equippable.
#[test]
fn olympiad_restricted_and_hero_items_are_refused_inside_a_match() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::Level(1), 1518);
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: ITEM,
        name: "Restricted".into(),
        kind: ItemKind::Armor,
        body_part: SLOT_HEAD,
        is_oly_restricted: true,
        ..ItemTemplate::for_test()
    });
    world.olympiad.in_competition.insert(PLAYER);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID));
    assert!(
        has_system_message(
            &drain(&mut rx),
            sm_ids::YOU_CANNOT_EQUIP_THAT_ITEM_IN_A_OLYMPIAD_MATCH
        ),
        "the equip wording, not the use one"
    );

    world.olympiad.in_competition.remove(&PLAYER);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID), "outside a match it is gear");
}

/// The hero range is computed from the id (`_heroItem`), not read from a flag,
/// so an item with no `<cond>` at all still hits the Olympiad arm.
#[test]
fn a_hero_item_is_recognised_by_its_id_range() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::Level(1), 1518);
    // 6842 — the hero circlet, the one id outside the two ranges.
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 6842,
        name: "Wings of Destiny Circlet".into(),
        kind: ItemKind::Armor,
        body_part: SLOT_HEAD,
        ..ItemTemplate::for_test()
    });
    let World { objects, data, .. } = &mut world;
    objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .add_item(&data.item_data, ITEM_OID + 5, 6842, 1);
    world.olympiad.in_competition.insert(PLAYER);
    drain(&mut rx);

    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID + 5));
    assert!(!is_equipped(&world, ITEM_OID + 5));
    assert!(has_system_message(
        &drain(&mut rx),
        sm_ids::YOU_CANNOT_EQUIP_THAT_ITEM_IN_A_OLYMPIAD_MATCH
    ));
}

/// `_isEventRestricted` answers with a plain `sendMessage`, which is `S1_TEXT`
/// on the wire — not a `SystemMessageId` of its own.
#[test]
fn an_event_restricted_item_is_refused_while_on_an_event() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::Level(1), 1518);
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: ITEM,
        name: "Event-locked".into(),
        kind: ItemKind::Armor,
        body_part: SLOT_HEAD,
        is_event_restricted: true,
        ..ItemTemplate::for_test()
    });
    set_player(&mut world, |p| p.on_event = true);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID));
    assert!(has_system_message(&drain(&mut rx), sm_ids::S1_TEXT));
}

/// The other half of the gate: what is already worn is re-judged when the
/// state behind the condition moves, and comes off with Java's per-item line.
#[test]
fn check_item_restriction_strips_what_stopped_qualifying() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::PkCount(0), 1685);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID));
    drain(&mut rx);

    // A PK pushes the wearer past `pkCount="0"`.
    set_player(&mut world, |p| p.pk_kills = 1);
    items::check_item_restriction(&mut world, PLAYER);

    assert!(!is_equipped(&world, ITEM_OID), "stripped");
    assert!(
        has_system_message(&drain(&mut rx), sm_ids::S1_HAS_BEEN_UNEQUIPPED),
        "the unenchanted removal line"
    );
}

/// The enchanted variant of the same message, which carries the enchant level
/// as well as the name.
#[test]
fn a_stripped_enchanted_item_gets_the_enchant_wording() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::PkCount(0), 1685);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    {
        let inv = world
            .objects
            .get_component_mut::<Inventory>(&PLAYER)
            .unwrap();
        inv.set_enchant_level(ITEM_OID, 5);
    }
    drain(&mut rx);

    set_player(&mut world, |p| p.pk_kills = 1);
    items::check_item_restriction(&mut world, PLAYER);
    let pkts = drain(&mut rx);
    assert!(!is_equipped(&world, ITEM_OID));
    assert!(has_system_message(
        &pkts,
        sm_ids::THE_EQUIPMENT_S1_S2_HAS_BEEN_REMOVED
    ));
    assert!(!has_system_message(&pkts, sm_ids::S1_HAS_BEEN_UNEQUIPPED));
}

/// Java `return`s out of the whole sweep when a **cloak** comes off, and says
/// so with the armour-set line. Reproduced deliberately: the message is the
/// one `cloakStatus` exists for.
#[test]
fn a_failing_cloak_ends_the_sweep_with_its_own_message() {
    let (mut world, mut rx, _db) = world_with_gated_item(Cond::PkCount(0), 1685);
    gated_item(&mut world, ITEM + 2, SLOT_BACK, Cond::PkCount(0), 1685);
    let World { objects, data, .. } = &mut world;
    objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .add_item(&data.item_data, ITEM_OID + 2, ITEM + 2, 1);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID + 2));
    assert!(is_equipped(&world, ITEM_OID + 2), "cloak on");
    drain(&mut rx);

    set_player(&mut world, |p| p.pk_kills = 1);
    items::check_item_restriction(&mut world, PLAYER);
    let pkts = drain(&mut rx);
    assert!(!is_equipped(&world, ITEM_OID + 2));
    assert!(has_system_message(
        &pkts,
        sm_ids::YOUR_CLOAK_HAS_BEEN_UNEQUIPPED_BECAUSE_YOUR_ARMOR_SET_IS_NO_LONGER_COMPLETE
    ));
    assert!(
        !has_system_message(&pkts, sm_ids::S1_HAS_BEEN_UNEQUIPPED),
        "the cloak arm returns instead of falling through to the generic line"
    );
}

/// `cloakStatus` is `PlayerStat._cloakSlot`, whose setter has no caller in the
/// Java tree — so the 49 items gated on it are unwearable on both sides.
#[test]
fn the_cloak_status_gate_is_closed_for_everyone() {
    let (mut world, _rx, _db) = world_with_gated_item(Cond::CloakStatus(true), 2453);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(
        !is_equipped(&world, ITEM_OID),
        "nothing sets the cloak slot, in Java either"
    );
}

/// A castle condition reads the *clan's* castle, and `-1` means "any".
#[test]
fn the_castle_gate_reads_the_clans_castle() {
    let (mut world, _rx, _db) = world_with_gated_item(Cond::HasCastle(-1), 1518);
    world.clans.insert(77, test_clan(77, PLAYER));
    set_player(&mut world, |p| p.clan_id = 77);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID), "a clan with no castle");

    world.clans.get_mut(&77).unwrap().castle_id = 3;
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID));
}

/// `instanceId` matches the instance **template** id, not the runtime id the
/// player's component carries.
#[test]
fn the_instance_gate_matches_the_template_id() {
    let (mut world, _rx, _db) = world_with_gated_item(Cond::InstanceId(vec![43, 44]), 1518);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID), "not in an instance");

    let instance_id = world.instances.create(44);
    world
        .objects
        .add_components(&PLAYER, crate::model::components::InstanceId(instance_id));
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID), "inside template 44");
}

/// `MinimumVitalityPoints` is a floor on the live vitality pool.
#[test]
fn the_vitality_gate_is_a_floor() {
    let (mut world, _rx, _db) = world_with_gated_item(Cond::MinimumVitalityPoints(35_000), 1518);
    set_player(&mut world, |p| p.vitality_points = 34_999);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(!is_equipped(&world, ITEM_OID));

    set_player(&mut world, |p| p.vitality_points = 35_000);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID));
}

/// `categoryType` resolves against `CategoryData.xml` with the wearer's id —
/// a player's **class** id.
#[test]
fn the_category_gate_reads_the_players_class_id() {
    let (mut world, _rx, _db) =
        world_with_gated_item(Cond::CategoryType(vec!["TEST_GROUP".into()]), 1518);
    world
        .data
        .categories
        .insert_for_test("TEST_GROUP", &[0, 1, 4]);
    set_player(&mut world, |p| p.class_id = 7);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(
        !is_equipped(&world, ITEM_OID),
        "class 7 is not in the group"
    );

    set_player(&mut world, |p| p.class_id = 4);
    items::handle_use_item(&mut world, CID, &use_item_body(ITEM_OID));
    assert!(is_equipped(&world, ITEM_OID));
}
