use super::*;
use crate::game_loop::helpers::send_to_client;

use crate::model::clan::{
    CL_REGISTER_CREST, CREST_TYPE_ALLY, CREST_TYPE_PLEDGE, CREST_TYPE_PLEDGE_LARGE, Crest,
};

/// `CrestTable.createCrest`: allocate the next id, store the bitmap, persist.
fn create_crest(world: &mut World, data: &[u8], kind: i32) -> i32 {
    let id = world.next_crest_id;
    world.next_crest_id += 1;
    world.crests.insert(
        id,
        Crest {
            id,
            data: data.to_vec(),
            kind,
        },
    );
    let _ = world.db.send(DbCommand::InsertCrest {
        id,
        data: data.to_vec(),
        kind,
    });
    id
}

/// `CrestTable.removeCrest`: drop the bitmap, but never delete (or let a
/// caller reuse) the most recently allocated id — Java's guard against a
/// stale client cache showing the wrong image for a brand-new crest.
fn remove_crest(world: &mut World, crest_id: i32) {
    world.crests.remove(&crest_id);
    if crest_id == world.next_crest_id - 1 {
        return;
    }
    let _ = world.db.send(DbCommand::DeleteCrest { id: crest_id });
}

/// Sync every online member's denormalized `Player.clan_crest_id` /
/// `clan_crest_large_id` with the clan and re-broadcast their
/// UserInfo/CharInfo — `Clan.changeClanCrest`'s `for (member : getOnlineMembers()) broadcastUserInfo()`.
#[cfg(test)]
pub(crate) fn refresh_clan_crest_on_members_for_test(world: &mut World, clan_id: i32) {
    refresh_clan_crest_on_members(world, clan_id);
}

fn refresh_clan_crest_on_members(world: &mut World, clan_id: i32) {
    let (crest_id, crest_large_id) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.crest_id, c.crest_large_id))
        .unwrap_or((0, 0));
    for oid in online_members(world, clan_id) {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_crest_id = crest_id;
            p.clan_crest_large_id = crest_large_id;
        }
        crate::game_loop::party::broadcast_user_info(world, oid);
    }
}

fn read_crest_data(r: &mut PacketReader, length: i32) -> Option<Vec<u8>> {
    if length > 0 {
        r.read_bytes(length as usize).map(|d| d.to_vec())
    } else {
        Some(Vec::new())
    }
}

/// `RequestSetPledgeCrest` (0x09): the small (≤256-byte) clan crest.
pub(crate) fn handle_request_set_pledge_crest(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let Some(length) = r.read_i32() else { return };
    if length > 256 {
        return; // Java's own readImpl bails before the length even reaches runImpl
    }
    let Some(data) = read_crest_data(&mut r, length) else {
        return;
    };

    let Some((clan_id, privs, _)) = clan_membership(world, player) else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.dissolving_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_CANNOT_REGISTER_OR_DELETE_CREST,
            &[],
        );
        return;
    }
    if !clan.has_privilege(player, privs, CL_REGISTER_CREST) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if data.is_empty() {
        if clan.crest_id != 0 {
            let old = clan.crest_id;
            remove_crest(world, old);
            if let Some(c) = world.clans.get_mut(&clan_id) {
                c.crest_id = 0;
            }
            let _ = world.db.send(DbCommand::UpdateClanCrest {
                clan_id,
                crest_id: 0,
            });
            refresh_clan_crest_on_members(world, clan_id);
            send_sm_with(world, player, sm_ids::THE_CLAN_MARK_HAS_BEEN_DELETED, &[]);
        }
        return;
    }
    if clan.level < 3 {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_CREST_CAN_ONLY_BE_REGISTERED_WHEN_THE_CLAN_S_SKILL_LEVEL_IS_3_OR_ABOVE,
            &[],
        );
        return;
    }
    let crest_id = create_crest(world, &data, CREST_TYPE_PLEDGE);
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.crest_id = crest_id;
    }
    let _ = world
        .db
        .send(DbCommand::UpdateClanCrest { clan_id, crest_id });
    refresh_clan_crest_on_members(world, clan_id);
    send_sm_with(
        world,
        player,
        sm_ids::THE_CREST_WAS_SUCCESSFULLY_REGISTERED,
        &[],
    );
}

/// `RequestPledgeCrest` (0x67): answer with the small crest's bitmap.
pub(crate) fn handle_request_pledge_crest(world: &World, client_id: u32, body: &[u8]) {
    let mut r = PacketReader::new(body);
    let Some(crest_id) = r.read_i32() else { return };
    let data = world.crests.get(&crest_id).map(|c| c.data.as_slice());
    send_to_client(
        world,
        client_id,
        server_packets::pledge_crest(crest_id, data),
    );
}

