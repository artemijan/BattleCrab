//! Port of `clientpackets/Say2` + the `handlers/chathandlers/*` scripts
//! (General/Shout/Trade/Whisper/Party/Clan/Alliance), plus the shift-click
//! item link pair `RequestExRqItemLink`/`ExRpItemLink`. Guards that need absent
//! systems (chat bans, jail, olympiad, block list, say filter, voiced
//! commands) are skipped — see PLAN_G10_SOCIAL.md §2/§4.

use tracing::warn;

use crate::enums::ChatType;
use crate::model::Player;
use crate::model::components::{Position, RegionCell};
use crate::model::inventory::{Inventory, ItemInstance};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::{World, regions_adjacent};

/// Java `Say2`'s no-item-link cap (105 chars, "verified on official").
const MAX_CHAT_LENGTH: usize = 105;

/// Java `Say2`'s cap for a line that carries at least one shift-clicked item
/// link: the link markup alone runs well past 105 chars, so the limit is
/// raised to 500 whenever the text contains [`ITEM_LINK_MARKER`].
const MAX_CHAT_LENGTH_WITH_ITEM_LINK: usize = 500;

/// The delimiter the client wraps a shift-clicked item link in — an ASCII
/// backspace (Java's bare `8` in `_text.indexOf(8)`). The markup between a
/// pair of them looks like
/// `\x08\tType=1\tID=268501230\tColor=0\tUnderline=0\tTitle=Squire's Sword\x08`.
const ITEM_LINK_MARKER: char = '\u{8}';

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
    // Java: "Allow higher limit if player shift some item (text is longer
    // then)" — a line carrying an item link gets 500 chars instead of 105, and
    // a GM is exempt from both caps. Without the item-link branch every
    // shift-clicked link past the 105th character is swallowed as spam.
    let has_item_link = pkt.text.contains(ITEM_LINK_MARKER);
    let limit = if has_item_link {
        MAX_CHAT_LENGTH_WITH_ITEM_LINK
    } else {
        MAX_CHAT_LENGTH
    };
    let is_gm = world
        .objects
        .get_component::<Player>(&sender_oid)
        .is_some_and(|p| p.is_gm(&world.data));
    if !is_gm && pkt.text.chars().count() > limit {
        send_sm(world, client_id, sm_ids::KEYBOARD_INPUT_SPAM_WARNING);
        return;
    }

    // `Config.L2WALKER_PROTECTION` — the L2Walker bot client drives the game by
    // *whispering* its own verbs, so a whisper opening with one is taken as
    // proof of an emulator. Java punishes with `DEFAULT_PUNISH` (`KICK` on this
    // dist) and drops the line.
    if world.cfg.custom_misc.walker_protection
        && chat_type == ChatType::Whisper
        && crate::config::custom_misc::WALKER_COMMAND_LIST
            .iter()
            .any(|c| pkt.text.starts_with(c))
    {
        tracing::warn!("Client Emulator Detect: object {sender_oid} using L2Walker; kicking.");
        super::admin::moderation::disconnect_player(world, sender_oid);
        return;
    }

    // Java `Say2`: the curse silences the two *broadcast* channels — a cursed
    // wielder can't hide behind Trade or Shout to bait victims from off-screen.
    // Ordinary/party/clan chat is untouched. Sits *after* the L2Walker check,
    // as in Java, where the emulator kick precedes the cursed-weapon guard.
    if matches!(chat_type, ChatType::Trade | ChatType::Shout)
        && world
            .objects
            .get_component::<crate::model::Player>(&sender_oid)
            .is_some_and(|p| p.cursed_weapon_equipped_id != 0)
    {
        send_sm(
            world,
            client_id,
            sm_ids::SHOUT_AND_TRADE_CHATTING_CANNOT_BE_USED_WHILE_POSSESSING_A_CURSED_WEAPON,
        );
        return;
    }

    // Java `Say2`: a chat-banned player can still use `.`-prefixed commands but
    // no ordinary chat gets through (G31). The prohibition message is sent, then
    // the message is dropped.
    if !pkt.text.starts_with('.') && super::punishment::is_chat_banned(world, sender_oid) {
        send_sm(world, client_id, sm_ids::CHATTING_IS_CURRENTLY_PROHIBITED);
        return;
    }

    // Petition consultation chat (Java `Say2` → `sendActivePetitionMessage`,
    // G31): route the line to both participants, don't broadcast it.
    if matches!(chat_type, ChatType::PetitionPlayer | ChatType::PetitionGm) {
        super::petition::send_active_petition_message(world, sender_oid, &pkt.text);
        return;
    }

    let Some(p) = world.objects.get_component::<Player>(&sender_oid) else {
        return;
    };
    let (sender_name, sender_level) = (p.name.clone(), p.level);

    // Java `Player.broadcastSnoop`: mirror the line to any GM snooping this
    // speaker (G31). Fires for every chat channel the player originates.
    broadcast_snoop(world, sender_oid, chat_type, &sender_name, &pkt.text);

    // `ChatGeneral`: a `.`-prefixed line is a voiced command, not chat — the
    // handler runs and the text is never broadcast. Java's `MasterHandler`
    // registers each handler only when its `Custom/*.ini` flag is on, so an
    // unregistered command falls through to being *said*; the gates below keep
    // that shape. Still unported: `.premium` and `.password` (`TODO(G33)`) —
    // auto-play and auto-potions landed with the `Custom/*.ini` audit and are
    // dispatched below.
    if chat_type == ChatType::General
        && let Some(rest) = pkt.text.strip_prefix('.')
    {
        let command = rest.split_whitespace().next().unwrap_or("");
        match command {
            "offline" => {
                super::offline_trade::handle_voiced_offline(world, client_id);
                return;
            }
            "online" if world.cfg.custom_misc.online_command => {
                handle_voiced_online(world, client_id);
                return;
            }
            "bank" | "deposit" | "withdraw" if world.cfg.banking.enabled => {
                handle_voiced_banking(world, client_id, sender_oid, command);
                return;
            }
            // `handlers/voicedcommandhandlers/ChatAdmin` — the `.`-prefixed
            // twins of `//chatban`/`//unban_chat`. Java gates them on the very
            // same access-table entry as the `//` form, so a player typing them
            // gets nothing (the handler returns false and the line is dropped).
            "play" | "playskills" | "playitems" | "playpotion" if world.cfg.auto_play.enabled => {
                let args: Vec<&str> = rest.split_whitespace().skip(1).collect();
                super::auto_play::handle_voiced(world, client_id, sender_oid, command, &args);
                return;
            }
            "apon" | "apoff" | "potionon" | "potionoff" if world.cfg.auto_potions.enabled => {
                super::auto_potions::handle_voiced(world, client_id, sender_oid, command);
                return;
            }
            "sellbuff" | "sellbuffs" if world.cfg.sell_buffs.enabled => {
                super::sell_buffs::handle_voiced_sellbuff(world, client_id, sender_oid);
                return;
            }
            "banchat" | "chatban" | "unbanchat" | "chatunban"
                if world.cfg.custom_npc.chat_admin =>
            {
                handle_voiced_chat_admin(world, client_id, sender_oid, command, rest);
                return;
            }
            _ => {}
        }
    }

    // Java `Say2`: `if ((_text.indexOf(8) >= 0) && !parseAndPublishItem(...))
    // return;` — every item the speaker shift-clicked into the line is
    // *published*, which is the sole gate `RequestExRqItemLink` checks before
    // answering another player's click on the link. A line that links an item
    // the speaker does not own is dropped whole.
    if has_item_link && !parse_and_publish_item(world, sender_oid, &pkt.text) {
        return;
    }

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
        ChatType::PartyroomCommander | ChatType::PartyroomAll => {
            // ChatPartyRoomCommander / ChatPartyRoomAll: command channel chat.
            // Commander (15) — only the CC leader may speak; All (16) — any
            // party leader in the channel. Silently ignored otherwise (Java).
            // Every CC member receives it (the block-list skip is the same
            // absent-system gap as the other channels, see the module header).
            let Some(party_id) = super::command_channel::party_id_of(world, sender_oid) else {
                return;
            };
            let Some(cc_id) = super::command_channel::cc_id_of_party(world, party_id) else {
                return;
            };
            let may_speak = if chat_type == ChatType::PartyroomCommander {
                world
                    .command_channels
                    .get(&cc_id)
                    .is_some_and(|cc| cc.is_leader(sender_oid))
            } else {
                world
                    .parties
                    .get(&party_id)
                    .is_some_and(|p| p.is_leader(sender_oid))
            };
            if !may_speak {
                return;
            }
            let say =
                server_packets::creature_say(sender_oid, chat_type, &sender_name, &pkt.text, None);
            super::command_channel::broadcast_to_cc(world, cc_id, &say);
        }
        ChatType::PartyMatchRoom => {
            // ChatPartyMatchRoom — `CreatureSay` to every member of the room the
            // speaker is in (speaker included), G30.
            let say =
                server_packets::creature_say(sender_oid, chat_type, &sender_name, &pkt.text, None);
            if let Some(room_id) = world.matching_rooms.room_id_of(sender_oid) {
                let members = world
                    .matching_rooms
                    .get(room_id)
                    .map(|r| r.all_members())
                    .unwrap_or_default();
                for oid in members {
                    super::party_room::send_to(world, oid, say.clone());
                }
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
        // Server-sent only (ferry announcements / event announcements) or handled
        // before the match (petition consultation chat); a client never
        // legitimately originates these here, so drop them.
        ChatType::Boat
        | ChatType::Announcement
        | ChatType::PetitionPlayer
        | ChatType::PetitionGm
        | ChatType::HeroVoice => {}
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
    if is_friend { 0x01 } else { 0 }
}

/// Java `Player.broadcastSnoop`: send a `Snoop` line to every GM currently
/// eavesdropping on `speaker` (`//snoop`). Offline listeners are skipped.
fn broadcast_snoop(
    world: &World,
    speaker: i32,
    chat_type: ChatType,
    speaker_name: &str,
    text: &str,
) {
    let listeners = match world.objects.get_component::<Player>(&speaker) {
        Some(p) if !p.snoop_listeners.is_empty() => p.snoop_listeners.clone(),
        _ => return,
    };
    let snoop = server_packets::snoop(speaker, speaker_name, chat_type, speaker_name, text);
    for gm in listeners {
        if let Some(cs) =
            super::helpers::client_for_player(world, gm).and_then(|c| world.clients.get(&c))
        {
            cs.send(snoop.clone());
        }
    }
}

fn send_sm(world: &World, client_id: u32, message_id: i16) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, &[]));
    }
}

