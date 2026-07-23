//! Port of `clientpackets/Say2` + the `handlers/chathandlers/*` scripts
//! (General/Shout/Trade/Whisper/Party/Clan/Alliance). Guards that need absent
//! systems (chat bans, jail, olympiad, block list, say filter, voiced
//! commands, item links) are skipped — see PLAN_G10_SOCIAL.md §2/§4.

use tracing::warn;

use crate::enums::ChatType;
use crate::model::components::{Position, RegionCell};
use crate::model::Player;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::{regions_adjacent, World};

/// Java `Say2`'s no-item-link cap (105 chars, "verified on official").
const MAX_CHAT_LENGTH: usize = 105;

/// `ChatGeneral`'s `forEachVisibleObjectInRange` radius.
const GENERAL_CHAT_RANGE: f64 = 1250.0;

pub(crate) fn handle_say2(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let sender_oid = session.player_object_id();
    let Some(pkt) = cp::Say2::read(body) else {
        return;
    };

    // Java disconnects on an unknown type or empty text ("possible packet
    // hack"); we log and drop instead (deliberate deviation — a malformed
    // packet shouldn't kill the session).
    let Some(chat_type) = ChatType::from_client_id(pkt.chat_type) else {
        warn!(
            "Say2: invalid chat type {} from object {sender_oid}.",
            pkt.chat_type
        );
        return;
    };
    if pkt.text.is_empty() {
        warn!("Say2: empty text from object {sender_oid}.");
        return;
    }
    if pkt.text.chars().count() > MAX_CHAT_LENGTH {
        send_sm(world, client_id, sm_ids::KEYBOARD_INPUT_SPAM_WARNING);
        return;
    }

    let Some(p) = world.objects.get_component::<Player>(&sender_oid) else {
        return;
    };
    let (sender_name, sender_level) = (p.name.clone(), p.level);

    match chat_type {
        ChatType::General => {
            // ChatGeneral: everyone within 1250 units + the speaker.
            let say =
                server_packets::creature_say(sender_oid, chat_type, &sender_name, &pkt.text, None);
            let Some(&from_pos) = world.objects.get_component::<Position>(&sender_oid) else {
                return;
            };
            let Some(&RegionCell(from_region)) =
                world.objects.get_component::<RegionCell>(&sender_oid)
            else {
                return;
            };
            for cs in world.clients.values() {
                let ClientSession::InGame(s) = cs else {
                    continue;
                };
                let other_oid = s.player_object_id();
                if other_oid == sender_oid {
                    cs.send(say.clone());
                    continue;
                }
                let Some(&RegionCell(other_region)) =
                    world.objects.get_component::<RegionCell>(&other_oid)
                else {
                    continue;
                };
                if !regions_adjacent(from_region, other_region) {
                    continue;
                }
                let Some(other_pos) = world.objects.get_component::<Position>(&other_oid) else {
                    continue;
                };
                if from_pos.distance_2d(other_pos) <= GENERAL_CHAT_RANGE {
                    cs.send(say.clone());
                }
            }
        }
        ChatType::Shout | ChatType::Trade => {
            // ChatShout/ChatTrade with `GlobalChat`/`TradeChat = ON` (this
            // dist): everyone whose position maps to the same map-region
            // *tile group* (`MapRegionManager.getMapRegionLocId`), speaker
            // included. Region identity = the region entry; two off-map
            // players share Java's `0` bucket (both `None` here).
            let say =
                server_packets::creature_say(sender_oid, chat_type, &sender_name, &pkt.text, None);
            let Some(from_pos) = world.objects.get_component::<Position>(&sender_oid) else {
                return;
            };
            let from_region = world
                .data
                .map_region
                .region_at(from_pos.x, from_pos.y)
                .map(|r| r.name.clone());
            for cs in world.clients.values() {
                let ClientSession::InGame(s) = cs else {
                    continue;
                };
                let other_oid = s.player_object_id();
                let Some(other_pos) = world.objects.get_component::<Position>(&other_oid) else {
                    continue;
                };
                let other_region = world
                    .data
                    .map_region
                    .region_at(other_pos.x, other_pos.y)
                    .map(|r| r.name.clone());
                if from_region == other_region {
                    cs.send(say.clone());
                }
            }
        }
        ChatType::Whisper => {
            let target_name = pkt.target.as_deref().unwrap_or_default();
            // World.getPlayer(name) — case-insensitive scan over in-game players.
            let receiver = world.clients.iter().find_map(|(&cid, cs)| match cs {
                ClientSession::InGame(s) => {
                    let oid = s.player_object_id();
                    world
                        .objects
                        .get_component::<Player>(&oid)
                        .filter(|p| p.name.eq_ignore_ascii_case(target_name))
                        .map(|p| (cid, oid, p.name.clone()))
                }
                _ => None,
            });
            let Some((receiver_cid, receiver_oid, receiver_name)) = receiver else {
                send_sm(world, client_id, sm_ids::THAT_PLAYER_IS_NOT_ONLINE);
                return;
            };
            // Java `ChatWhisper`: a receiver in silence/message-refusal mode
            // refuses the PM — the sender gets the refusal notice, nothing is
            // delivered.
            if world
                .objects
                .get_component::<crate::model::components::AdminFlags>(&receiver_oid)
                .is_some_and(|f| f.silence)
            {
                send_sm(
                    world,
                    client_id,
                    sm_ids::THAT_PERSON_IS_IN_MESSAGE_REFUSAL_MODE,
                );
                return;
            }
            // Relation mask: bit 0x01 = sender on the receiver's friend list
            // (wired with the friend system); other bits need clans/mentors.
            let mask = whisper_relation_mask(world, sender_oid, receiver_oid);
            if let Some(rcs) = world.clients.get(&receiver_cid) {
                rcs.send(server_packets::creature_say(
                    sender_oid,
                    chat_type,
                    &sender_name,
                    &pkt.text,
                    Some((mask, sender_level)),
                ));
            }
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::creature_say(
                    sender_oid,
                    chat_type,
                    &format!("->{receiver_name}"),
                    &pkt.text,
                    Some((mask, sender_level)),
                ));
            }
        }
        ChatType::Party => {
            // ChatParty — `party.broadcastCreatureSay` (speaker included).
            let say =
                server_packets::creature_say(sender_oid, chat_type, &sender_name, &pkt.text, None);
            if !super::party::party_say(world, sender_oid, &say) {
                send_sm(world, client_id, sm_ids::YOU_ARE_NOT_IN_A_PARTY);
            }
        }
        ChatType::Clan => {
            // ChatClan — `clan.broadcastToOnlineMembers` (speaker included).
            let clan_id = world
                .objects
                .get_component::<crate::model::Player>(&sender_oid)
                .map(|p| p.clan_id)
                .unwrap_or(0);
            if clan_id != 0 && world.clans.contains_key(&clan_id) {
                let say = server_packets::creature_say(
                    sender_oid,
                    chat_type,
                    &sender_name,
                    &pkt.text,
                    None,
                );
                super::clans::broadcast_to_clan(world, clan_id, &say);
            } else {
                send_sm(world, client_id, sm_ids::YOU_ARE_NOT_IN_A_CLAN);
            }
        }
        ChatType::Alliance => send_sm(world, client_id, sm_ids::YOU_ARE_NOT_IN_AN_ALLIANCE),
    }
}

/// The `CreatureSay` whisper-tail relation mask (receiver's view of the
/// sender). Only the friend bit (0x01) is representable so far — the
/// clan/mentor/ally bits need their systems.
fn whisper_relation_mask(world: &World, sender_oid: i32, receiver_oid: i32) -> u8 {
    let is_friend = world
        .objects
        .get_component::<crate::model::components::Friends>(&receiver_oid)
        .is_some_and(|fl| fl.0.iter().any(|f| f.char_id == sender_oid));
    if is_friend {
        0x01
    } else {
        0
    }
}

fn send_sm(world: &World, client_id: u32, message_id: i16) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, &[]));
    }
}
