//! Port of `model/BlockList` + `clientpackets/RequestBlock` — the per-player
//! ignore list, and the `isBlocked` filter every broadcast channel applies.
//!
//! **`isBlocked` is two things ORed together**, and reading either alone is a
//! bug:
//!
//! ```text
//! isBlocked(owner, target) = owner.isBlockAll() || owner.isInBlockList(target)
//! ```
//!
//! `isBlockAll()` is `Player.getMessageRefusal()` — message-refusal / silence
//! mode, which this port already had as [`AdminFlags::silence`] and which the
//! whisper handler was already honouring. `isInBlockList` is the persisted list
//! this module adds ([`World::block_lists`]). A player in refusal mode has an
//! *empty* block list and still blocks everyone, which is exactly why
//! [`is_blocked`] exists rather than callers reading either source directly.
//!
//! **The list is the receiver's, never the speaker's.** Every call site passes
//! the person who would *hear* the line as `owner`. The single exception is
//! `ChatShout`'s in-region branch, which checks both directions — see
//! `game_loop::chat`.
//!
//! Blocking is **not** mutual: one row, one direction. Java's
//! `character_friends` stores friends at `relation = 0` and blocks at
//! `relation = 1`.

use super::helpers::send_sm_bare_to_client as send_sm;
use crate::game_loop::helpers::is_gm;
use crate::game_loop::helpers::send_to_client;
use crate::model::components::AdminFlags;
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

/// `RequestBlock`'s five sub-commands (the leading int of the packet).
const BLOCK: i32 = 0;
const UNBLOCK: i32 = 1;
const BLOCKLIST: i32 = 2;
const ALLBLOCK: i32 = 3;
const ALLUNBLOCK: i32 = 4;

/// Java `BlockList.isBlocked(listOwner, target)` — whether `owner` refuses to
/// hear from `target`.
///
/// Both halves, in Java's order: message-refusal mode first (it blocks
/// everyone regardless of the list), then the list itself.
pub(crate) fn is_blocked(world: &World, owner_oid: i32, target_oid: i32) -> bool {
    let block_all = world
        .objects
        .get_component::<AdminFlags>(&owner_oid)
        .is_some_and(|f| f.silence);
    block_all || is_in_block_list(world, owner_oid, target_oid)
}

/// The persisted half — Java `BlockList.isInBlockList`. Works for an offline
/// owner, which `RequestSendPost` depends on.
pub(crate) fn is_in_block_list(world: &World, owner_oid: i32, target_oid: i32) -> bool {
    world
        .block_lists
        .get(&owner_oid)
        .is_some_and(|set| set.contains(&target_oid))
}

/// The inverse, named after Java's `Player.isNotBlocked` so broadcast loops
/// read the way the reference does.
pub(crate) fn is_not_blocked(world: &World, owner_oid: i32, target_oid: i32) -> bool {
    !is_blocked(world, owner_oid, target_oid)
}

/// Port of `clientpackets/RequestBlock` (0xA9).
pub(crate) fn handle_request_block(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player_oid) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let Some(kind) = r.read_i32() else {
        return;
    };

    match kind {
        BLOCK | UNBLOCK => {
            // Only these two carry a name.
            let Some(name) = r.read_string() else {
                return;
            };
            if kind == BLOCK {
                add_to_block_list(world, client_id, player_oid, &name);
            } else {
                remove_from_block_list(world, client_id, player_oid, &name);
            }
        }
        BLOCKLIST => send_list_to_owner(world, client_id, player_oid),
        ALLBLOCK => set_block_all(world, client_id, player_oid, true),
        ALLUNBLOCK => set_block_all(world, client_id, player_oid, false),
        other => {
            tracing::info!("Unknown 0xA9 block type: {other}");
        }
    }
}