// ---------------------------------------------------------------------------
// Shift-click item links (`Say2.parseAndPublishItem` + `RequestExRqItemLink`)
// ---------------------------------------------------------------------------

/// Java `Say2.parseAndPublishItem`. Walk every `\x08 … ID=<objectId> … \x08`
/// span in the line, check the speaker really owns that inventory item, and
/// *publish* it — Java sets `Item._published`, the one flag
/// `RequestExRqItemLink` consults before answering a reader's click. Returns
/// false exactly where Java does (an id the speaker does not own, an
/// unparseable id, a link with no closing marker); the caller then drops the
/// whole line, as Java does.
fn parse_and_publish_item(world: &mut World, sender_oid: i32, text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let Some(owned) = world
        .objects
        .get_component::<Inventory>(&sender_oid)
        .map(|inv| inv.items().iter().map(|i| i.object_id).collect::<Vec<_>>())
    else {
        return false;
    };

    let mut publish = Vec::new();
    let mut cursor = 0;
    while let Some(open) = find_char(&chars, ITEM_LINK_MARKER, cursor) {
        // Java `_text.indexOf("ID=", pos1)`: the object id is the only field
        // the server reads out of the markup — everything else (Type, Color,
        // Underline, Title) is client-side decoration.
        let Some(id_at) = find_str(&chars, "ID=", open) else {
            return false;
        };
        let digits_at = id_at + 3;
        let mut pos = digits_at;
        while pos < chars.len() && chars[pos].is_ascii_digit() {
            pos += 1;
        }
        let Ok(object_id) = chars[digits_at..pos]
            .iter()
            .collect::<String>()
            .parse::<i32>()
        else {
            return false;
        };
        if !owned.contains(&object_id) {
            warn!("Say2: object {sender_oid} tried to publish item {object_id} it does not own.");
            return false;
        }
        publish.push(object_id);

        // Java `pos1 = _text.indexOf(8, pos) + 1` with `pos1 == 0` — i.e. no
        // closing marker — logged as an invalid publish message and dropped.
        let Some(close) = find_char(&chars, ITEM_LINK_MARKER, pos) else {
            warn!("Say2: object {sender_oid} sent an item link with no closing marker.");
            return false;
        };
        cursor = close + 1;
    }

    for object_id in publish {
        world.published_items.insert(object_id, sender_oid);
    }
    true
}

