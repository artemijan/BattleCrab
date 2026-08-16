//! Mercenary posting (`handlers/itemhandlers/MercTicket` +
//! `SiegeGuardManager`'s hired half).
//!
//! The 499 posting tickets across the nine castles were inert: `MercTicket` was
//! not in `ItemHandler`'s name match, so a ticket used on your own castle
//! grounds was consumed as a no-op and no defender ever appeared.

use super::*;

use crate::data::castle_siege_guards::SiegeGuardHolder;
use crate::data::zone_data::{Zone, ZoneKind};
use crate::game_loop::siege::{handle_mercenary_confirm, use_mercenary_ticket};
use crate::model::Player;
use crate::model::castle::{Castle, CastleSide};
use crate::model::clan::Clan;

const CASTLE: i32 = 3;
const LORD: i32 = 9601;
const CID: u32 = 1;
const CLAN: i32 = 77;
/// Gludio's "Greater Mercenary Posting Ticket (Sword/Stationary)" pair.
const TICKET: i32 = 6038;
const GUARD_NPC: i32 = 35030;

fn siege_zone(castle_id: i32) -> Zone {
    Zone {
        id: 0,
        name: format!("test_siege_{castle_id}"),
        kind: ZoneKind::Siege,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -2000,
                x2: 2000,
                y1: -2000,
                y2: 2000,
            },
            min_z: -2000,
            max_z: 2000,
        },
        castle_id,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
        mother_tree: None,
    }
}

/// A castle owned by `CLAN`, its siege zone, one guard row, and a lord standing
/// inside with a ticket and the `CS_MERCENARIES` privilege.
fn merc_world(max_npc_amount: i32) -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4600_0000..0x4600_0200;
    world.data.zone_data.insert(siege_zone(CASTLE));
    world
        .data
        .castle_siege_guards
        .insert_holder_for_test(SiegeGuardHolder {
            castle_id: CASTLE,
            item_id: TICKET,
            npc_id: GUARD_NPC,
            max_npc_amount,
            stationary: true,
        });
    world.castles = vec![Castle {
        show_npc_crest: false,
        id: CASTLE,
        name: "Giran".into(),
        side: CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    // Ownership lives on the clan (`Clan.castle_id`), which is what the
    // handler's `owns` check reads.
    world.clans.insert(
        CLAN,
        Clan {
            id: CLAN,
            name: "Owners".into(),
            leader_id: LORD,
            level: 5,
            reputation_score: 0,
            castle_id: CASTLE,
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
        },
    );

    let rx = ingame_player(&mut world, CID, LORD, 0, 0, 0);
    if let Some(p) = world.objects.get_component_mut::<Player>(&LORD) {
        p.clan_id = CLAN;
    }
    items::add_inventory_item(&mut world, LORD, TICKET, 5).unwrap();
    (world, rx)
}

fn ticket_oid(world: &World) -> i32 {
    item_oid(world, LORD, TICKET)
}

fn place(world: &mut World) {
    let obj = ticket_oid(world);
    use_mercenary_ticket(world, CID, LORD, obj, TICKET);
    handle_mercenary_confirm(world, LORD, true);
}

/// **The posting loop.** Using a ticket asks for confirmation; answering yes
/// records the mercenary, drops the ticket that marks the spot, and spends one
/// from the bag.
#[test]
fn a_confirmed_ticket_posts_a_mercenary() {
    let (mut world, mut rx) = merc_world(5);
    drain(&mut rx);

    let obj = ticket_oid(&world);
    use_mercenary_ticket(&mut world, CID, LORD, obj, TICKET);

    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CONFIRM_DLG),
        "the placement is confirmed first"
    );
    assert!(
        world.mercenaries.get(&CASTLE).is_none_or(|v| v.is_empty()),
        "and nothing is posted until it is answered"
    );

    handle_mercenary_confirm(&mut world, LORD, true);

    let posted = world.mercenaries.get(&CASTLE).expect("posted");
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0].npc_id, GUARD_NPC, "the guard the ticket names");
    assert_ne!(posted[0].ticket_oid, 0, "the ticket marks the spot");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&LORD)
            .unwrap()
            .count_of(TICKET),
        4,
        "one ticket spent"
    );
}

/// Declining leaves everything as it was — Java destroys the item only on the
/// yes branch.
#[test]
fn a_declined_ticket_is_kept() {
    let (mut world, _rx) = merc_world(5);
    let obj = ticket_oid(&world);

    use_mercenary_ticket(&mut world, CID, LORD, obj, TICKET);
    handle_mercenary_confirm(&mut world, LORD, false);

    assert!(world.mercenaries.get(&CASTLE).is_none_or(|v| v.is_empty()));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&LORD)
            .unwrap()
            .count_of(TICKET),
        5,
        "the ticket is still in the bag"
    );
}