/// Java `RequestBlock`'s BLOCK arm + `BlockList.addToBlockList`, in Java's
/// refusal order: unknown name, then GM, then self, then already-a-friend,
/// then already-blocked.
fn add_to_block_list(world: &mut World, client_id: u32, owner_oid: i32, name: &str) {
    let Some(target_oid) = resolve(world, name) else {
        // Java: "can't use block/unblock for locating invisible characters" —
        // an unknown name and a hidden one answer identically on purpose.
        send_sm(
            world,
            client_id,
            sm_ids::YOU_HAVE_FAILED_TO_REGISTER_THE_USER_TO_YOUR_IGNORE_LIST,
        );
        return;
    };
    if is_gm(world, target_oid) {
        send_sm(world, client_id, sm_ids::YOU_MAY_NOT_IMPOSE_A_BLOCK_ON_A_GM);
        return;
    }
    // Java returns silently here — no message for blocking yourself.
    if target_oid == owner_oid {
        return;
    }
    if is_friend(world, owner_oid, target_oid) {
        send_sm(
            world,
            client_id,
            sm_ids::THIS_PLAYER_IS_ALREADY_REGISTERED_ON_YOUR_FRIENDS_LIST,
        );
        return;
    }
    let already = is_in_block_list(world, owner_oid, target_oid);
    if already {
        // Java sends this as a literal `sendMessage`, not a SystemMessage.
        super::admin::send_message(world, client_id, "Already in ignore list.");
        return;
    }

    world
        .block_lists
        .entry(owner_oid)
        .or_default()
        .insert(target_oid);
    let _ = world.db.send(crate::db::DbCommand::InsertBlock {
        owner: owner_oid,
        target: target_oid,
    });

    let display = super::mail::char_name_by_id(world, target_oid);
    send_sm_str(
        world,
        client_id,
        sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_IGNORE_LIST,
        &display,
    );

    // The blocked player is *told*, if online — so blocking is not silent.
    let owner_name = super::mail::char_name_by_id(world, owner_oid);
    if let Some(cid) = client_of(world, target_oid) {
        send_sm_str(
            world,
            cid,
            sm_ids::C1_HAS_PLACED_YOU_ON_HIS_HER_IGNORE_LIST,
            &owner_name,
        );
    }
}

/// Java `BlockList.removeFromBlockList`.
fn remove_from_block_list(world: &mut World, client_id: u32, owner_oid: i32, name: &str) {
    let target_oid = resolve(world, name);
    let present = target_oid.is_some_and(|id| is_in_block_list(world, owner_oid, id));
    if !present {
        // Java checks list membership, not name validity, so an unknown name
        // and a known-but-unblocked one both answer SM 144.
        send_sm(world, client_id, sm_ids::THAT_IS_AN_INCORRECT_TARGET);
        return;
    }
    let target_oid = target_oid.expect("checked above");

    if let Some(set) = world.block_lists.get_mut(&owner_oid) {
        set.remove(&target_oid);
    }
    let _ = world.db.send(crate::db::DbCommand::DeleteBlock {
        owner: owner_oid,
        target: target_oid,
    });

    let display = super::mail::char_name_by_id(world, target_oid);
    send_sm_str(
        world,
        client_id,
        sm_ids::S1_HAS_BEEN_REMOVED_FROM_YOUR_IGNORE_LIST,
        &display,
    );
}

/// Java `BlockList.sendListToOwner`.
fn send_list_to_owner(world: &World, client_id: u32, owner_oid: i32) {
    let ids: Vec<i32> = world
        .block_lists
        .get(&owner_oid)
        .map(|set| {
            let mut v: Vec<i32> = set.iter().copied().collect();
            // A `HashSet` has no order; sort so the window is stable between
            // openings rather than reshuffling on each request.
            v.sort_unstable();
            v
        })
        .unwrap_or_default();
    let names: Vec<String> = ids
        .into_iter()
        .map(|id| super::mail::char_name_by_id(world, id))
        .collect();
    send_to_client(world, client_id, server_packets::block_list(&names));
}

/// Java `BlockList.setBlockAll` — message-refusal mode, the `isBlockAll()`
/// half of `isBlocked`. Shares its flag with the GM `//silence` toggle, as
/// Java shares `getMessageRefusal()`.
fn set_block_all(world: &mut World, client_id: u32, owner_oid: i32, on: bool) {
    send_sm(
        world,
        client_id,
        if on {
            sm_ids::MESSAGE_REFUSAL_MODE
        } else {
            sm_ids::MESSAGE_ACCEPTANCE_MODE
        },
    );
    let mut flags = world
        .objects
        .get_component::<AdminFlags>(&owner_oid)
        .copied()
        .unwrap_or_default();
    flags.silence = on;
    world.objects.add_components(&owner_oid, flags);
}

// ---------------------------------------------------------------------------

/// Java `CharInfoTable.getIdByName` — works for offline characters.
fn resolve(world: &World, name: &str) -> Option<i32> {
    super::mail::char_id_by_name(world, name)
}

fn is_friend(world: &World, owner_oid: i32, target_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Friends>(&owner_oid)
        .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == target_oid))
}

fn client_of(world: &World, object_id: i32) -> Option<u32> {
    world.clients.iter().find_map(|(&cid, cs)| match cs {
        crate::session::ClientSession::InGame(s) if s.player_object_id() == object_id => Some(cid),
        _ => None,
    })
}

fn send_sm_str(world: &World, client_id: u32, message_id: i16, text: &str) {
    send_to_client(
        world,
        client_id,
        server_packets::system_message_with(
            message_id,
            &[commons::system_messages::SmParam::Text(text.to_string())],
        ),
    );
}