/// `chars[from..].position(needle)`, in `char` units — the port of Java's
/// `String.indexOf(int, int)` on a UTF-16 string. Working in chars rather than
/// bytes keeps the offsets valid for non-ASCII item names in the link title.
fn find_char(chars: &[char], needle: char, from: usize) -> Option<usize> {
    chars[from.min(chars.len())..]
        .iter()
        .position(|&c| c == needle)
        .map(|i| i + from)
}

/// Java's `String.indexOf(String, int)`, in `char` units.
fn find_str(chars: &[char], needle: &str, from: usize) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() || chars.len() < needle.len() {
        return None;
    }
    (from..=chars.len() - needle.len()).find(|&i| chars[i..i + needle.len()] == needle[..])
}

/// Java `RequestExRqItemLink` (ex `0x1E`): a reader clicked the link in the
/// chat line, so the client asks for the item behind that object id. Java
/// answers `ExRpItemLink` — the same item row `ItemList` writes — for any
/// *published* item; with no answer the client has nothing to show and the
/// link stays a bare "?".
pub(crate) fn handle_request_item_link(world: &World, client_id: u32, body: &[u8]) {
    if !matches!(
        world.clients.get(&client_id),
        Some(ClientSession::InGame(_))
    ) {
        return;
    }
    let Some(object_id) = commons::network::PacketReader::new(body).read_i32() else {
        return;
    };
    // Java's `item.isPublished()` gate: an object id that was never shift-
    // clicked into a chat line is not answered, so a client cannot fish for
    // the contents of someone's inventory by guessing ids.
    if !world.published_items.contains_key(&object_id) {
        return;
    }
    let Some((item, equipped)) = find_published_item(world, object_id) else {
        return;
    };
    let Some(template) = world.data.item_data.get(item.item_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::ex_rp_item_link(
            &item, template, equipped,
        ));
    }
}