/// `RequestExSetPledgeCrestLarge` (ex 0x11): the large (≤2176-byte) crest,
/// shown on clan-hall/castle items.
pub(crate) fn handle_request_ex_set_pledge_crest_large(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(ex_body);
    let Some(length) = r.read_i32() else { return };
    if length > 2176 {
        return;
    }
    let Some(data) = read_crest_data(&mut r, length) else {
        return;
    };

    let Some((clan_id, privs, _)) = clan_membership(world, player) else {
        return;
    };
    if !(0..=2176).contains(&length) {
        send_sm_with(
            world,
            player,
            sm_ids::THE_SIZE_OF_THE_UPLOADED_SYMBOL_DOES_NOT_MEET_STANDARDS,
            &[],
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.dissolving_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_CANNOT_REGISTER_OR_DELETE_CREST,
            &[],
        );
        return;
    }
    if !clan.has_privilege(player, privs, CL_REGISTER_CREST) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if data.is_empty() {
        if clan.crest_large_id != 0 {
            let old = clan.crest_large_id;
            remove_crest(world, old);
            if let Some(c) = world.clans.get_mut(&clan_id) {
                c.crest_large_id = 0;
            }
            let _ = world.db.send(DbCommand::UpdateClanCrestLarge {
                clan_id,
                crest_large_id: 0,
            });
            // Java broadcasts UserInfo to every online member here too, even
            // though the large crest id isn't part of that packet (only
            // fetched on demand via `RequestExPledgeCrestLarge`) — kept
            // faithful; it's a no-op refresh for everyone but the actor.
            for oid in online_members(world, clan_id) {
                crate::game_loop::party::broadcast_user_info(world, oid);
            }
            send_sm_with(world, player, sm_ids::THE_CLAN_MARK_HAS_BEEN_DELETED, &[]);
        }
        return;
    }
    if clan.level < 3 {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_CREST_CAN_ONLY_BE_REGISTERED_WHEN_THE_CLAN_S_SKILL_LEVEL_IS_3_OR_ABOVE,
            &[],
        );
        return;
    }
    let crest_id = create_crest(world, &data, CREST_TYPE_PLEDGE_LARGE);
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.crest_large_id = crest_id;
    }
    let _ = world.db.send(DbCommand::UpdateClanCrestLarge {
        clan_id,
        crest_large_id: crest_id,
    });
    for oid in online_members(world, clan_id) {
        crate::game_loop::party::broadcast_user_info(world, oid);
    }
    send_sm_with(
        world,
        player,
        sm_ids::THE_CLAN_MARK_WAS_SUCCESSFULLY_REGISTERED_ON_ITEMS,
        &[],
    );
}

/// `RequestExPledgeCrestLarge` (ex 0x10): answer with the large crest's
/// bitmap, chunked into ≤14336-byte `ExPledgeEmblem` packets (always a single
/// chunk on this dist's 2176-byte cap, but the loop stays general).
pub(crate) fn handle_request_ex_pledge_crest_large(world: &World, client_id: u32, ex_body: &[u8]) {
    let mut r = PacketReader::new(ex_body);
    let Some(crest_id) = r.read_i32() else { return };
    let Some(clan_id) = r.read_i32() else { return };
    let Some(data) = world.crests.get(&crest_id).map(|c| c.data.clone()) else {
        return;
    };
    const CHUNK: usize = 14_336;
    for i in 0..5 {
        let start = CHUNK * i;
        if start >= data.len() {
            continue;
        }
        let end = (start + CHUNK).min(data.len());
        send_to_client(
            world,
            client_id,
            server_packets::ex_pledge_emblem(clan_id, crest_id, i as i32, &data[start..end]),
        );
    }
}

/// `RequestSetAllyCrest` (0x91): the alliance crest (≤192 bytes) — only the
/// alliance leader (the leader-clan's own clan leader) may set it.
pub(crate) fn handle_request_set_ally_crest(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let Some(length) = r.read_i32() else { return };
    if length > 192 {
        return;
    }
    let Some(data) = read_crest_data(&mut r, length) else {
        return;
    };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let ally_id = p.ally_id;
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    if length < 0 {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text("File transfer error.".to_string())],
        );
        return;
    }
    if length > 192 {
        send_sm_with(
            world,
            player,
            sm_ids::PLEASE_ADJUST_THE_IMAGE_SIZE_TO_8X12,
            &[],
        );
        return;
    }
    if ally_id == 0 || clan_id != ally_id || !is_leader {
        send_sm_with(
            world,
            player,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return;
    }
    if data.is_empty() {
        let old = world
            .clans
            .get(&clan_id)
            .map(|c| c.ally_crest_id)
            .unwrap_or(0);
        if old != 0 {
            remove_crest(world, old);
            set_alliance_crest(world, ally_id, 0);
        }
        return;
    }
    let crest_id = create_crest(world, &data, CREST_TYPE_ALLY);
    set_alliance_crest(world, ally_id, crest_id);
    send_sm_with(
        world,
        player,
        sm_ids::THE_CREST_WAS_SUCCESSFULLY_REGISTERED,
        &[],
    );
}

/// `Clan.changeAllyCrest(id, onlyThisClan=false)`: push the crest id to every
/// clan in the alliance and refresh their online members.
fn set_alliance_crest(world: &mut World, ally_id: i32, crest_id: i32) {
    let clan_ids = ally_clan_ids(world, ally_id);
    for cid in &clan_ids {
        if let Some(c) = world.clans.get_mut(cid) {
            c.ally_crest_id = crest_id;
        }
    }
    let _ = world.db.send(DbCommand::UpdateAllyCrestForAlliance {
        ally_id,
        ally_crest_id: crest_id,
    });
    for cid in clan_ids {
        for oid in online_members(world, cid) {
            if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
                p.ally_crest_id = crest_id;
            }
            crate::game_loop::party::broadcast_user_info(world, oid);
        }
    }
}

/// `RequestAllyCrest` (0x92): answer with the alliance crest's bitmap.
pub(crate) fn handle_request_ally_crest(world: &World, client_id: u32, body: &[u8]) {
    let mut r = PacketReader::new(body);
    let Some(crest_id) = r.read_i32() else { return };
    let data = world.crests.get(&crest_id).map(|c| c.data.as_slice());
    send_to_client(world, client_id, server_packets::ally_crest(crest_id, data));
}

// --- G18 slice 8: recruitment registry (ClanEntryManager) ------------------