/// `isTooCloseToAnotherTicket` — 25 units. Two postings cannot share a spot.
#[test]
fn mercenaries_cannot_be_posted_on_top_of_each_other() {
    let (mut world, _rx) = merc_world(5);
    place(&mut world);
    assert_eq!(world.mercenaries[&CASTLE].len(), 1);

    // Same position → refused.
    place(&mut world);
    assert_eq!(
        world.mercenaries[&CASTLE].len(),
        1,
        "the second posting is too close"
    );

    // 30 units away → accepted.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&LORD)
    {
        p.x = 30;
    }
    place(&mut world);
    assert_eq!(world.mercenaries[&CASTLE].len(), 2, "far enough apart");
}

/// `isAtNpcLimit` — the `<guard>` row's `npcMaxAmount` caps how many of that
/// ticket a castle may field.
#[test]
fn the_guard_row_caps_how_many_can_be_posted() {
    let (mut world, _rx) = merc_world(1);
    place(&mut world);

    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&LORD)
    {
        p.x = 500;
    }
    place(&mut world);

    assert_eq!(
        world.mercenaries[&CASTLE].len(),
        1,
        "npcMaxAmount = 1 means one"
    );
}

/// Only the owning clan, and only with `CS_MERCENARIES`.
#[test]
fn an_outsider_cannot_post_mercenaries() {
    let (mut world, mut rx) = merc_world(5);
    // Not in the owning clan any more.
    if let Some(p) = world.objects.get_component_mut::<Player>(&LORD) {
        p.clan_id = 0;
    }
    drain(&mut rx);

    let obj = ticket_oid(&world);
    use_mercenary_ticket(&mut world, CID, LORD, obj, TICKET);

    assert!(
        drain(&mut rx)
            .iter()
            .all(|p| p[0] != server_packets::opcodes::CONFIRM_DLG),
        "no prompt for someone with no authority"
    );
    assert!(world.mercenaries.get(&CASTLE).is_none_or(|v| v.is_empty()));
}

/// `castle.getSiege().isInProgress()` — postings are a between-sieges activity.
#[test]
fn mercenaries_cannot_be_posted_once_the_siege_starts() {
    let (mut world, _rx) = merc_world(5);
    let mut siege = crate::model::siege::Siege::new(CASTLE);
    siege.in_progress = true;
    world.sieges.insert(CASTLE, siege);

    place(&mut world);

    assert!(world.mercenaries.get(&CASTLE).is_none_or(|v| v.is_empty()));
}

/// **The payoff.** A posting is only a row and a ticket on the ground until the
/// siege starts; then the mercenary stands up beside the stationed garrison
/// (`SiegeGuardManager.spawnSiegeGuard`'s hired half).
#[test]
fn posted_mercenaries_spawn_when_the_siege_starts() {
    let (mut world, _rx) = merc_world(5);
    let mut t = crate::data::npc_data::default_template(GUARD_NPC);
    t.type_name = "Defender".into();
    t.name = "Mercenary".into();
    t.level = 60;
    t.base_hp_max = 3000.0;
    world.data.npc_data.insert_for_test(t);
    // `start_siege` is a no-op without a registered siege for the castle.
    world
        .sieges
        .insert(CASTLE, crate::model::siege::Siege::new(CASTLE));
    place(&mut world);
    let spot = *world
        .objects
        .get_component::<crate::model::components::Position>(&LORD)
        .unwrap();

    let count = |w: &mut World| {
        let mut n = 0;
        w.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
            if x.npc_id == GUARD_NPC {
                n += 1;
            }
        });
        n
    };
    let before = count(&mut world);
    crate::game_loop::siege::start_siege(&mut world, CASTLE);

    assert_eq!(
        count(&mut world),
        before + 1,
        "the hired mercenary is on the field"
    );
    // `npc.spawnMe(x, y, z + 20)`.
    let mut placed = None;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &crate::model::components::Position)>(
            |(npc, pos)| {
                if npc.npc_id == GUARD_NPC {
                    placed = Some(*pos);
                }
            },
        );
    let placed = placed.expect("spawned");
    assert_eq!(
        (placed.x, placed.y),
        (spot.x, spot.y),
        "where it was posted"
    );
    assert_eq!(placed.z, spot.z + 20, "lifted 20 units, as Java does");
}

/// A change of ownership wipes the former lord's postings — the rows, the
/// tickets on the ground, and the map entry (`deleteTickets` +
/// `removeSiegeGuards`).
#[test]
fn a_change_of_ownership_clears_the_postings() {
    let (mut world, _rx) = merc_world(5);
    place(&mut world);
    let ticket_ground_oid = world.mercenaries[&CASTLE][0].ticket_oid;
    assert!(
        world
            .objects
            .has_component::<crate::model::components::GroundItem>(&ticket_ground_oid),
        "the ticket is on the ground"
    );

    crate::game_loop::siege::clear_castle_mercenaries(&mut world, CASTLE);

    assert!(
        world.mercenaries.get(&CASTLE).is_none_or(|v| v.is_empty()),
        "postings gone"
    );
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::GroundItem>(&ticket_ground_oid),
        "and the ticket with them"
    );
}