/// Java `World.findObject(objectId)` narrowed to items: the published item is
/// looked up live, so the reader sees its *current* state (enchant, count) and
/// the link keeps working after a trade. Only inventories of loaded players
/// are searched — an item whose owner logged off is gone from the world in
/// Java too, and its `_published` flag died with the `Item` instance.
fn find_published_item(world: &World, object_id: i32) -> Option<(ItemInstance, bool)> {
    let owners = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .chain(world.offline_traders.keys().copied());
    for owner in owners {
        let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
            continue;
        };
        if let Some(item) = inv.items().iter().find(|i| i.object_id == object_id) {
            let equipped = inv.paperdoll_slot_of(object_id).is_some();
            return Some((*item, equipped));
        }
    }
    None
}

/// The publish flags of a leaving player's items die with them, as Java's
/// per-`Item` `_published` does when the instance is dropped at logout.
pub(crate) fn on_player_leave_world(world: &mut World, player_object_id: i32) {
    world
        .published_items
        .retain(|_, publisher| *publisher != player_object_id);
}

// ---------------------------------------------------------------------------
// `Custom/*.ini` voiced commands (G33 audit)
// ---------------------------------------------------------------------------

/// Java's `Goldbar` item — what `.deposit` pays out and `.withdraw` consumes.
const GOLDBAR_ITEM_ID: i32 = 3470;
/// Adena.
const ADENA_ITEM_ID: i32 = 57;

