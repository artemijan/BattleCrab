//! The clan warehouse and the privilege that gates withdrawal.

use super::*;

/// Clan warehouse: a shared container. The leader deposits (persisted), an
/// unprivileged member is denied the withdraw window, and the leader withdraws.
#[test]
fn clan_warehouse_withdrawal_is_leader_only_at_the_shipped_setting() {
    use crate::game_loop::commerce::warehouse;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::commerce::ActiveWarehouse;
    let (mut world, _tx, _db_rx, _lrx) = admin_world();
    let _leader_rx = ingame_player_access(&mut world, 1, 3001, 0);
    let _member_rx = ingame_player_access(&mut world, 2, 3002, 0);

    let clan_id = 0x7000_0009;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "WhGate".into(),
            leader_id: 3001,
            level: 1,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001), cm(3002)],
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
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = clan_id;
    }
    // **3002 holds the view-warehouse privilege.** That is the whole point:
    // under the port's old unconditional privilege gate this member could
    // withdraw, and on this dist they must not be able to.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_privs = model::clan::ALL_CLAN_PRIVILEGES;
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_privs = model::clan::CL_VIEW_WAREHOUSE;

    let opened = |world: &mut World, client: u32, oid: i32| {
        world.objects.remove_component::<ActiveWarehouse>(&oid);
        warehouse::open_clan(world, client, oid, true);
        world.objects.has_component::<ActiveWarehouse>(&oid)
    };

    // Shipped: `AltMembersCanWithdrawFromClanWH = False` → leader only.
    assert!(!world.cfg.character.alt_members_can_withdraw_from_clan_wh);
    assert!(opened(&mut world, 1, 3001), "the leader may withdraw");
    assert!(
        !opened(&mut world, 2, 3002),
        "a privileged member must NOT withdraw while the key is off"
    );

    // Turned on: the privilege becomes the gate instead.
    world.cfg.character.alt_members_can_withdraw_from_clan_wh = true;
    assert!(opened(&mut world, 2, 3002), "…and may once the key is on");
}

#[test]
fn clan_warehouse_shared_deposit_withdraw_and_privilege() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::inventory::Inventory;
    let (mut world, _tx, mut db_rx, _lrx) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut leader_rx = ingame_player_access(&mut world, 1, 3001, 0);
    let mut member_rx = ingame_player_access(&mut world, 2, 3002, 0);
    drain(&mut leader_rx);
    drain(&mut member_rx);

    // A level-1 clan: 3001 leader, 3002 plain member (no privileges).
    let clan_id = 0x7000_0001;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "WhClan".into(),
            leader_id: 3001,
            level: 1,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001), cm(3002)],
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
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = clan_id;
    }
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_privs = model::clan::ALL_CLAN_PRIVILEGES;
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .clan_privs = 0;

    // Leader deposits 500 adena into the shared clan warehouse.
    inventory::add_inventory_item(&mut world, 3001, 57, 500).unwrap();
    let adena_oid = item_oid(&world, 3001, 57);
    warehouse::open_clan(&mut world, 1, 3001, false); // keeper bypass → active = clan
    let deposit = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::SEND_WARE_HOUSE_DEPOSIT_LIST);
        w.write_i32(1);
        w.write_i32(adena_oid);
        w.write_i64(500);
        w.into_bytes()
    };
    on_packet(&mut world, 1, deposit);
    assert_eq!(
        world.clans[&clan_id].warehouse.0.count_of(57),
        500,
        "deposited into clan warehouse"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(57),
        0,
        "left leader inventory"
    );
    // Persistence flush emitted for the clan.
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::StoreClanWarehouse { clan_id: cid, items } if *cid == clan_id && items.iter().any(|r| r.item_id == 57 && r.count == 500 && r.loc == "CLANWH"))), "clan warehouse persisted");

    // An unprivileged member cannot open the withdraw window.
    drain(&mut member_rx);
    warehouse::open_clan(&mut world, 2, 3002, true);
    let denied = drain(&mut member_rx);
    assert!(
        !denied
            .iter()
            .any(|p| p[0] == server_packets::opcodes::WAREHOUSE_WITHDRAW_LIST),
        "member without CL_VIEW_WAREHOUSE is denied"
    );

    // The leader withdraws 200 back — the shared container drops to 300.
    let wh_oid = world.clans[&clan_id]
        .warehouse
        .0
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;
    warehouse::open_clan(&mut world, 1, 3001, true);
    let withdraw = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST);
        w.write_i32(1);
        w.write_i32(wh_oid);
        w.write_i64(200);
        w.into_bytes()
    };
    on_packet(&mut world, 1, withdraw);
    assert_eq!(
        world.clans[&clan_id].warehouse.0.count_of(57),
        300,
        "300 remains in clan warehouse"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(57),
        200,
        "200 withdrawn to leader"
    );
}