/// `handlers/voicedcommandhandlers/Online` — `.online`, the player count.
/// Java's singular/plural split is verbatim; the count is every in-game
/// session, which includes an offline shop still standing (`World.getPlayers`
/// keeps them, and so does the port's `clients` map plus `offline_traders`).
fn handle_voiced_online(world: &World, client_id: u32) {
    let count = world
        .clients
        .values()
        .filter(|cs| matches!(cs, ClientSession::InGame(_)))
        .count()
        + world.offline_traders.len();
    let text = if count > 1 {
        format!("There are {count} players online!")
    } else {
        "There is 1 player online!".to_string()
    };
    crate::game_loop::admin::send_message(world, client_id, &text);
}

/// `handlers/voicedcommandhandlers/Banking` — `.bank` explains the rate,
/// `.deposit` turns `BankingAdenaCount` adena into `BankingGoldbarCount`
/// goldbars, `.withdraw` reverses it. Java messages the shortfall rather than
/// refusing silently, and its `updateDatabase()` call is a no-op here: the port
/// is memory-first and the inventory flushes on the next autosave.
fn handle_voiced_banking(world: &mut World, client_id: u32, player_oid: i32, command: &str) {
    let (adena, goldbars) = (world.cfg.banking.adena, world.cfg.banking.goldbars);
    let count_of = |world: &World, item_id: i32| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&player_oid)
            .map_or(0, |inv| inv.count_of(item_id))
    };
    match command {
        "bank" => {
            let text = format!(
                ".deposit ({adena} Adena = {goldbars} Goldbar) / \
                 .withdraw ({goldbars} Goldbar = {adena} Adena)"
            );
            crate::game_loop::admin::send_message(world, client_id, &text);
        }
        "deposit" => {
            if count_of(world, ADENA_ITEM_ID) < adena {
                let text = format!(
                    "You do not have enough Adena to convert to Goldbar(s), \
                     you need {adena} Adena."
                );
                crate::game_loop::admin::send_message(world, client_id, &text);
                return;
            }
            super::quests::take_items(world, client_id, player_oid, ADENA_ITEM_ID, adena);
            super::items::add_inventory_item(world, player_oid, GOLDBAR_ITEM_ID, goldbars);
            let text =
                format!("Thank you, you now have {goldbars} Goldbar(s), and {adena} less adena.");
            crate::game_loop::admin::send_message(world, client_id, &text);
        }
        "withdraw" => {
            if count_of(world, GOLDBAR_ITEM_ID) < goldbars {
                let text = format!("You do not have any Goldbars to turn into {adena} Adena.");
                crate::game_loop::admin::send_message(world, client_id, &text);
                return;
            }
            super::quests::take_items(world, client_id, player_oid, GOLDBAR_ITEM_ID, goldbars);
            super::items::add_inventory_item(world, player_oid, ADENA_ITEM_ID, adena);
            let text =
                format!("Thank you, you now have {adena} Adena, and {goldbars} less Goldbar(s).");
            crate::game_loop::admin::send_message(world, client_id, &text);
        }
        _ => {}
    }
}

/// `ChatAdmin.useVoicedCommand` — `.banchat` / `.unbanchat` and their aliases,
/// routed into the same punishment code the `//` commands use. Java checks
/// `AdminData.hasAccess(command, …)` first and simply returns on failure, so an
/// ordinary player's `.banchat` does nothing at all — no message, no chat.
fn handle_voiced_chat_admin(
    world: &mut World,
    client_id: u32,
    sender_oid: i32,
    command: &str,
    rest: &str,
) {
    let access_level = world
        .objects
        .get_component::<Player>(&sender_oid)
        .map_or(0, |p| p.access_level);
    // The access table keys the `//` names; `.banchat` maps onto `admin_banchat`
    // exactly as Java's shared `VOICED_COMMANDS` list does.
    if !world
        .data
        .admin
        .has_access(&format!("admin_{command}"), access_level)
    {
        return;
    }
    let args: Vec<&str> = rest.split_whitespace().skip(1).collect();
    match command {
        "banchat" | "chatban" => {
            super::admin::moderation::admin_ban_chat(world, client_id, sender_oid, &args)
        }
        _ => super::admin::moderation::admin_unban_chat(world, client_id, &args),
    }
}
