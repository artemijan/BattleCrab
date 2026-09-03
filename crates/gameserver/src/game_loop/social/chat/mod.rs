//! Port of `clientpackets/Say2` + the `handlers/chathandlers/*` scripts
//! (General/Shout/Trade/Whisper/Party/Clan/Alliance/World/HeroVoice), plus the
//! shift-click item link pair `RequestExRqItemLink`/`ExRpItemLink`.
//!
//! The `handlers/chathandlers/*` scripts live in the **datapack**
//! (`dist/game/data/scripts/`), not under `java/`, and `Say2` reaches them
//! through `ChatHandler` rather than a switch — so a `java/`-only grep for a
//! channel name finds nothing and reads the channel as unimplemented. That is
//! how world chat sat unported behind an enabled config for so long.
//!
//! Every guard `Say2` and the handlers apply is now ported: the say filter and
//! voiced commands landed with G33's `Custom/*.ini` audit, chat bans with G31,
//! and the jail gate, the olympiad gate and the block-list filter with the
//! 2026-08-07 sweep. See PLAN_G10_SOCIAL.md §2/§4 for the original scope, and
//! `game_loop::block_list` for why `isBlocked` must never be read in halves.

use crate::enums::ChatType;
use crate::game_loop::character::inventory;
use crate::game_loop::clans::clan_of_or_zero;
use crate::game_loop::{helpers, items};
use crate::model::Player;
use crate::model::components::{Position, RegionCell};
use crate::model::inventory::{Inventory, ItemInstance};
use crate::model::skill::effect_flag;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids};
use crate::session::ClientSession;
use crate::world::{World, regions_adjacent};
use commons::audit;
use serde_json::json;
use tracing::warn;

pub mod block_list;

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
    let Some(sender_oid) = world.player_oid(client_id) else {
        return;
    };
    let Some(mut pkt) = cp::Say2::read(body) else {
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
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::KEYBOARD_INPUT_SPAM_WARNING);
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
        super::super::moderation::punishment::illegal_action(
            world,
            sender_oid,
            &format!("Client Emulator Detect: player {sender_oid} using L2Walker."),
        );
        return;
    }

    // Java `Say2`: the curse silences the two *broadcast* channels — a cursed
    // wielder can't hide behind Trade or Shout to bait victims from off-screen.
    // Ordinary/party/clan chat is untouched. Sits *after* the L2Walker check,
    // as in Java, where the emulator kick precedes the cursed-weapon guard.
    if matches!(chat_type, ChatType::Trade | ChatType::Shout)
        && world
            .objects
            .get_component::<Player>(&sender_oid)
            .is_some_and(|p| p.cursed_weapon_equipped_id != 0)
    {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::SHOUT_AND_TRADE_CHATTING_CANNOT_BE_USED_WHILE_POSSESSING_A_CURSED_WEAPON,
        );
        return;
    }

    // Java `Say2`: a chat-banned player can still use `.`-prefixed commands but
    // no ordinary chat gets through (G31). The ban covers **every** channel —
    // Java's `return` is unconditional — and only the *message* varies:
    //   * under a `BlockChat` debuff (the bot-report punishment) the player is
    //     told they were reported as an illegal-program user;
    //   * otherwise the prohibition notice, but only on a `BanChatChannels`
    //     channel. On any other channel the line vanishes silently.
    if !pkt.text.starts_with('.')
        && super::super::moderation::punishment::is_chat_banned(world, sender_oid)
    {
        if super::super::abnormal::flags_of(world, sender_oid) & effect_flag::CHAT_BLOCK != 0 {
            helpers::send_sm_bare_to_client(
                world,
                client_id,
                sm_ids::YOU_HAVE_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_SO_CHATTING_IS_NOT_ALLOWED,
            );
        } else if world.cfg.chat_filter.ban_chat_channels.contains(&chat_type) {
            helpers::send_sm_bare_to_client(
                world,
                client_id,
                sm_ids::CHATTING_IS_CURRENTLY_PROHIBITED,
            );
        }
        return;
    }

    // Java `Say2`: no chat at all while in a bout or sitting in the queue.
    // Covers **every** channel, party and clan included, and has no GM escape
    // hatch — a GM who registers is silenced like anyone else.
    if super::super::olympiad::in_match(world, sender_oid)
        || world.olympiad.is_registered(sender_oid)
    {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_CANNOT_CHAT_WHILE_PARTICIPATING_IN_THE_OLYMPIAD,
        );
        return;
    }

    // `Config.MINIMUM_CHAT_LEVEL` (**0** here, so inert). Three of Java's chat
    // handlers open with the same test and **each sends a different system
    // message** naming its own channel — `ChatGeneral`, `ChatShout` and
    // `ChatWhisper`. Trade has no such gate; it has a hard-coded level 20 of
    // its own, below.
    //
    // A `CHAT_CONDITIONS` holder is exempt, which is the same override the
    // scope branches read.
    if let Some(sm) = match chat_type {
        ChatType::General => Some(sm_ids::GENERAL_CHAT_CANNOT_BE_USED_BELOW_LEVEL_S1),
        ChatType::Shout => Some(sm_ids::SHOUT_CHAT_CANNOT_BE_USED_BELOW_LEVEL_S1),
        ChatType::Whisper => Some(sm_ids::WHISPER_CANNOT_BE_INITIATED_BELOW_LEVEL_S1),
        _ => None,
    } && let Some(p) = world.objects.get_component::<Player>(&sender_oid)
        && p.level < world.cfg.general.minimum_chat_level
        && !p.can_override_cond(CHAT_CONDITIONS_ORDINAL)
    {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm,
            &[server_packets::SmParam::Int(
                world.cfg.general.minimum_chat_level,
            )],
        );
        return;
    }

    // Java `Say2`'s jail gate, and note how it differs from the *other*
    // `JailDisableChat` check in `world_chat` below — the two are not copies:
    //
    //   * this one covers **four channels only** (whisper, shout, trade, hero
    //     voice). WORLD is absent, which is exactly why `ChatWorld` carries its
    //     own check; ordinary, party and clan chat stay open to a prisoner on
    //     both paths.
    //   * it has **no `PlayerCondOverride.CHAT_CONDITIONS` escape**, where
    //     `ChatWorld`'s does — so a GM with the override may use world chat
    //     from jail but not a whisper.
    //   * it answers with `sendMessage(String)`, a literal line, not a
    //     `SystemMessage` — so the text is the server's rather than the
    //     client's, and is reproduced verbatim here.
    if world.cfg.general.jail_disable_chat
        && matches!(
            chat_type,
            ChatType::Whisper | ChatType::Shout | ChatType::Trade | ChatType::HeroVoice
        )
        && world
            .objects
            .get_component::<Player>(&sender_oid)
            .is_some_and(|p| p.jailed)
    {
        helpers::send_message(
            world,
            client_id,
            "You can not chat with players outside of the jail.",
        );
        return;
    }

    // Petition consultation chat (Java `Say2` → `sendActivePetitionMessage`,
    // G31): route the line to both participants, don't broadcast it.
    if matches!(chat_type, ChatType::PetitionPlayer | ChatType::PetitionGm) {
        super::super::moderation::petition::send_active_petition_message(
            world, sender_oid, &pkt.text,
        );
        return;
    }

    let Some(p) = world.objects.get_component::<Player>(&sender_oid) else {
        return;
    };
    let (sender_name, sender_level) = (p.name.clone(), p.level);

    // Java `Say2` `Config.LOG_CHAT`, in the same position: after every filter
    // (chat ban, curse, spam cap, L2Walker) and before the line is delivered,
    // so the audit file holds what was actually said rather than what was
    // attempted. Whisper records the addressee too, as Java's " to <target>".
    if world.cfg.general.log_chat {
        audit::record(
            audit::Category::Chat,
            json!({
                "chat_type": format!("{chat_type:?}"),
                "char_name": sender_name,
                "oid": sender_oid,
                "target": pkt.target,
                "text": pkt.text,
            }),
        );
    }

    // Java `Player.broadcastSnoop`: mirror the line to any GM snooping this
    // speaker (G31). Fires for every chat channel the player originates.
    broadcast_snoop(world, sender_oid, chat_type, &sender_name, &pkt.text);

    // `ChatGeneral`: a `.`-prefixed line is a voiced command, not chat — the
    // handler runs and the text is never broadcast. Java's `MasterHandler`
    // registers each handler only when its `Custom/*.ini` flag is on, so an
    // unregistered command falls through to being *said*; the gates below keep
    // that shape.
    //
    // `.password` is deliberately absent: `handlers/voicedcommandhandlers/
    // ChangePassword` needs `AllowChangePassword`, which is `False` on this
    // dist, so Java never registers it either and the line is said aloud —
    // which is what happens here too. Porting it would need the login server's
    // account-password path, since the game server does not hold credentials.
    if chat_type == ChatType::General
        && let Some(rest) = pkt.text.strip_prefix('.')
    {
        let command = rest.split_whitespace().next().unwrap_or("");
        match command {
            "offline" => {
                super::super::commerce::offline_trade::handle_voiced_offline(world, client_id);
                return;
            }
            // `handlers/voicedcommandhandlers/Premium` — the account panel.
            // Gated on the same `EnablePremiumSystem` Java gates it on, so with
            // the system off the line falls through and is said aloud.
            "premium" if world.cfg.premium.enabled => {
                super::super::admin::premium::show_premium_panel(world, client_id, sender_oid);
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
                super::super::automation::play::handle_voiced(
                    world, client_id, sender_oid, command, &args,
                );
                return;
            }
            "apon" | "apoff" | "potionon" | "potionoff" if world.cfg.auto_potions.enabled => {
                super::super::automation::potions::handle_voiced(
                    world, client_id, sender_oid, command,
                );
                return;
            }
            "sellbuff" | "sellbuffs" if world.cfg.sell_buffs.enabled => {
                super::super::commerce::sell_buffs::send_sell_menu(world, client_id, sender_oid);
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

    // Java `Say2`: `if (Config.USE_SAY_FILTER) checkText();` — the last thing
    // before the channel handler runs, so the *published* item links above are
    // matched against the raw line but everyone hears the filtered one.
    if let Some(filtered) = world.cfg.chat_filter.filter(&pkt.text) {
        pkt.text = filtered;
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
                // `ChatGeneral`: `!BlockList.isBlocked(player, activeChar)` —
                // the *listener's* list decides, one way only.
                if block_list::is_blocked(world, other_oid, sender_oid) {
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
            // `ChatTrade` alone has a **hard-coded** level gate before its
            // scope branch — not `MinimumChatLevel`, a literal 20. Shout has
            // none. Ported here rather than with the config gate above,
            // because it is not configurable.
            if chat_type == ChatType::Trade
                && world
                    .objects
                    .get_component::<Player>(&sender_oid)
                    .is_some_and(|p| p.level < TRADE_CHAT_MIN_LEVEL)
            {
                helpers::send_sm_to_client(
                    world,
                    client_id,
                    sm_ids::TRADE_CHAT_CANNOT_BE_USED_BELOW_LEVEL_S1,
                    &[server_packets::SmParam::Int(TRADE_CHAT_MIN_LEVEL)],
                );
                return;
            }
            // `GlobalChat` (Shout) / `TradeChat` (Trade) choose the audience.
            // `Region` and a `gm`-scoped channel for an override holder both
            // land here; `Global` goes server-wide below; anything else
            // matches no Java branch and the line is dropped in silence.
            let scope = if chat_type == ChatType::Trade {
                world.cfg.general.trade_chat
            } else {
                world.cfg.general.global_chat
            };
            let overrides = world
                .objects
                .get_component::<Player>(&sender_oid)
                .is_some_and(|p| p.can_override_cond(CHAT_CONDITIONS_ORDINAL));
            let region_scoped = match scope {
                crate::config::general::ChatScope::Region => true,
                crate::config::general::ChatScope::GmOnly => overrides,
                _ => false,
            };
            let server_wide = !region_scoped && scope == crate::config::general::ChatScope::Global;
            if !region_scoped && !server_wide {
                return;
            }
            // With `ON` (this dist): everyone whose position maps to the same
            // map-region *tile group* (`MapRegionManager.getMapRegionLocId`),
            // speaker included. Region identity = the region entry; two off-map
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
                // **Shout and Trade differ here, and it is not an oversight.**
                // `ChatShout`'s in-region branch tests *both* directions —
                // `!isBlocked(player, activeChar) && !isBlocked(activeChar,
                // player)` — so either party's list suppresses the line.
                // `ChatTrade`'s otherwise-identical branch tests only the
                // listener's. Collapsing them would silently change one.
                if block_list::is_blocked(world, other_oid, sender_oid)
                    || (chat_type == ChatType::Shout
                        && block_list::is_blocked(world, sender_oid, other_oid))
                {
                    continue;
                }
                let Some(other_pos) = world.objects.get_component::<Position>(&other_oid) else {
                    continue;
                };
                let other_region = world
                    .data
                    .map_region
                    .region_at(other_pos.x, other_pos.y)
                    .map(|r| r.name.clone());
                if server_wide || from_region == other_region {
                    cs.send(say.clone());
                }
            }
        }
        ChatType::Whisper => {
            let target_name = pkt.target.as_deref().unwrap_or_default();
            // World.getPlayer(name) — case-insensitive, so the reply must echo
            // the name as stored, not as the sender typed it.
            let receiver = super::super::party::find_player_by_name(world, target_name).and_then(
                |(cid, oid)| {
                    world
                        .objects
                        .get_component::<Player>(&oid)
                        .map(|p| (cid, oid, p.name.clone()))
                },
            );
            let Some((receiver_cid, receiver_oid, receiver_name)) = receiver else {
                helpers::send_sm_bare_to_client(
                    world,
                    client_id,
                    sm_ids::THAT_PLAYER_IS_NOT_ONLINE,
                );
                return;
            };
            // Java `ChatWhisper`: `BlockList.isBlocked(receiver, activeChar)`
            // refuses the PM — the sender gets the refusal notice, nothing is
            // delivered. **Both halves matter**: message-refusal mode (which is
            // all this used to check) *and* the receiver having the sender on
            // their ignore list. Java answers with the same message either way,
            // so a blocked sender cannot tell which it was.
            if block_list::is_blocked(world, receiver_oid, sender_oid) {
                helpers::send_sm_bare_to_client(
                    world,
                    client_id,
                    sm_ids::THAT_PERSON_IS_IN_MESSAGE_REFUSAL_MODE,
                );
                return;
            }
            // Relation mask: bit 0x01 = sender on the receiver's friend list
            // (wired with the friend system); other bits need clans/mentors.
            let mask = whisper_relation_mask(world, sender_oid, receiver_oid);
            helpers::send_to_client(
                world,
                receiver_cid,
                server_packets::creature_say(
                    sender_oid,
                    chat_type,
                    &sender_name,
                    &pkt.text,
                    Some((mask, sender_level)),
                ),
            );
            helpers::send_to_client(
                world,
                client_id,
                server_packets::creature_say(
                    sender_oid,
                    chat_type,
                    &format!("->{receiver_name}"),
                    &pkt.text,
                    Some((mask, sender_level)),
                ),
            );
        }
        ChatType::Party => {
            // ChatParty — `party.broadcastCreatureSay` (speaker included).
            let say =
                server_packets::creature_say(sender_oid, chat_type, &sender_name, &pkt.text, None);
            if !super::super::party::party_say(world, sender_oid, &say) {
                helpers::send_sm_bare_to_client(world, client_id, sm_ids::YOU_ARE_NOT_IN_A_PARTY);
            }
        }
        ChatType::PartyroomCommander | ChatType::PartyroomAll => {
            // ChatPartyRoomCommander / ChatPartyRoomAll: command channel chat.
            // Commander (15) — only the CC leader may speak; All (16) — any
            // party leader in the channel. Silently ignored otherwise (Java).
            // Every CC member receives it (the block-list skip is the same
            // absent-system gap as the other channels, see the module header).
            let Some(party_id) =
                super::super::party::command_channel::party_id_of(world, sender_oid)
            else {
                return;
            };
            let Some(cc_id) = super::super::party::command_channel::cc_id_of_party(world, party_id)
            else {
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
            super::super::party::command_channel::broadcast_to_cc(world, cc_id, &say);
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
                    helpers::send_to_player(world, oid, say.clone());
                }
            }
        }
        ChatType::Clan => {
            // ChatClan — `clan.broadcastToOnlineMembers` (speaker included).
            let clan_id = clan_of_or_zero(world, sender_oid);
            if clan_id != 0 && world.clans.contains_key(&clan_id) {
                let say = server_packets::creature_say(
                    sender_oid,
                    chat_type,
                    &sender_name,
                    &pkt.text,
                    None,
                );
                super::super::clans::broadcast_to_clan(world, clan_id, &say);
            } else {
                helpers::send_sm_bare_to_client(world, client_id, sm_ids::YOU_ARE_NOT_IN_A_CLAN);
            }
        }
        ChatType::Alliance => {
            helpers::send_sm_bare_to_client(world, client_id, sm_ids::YOU_ARE_NOT_IN_AN_ALLIANCE)
        }
        ChatType::World => world_chat(world, client_id, sender_oid, &sender_name, &pkt.text),
        ChatType::HeroVoice => hero_voice(world, client_id, sender_oid, &sender_name, &pkt.text),
        // Server-sent only (ferry announcements / event announcements) or handled
        // before the match (petition consultation chat); a client never
        // legitimately originates these here, so drop them.
        ChatType::Boat
        | ChatType::Announcement
        | ChatType::PetitionPlayer
        | ChatType::PetitionGm => {}
    }
}

/// `PlayerCondOverride.CHAT_CONDITIONS` — the GM escape hatch from the jail
/// chat gate.
const CHAT_CONDITIONS_ORDINAL: u8 = 8;

/// `ChatTrade`'s own level gate — a literal 20 in Java, unrelated to
/// `MinimumChatLevel`, and with no counterpart on Shout.
const TRADE_CHAT_MIN_LEVEL: i32 = 20;

/// The speaker's remaining world-chat allowance, i.e. what `ExWorldChatCnt`
/// reports: Java's constructor arithmetic
/// `level < WORLD_CHAT_MIN_LEVEL ? 0 : max(points - used, 0)`.
///
/// `points` is Java's `getWorldChatPoints()` — `WorldChatPointsPerDay` scaled
/// by the `Stat.WORLD_CHAT_POINTS` add/mul. **No skill, item or effect on this
/// dist grants that stat**, so the config value *is* the quota here and the
/// stat is not modelled; grep the datapack before assuming that still holds if
/// a later chronicle's data lands.
pub(crate) fn world_chat_points_left(world: &World, player_oid: i32) -> i32 {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return 0;
    };
    if p.level < world.cfg.general.world_chat_min_level {
        return 0;
    }
    let used = get_used_world_chat(world, player_oid);
    (world.cfg.general.world_chat_points_per_day - used).max(0)
}

/// Port of `handlers/chathandlers/ChatWorld` — the server-wide channel, gated
/// by a minimum level, a daily point quota and a per-speaker reuse window.
///
/// The gate order is Java's, and it matters: the **level** check runs before
/// the jail and quota checks, so an under-levelled character is told to level
/// up rather than told their quota is spent.
///
/// Two of Java's branches are deliberately absent:
///
/// * Its `isChatBanned() && BAN_CHAT_CHANNELS.contains(type)` branch is **dead
///   code upstream** — `Say2` returns unconditionally for a chat-banned
///   speaker long before dispatching to any handler, which is exactly what
///   this port does above. Porting it would be unreachable code that reads
///   like a live rule.
/// * Its `FACTION_SYSTEM_ENABLED && FACTION_SPECIFIC_CHAT` split (broadcast to
///   good or evil players only) is unreachable on this dist:
///   `Custom/FactionSystem.ini` ships `EnableFactionSystem = False`, so Java
///   takes the `else` — every online player — which is what runs here.
fn world_chat(world: &mut World, client_id: u32, sender_oid: i32, sender_name: &str, text: &str) {
    // Java returns *silently* when the system is off — no notice to the
    // speaker, so the line simply vanishes.
    if !world.cfg.general.world_chat_enabled {
        return;
    }

    let now = commons::util::now_millis();
    // Java `REUSE.values().removeIf(now::isAfter)`: drop entries whose instant
    // is already in the past. Runs before the gates, as upstream, so a lapsed
    // window is gone even when this call goes on to be refused for some other
    // reason.
    world.world_chat_reuse.retain(|_, until| *until >= now);

    let Some(p) = world.objects.get_component::<Player>(&sender_oid) else {
        return;
    };
    let (level, jailed, may_override) = (
        p.level,
        p.jailed,
        p.can_override_cond(CHAT_CONDITIONS_ORDINAL),
    );

    let min_level = world.cfg.general.world_chat_min_level;
    if level < min_level {
        helpers::send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                sm_ids::YOU_CAN_USE_WORLD_CHAT_FROM_LV_S1,
                &[commons::system_messages::SmParam::Int(min_level)],
            ),
        );
        return;
    }

    // The second of Java's two `JailDisableChat` gates — `Say2`'s is up in
    // `handle_say2`, over a different channel set and *without* the
    // `CHAT_CONDITIONS` escape this one has. See the note there.
    if world.cfg.general.jail_disable_chat && jailed && !may_override {
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::CHATTING_IS_CURRENTLY_PROHIBITED);
        return;
    }

    let used = get_used_world_chat(world, sender_oid);
    if used >= world.cfg.general.world_chat_points_per_day {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_HAVE_SPENT_YOUR_WORLD_CHAT_QUOTA_FOR_THE_DAY,
        );
        return;
    }

    // The reuse window. Java guards *both* the check and the later stamp with
    // `getSeconds() > 0`, so an interval of 0 disables the window rather than
    // making it instantaneous.
    let interval_secs = world.cfg.general.world_chat_interval_secs;
    if interval_secs > 0
        && let Some(&until) = world.world_chat_reuse.get(&sender_oid)
        && until > now
    {
        // Java `Duration.between(now, instant).getSeconds()` truncates, so a
        // 19.4 s wait reports 19 — matched by the integer division here.
        let remaining = ((until - now) / 1000) as i32;
        helpers::send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                sm_ids::YOU_HAVE_S1_SEC_UNTIL_YOU_ARE_ABLE_TO_USE_WORLD_CHAT,
                &[commons::system_messages::SmParam::Int(remaining)],
            ),
        );
        return;
    }

    // Java `activeChar.isNotBlocked(player)` — despite reading as "the speaker
    // is not blocked", `isNotBlocked` resolves against **`player`'s** list
    // (`!player.getBlockList().isBlockAll() &&
    // !player.getBlockList().isInBlockList(this)`), so it is the listener's
    // list here exactly as on every other channel. The method name is the trap.
    let say = server_packets::creature_say(sender_oid, ChatType::World, sender_name, text, None);
    let listeners: Vec<u32> = world
        .clients
        .iter()
        .filter(|(_, cs)| matches!(cs, ClientSession::InGame(_)))
        .filter(|(_, cs)| {
            let ClientSession::InGame(s) = cs else {
                return false;
            };
            block_list::is_not_blocked(world, s.player_object_id(), sender_oid)
        })
        .map(|(&cid, _)| cid)
        .collect();
    for cid in listeners {
        helpers::send_to_client(world, cid, say.clone());
    }

    // Spend the point, then tell the speaker what is left. Java writes the
    // variable through `setWorldChatUsed`, which the memory-first autosave
    // flushes with the rest of the character.
    helpers::set_player_var_int(
        world,
        sender_oid,
        crate::model::components::WORLD_CHAT_USED,
        used + 1,
    );
    let left = world_chat_points_left(world, sender_oid);
    helpers::send_to_client(world, client_id, server_packets::ex_world_chat_cnt(left));
    if interval_secs > 0 {
        world
            .world_chat_reuse
            .insert(sender_oid, now + interval_secs * 1000);
    }
}

/// Port of `handlers/chathandlers/ChatHeroVoice` — the hero channel (`%`):
/// server-wide, heroes only, one line per ten seconds.
///
/// Java's chat-ban and jail branches are deliberately absent for the same
/// reason as `world_chat`'s chat-ban branch: both are dead code upstream.
/// `Say2` returns for a chat-banned speaker before dispatching to any handler,
/// and its `JailDisableChat` gate already covers `HeroVoice` (see the note in
/// `handle_say2`). The faction split is unreachable on this dist
/// (`EnableFactionSystem = False`), so the broadcast is unconditional.
fn hero_voice(world: &mut World, client_id: u32, sender_oid: i32, sender_name: &str, text: &str) {
    // `!activeChar.isHero() && !activeChar.canOverrideCond(CHAT_CONDITIONS)`.
    let may_speak = world
        .objects
        .get_component::<Player>(&sender_oid)
        .is_some_and(|p| p.is_hero || p.can_override_cond(CHAT_CONDITIONS_ORDINAL));
    if !may_speak {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::ONLY_HEROES_CAN_ENTER_THE_HERO_CHANNEL,
        );
        return;
    }
    // `canUseHeroVoice()` — the one chat channel this dist rate-limits
    // (`FloodProtectorHeroVoiceInterval = 100` ticks, the ten seconds the
    // refusal line names). Java answers with `sendMessage(String)`, a literal
    // line, reproduced verbatim. `flood::gate`'s GM exemption stands in for
    // Java's `FLOOD_CONDITIONS` cond-override, as at every other call site.
    if !super::super::client::flood::gate(
        world,
        client_id,
        crate::config::flood_protector::FloodAction::HeroVoice,
    ) {
        helpers::send_message(
            world,
            client_id,
            "Action failed. Heroes are only able to speak in the global channel once every 10 seconds.",
        );
        return;
    }
    // `!BlockList.isBlocked(player, activeChar)` — the *listener's* list, one
    // way only, speaker included (`World.getPlayers` is everyone).
    let say =
        server_packets::creature_say(sender_oid, ChatType::HeroVoice, sender_name, text, None);
    let listeners: Vec<u32> = world
        .clients
        .iter()
        .filter(|(_, cs)| {
            let ClientSession::InGame(s) = cs else {
                return false;
            };
            !block_list::is_blocked(world, s.player_object_id(), sender_oid)
        })
        .map(|(&cid, _)| cid)
        .collect();
    for cid in listeners {
        helpers::send_to_client(world, cid, say.clone());
    }
}

/// The `CreatureSay` whisper-tail relation mask (receiver's view of the
/// sender). Only the friend bit (0x01) is representable so far — the
/// clan/mentor/ally bits need their systems.
fn whisper_relation_mask(world: &World, sender_oid: i32, receiver_oid: i32) -> u8 {
    if helpers::is_friend(world, sender_oid, receiver_oid) {
        0x01
    } else {
        0
    }
}

/// `SnoopQuit` (0xB4) — the GM closed a snoop window, so both halves of the
/// link come apart (`target.removeSnooper(player)` +
/// `player.removeSnooped(target)`).
///
/// Java takes the *snooped* player's object id off the wire and looks them up
/// in the world; a target who has since logged out leaves the GM's own
/// `snooped` entry behind, which is Java's behaviour and harmless — the entry
/// is only ever read to find live listeners.
pub(crate) fn handle_snoop_quit(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(gm) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let Some(snooped) = r.read_i32() else {
        return;
    };
    // `World.getPlayer(_snoopID) == null` → return, before either side is
    // touched.
    if !world.objects.has_component::<Player>(&snooped) {
        return;
    }
    if let Some(target) = world.objects.get_component_mut::<Player>(&snooped) {
        target.snoop_listeners.retain(|&oid| oid != gm);
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&gm) {
        p.snooped.retain(|&oid| oid != snooped);
    }
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
        helpers::send_to_player(world, gm, snoop.clone());
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
    helpers::send_to_client(
        world,
        client_id,
        crate::network::enter_world::ex_rp_item_link(&item, template, equipped),
    );
}

/// Java `World.findObject(objectId)` narrowed to items: the published item is
/// looked up live, so the reader sees its *current* state (enchant, count) and
/// the link keeps working after a trade. Only inventories of loaded players
/// are searched — an item whose owner logged off is gone from the world in
/// Java too, and its `_published` flag died with the `Item` instance.
fn find_published_item(world: &World, object_id: i32) -> Option<(ItemInstance, bool)> {
    let owners = world
        .in_game_player_oids()
        .chain(world.offline_traders.keys().copied());
    for owner in owners {
        let Some(inv) = world.objects.get_component::<Inventory>(&owner) else {
            continue;
        };
        if let Some(item) = inv.by_object_id(object_id) {
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
    helpers::send_message(world, client_id, &text);
}

/// `handlers/voicedcommandhandlers/Banking` — `.bank` explains the rate,
/// `.deposit` turns `BankingAdenaCount` adena into `BankingGoldbarCount`
/// goldbars, `.withdraw` reverses it. Java messages the shortfall rather than
/// refusing silently, and its `updateDatabase()` call is a no-op here: the port
/// is memory-first and the inventory flushes on the next autosave.
fn handle_voiced_banking(world: &mut World, client_id: u32, player_oid: i32, command: &str) {
    let (adena, goldbars) = (world.cfg.banking.adena, world.cfg.banking.goldbars);
    match command {
        "bank" => {
            let text = format!(
                ".deposit ({adena} Adena = {goldbars} Goldbar) / \
                 .withdraw ({goldbars} Goldbar = {adena} Adena)"
            );
            helpers::send_message(world, client_id, &text);
        }
        "deposit" => {
            if inventory::count_of(world, player_oid, ADENA_ITEM_ID) < adena {
                let text = format!(
                    "You do not have enough Adena to convert to Goldbar(s), \
                     you need {adena} Adena."
                );
                helpers::send_message(world, client_id, &text);
                return;
            }
            items::take_items(world, client_id, player_oid, ADENA_ITEM_ID, adena);
            items::add_inventory_item(world, player_oid, GOLDBAR_ITEM_ID, goldbars);
            let text =
                format!("Thank you, you now have {goldbars} Goldbar(s), and {adena} less adena.");
            helpers::send_message(world, client_id, &text);
        }
        "withdraw" => {
            if inventory::count_of(world, player_oid, GOLDBAR_ITEM_ID) < goldbars {
                let text = format!("You do not have any Goldbars to turn into {adena} Adena.");
                helpers::send_message(world, client_id, &text);
                return;
            }
            items::take_items(world, client_id, player_oid, GOLDBAR_ITEM_ID, goldbars);
            items::add_inventory_item(world, player_oid, ADENA_ITEM_ID, adena);
            let text =
                format!("Thank you, you now have {adena} Adena, and {goldbars} less Goldbar(s).");
            helpers::send_message(world, client_id, &text);
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
            super::super::admin::moderation::admin_ban_chat(world, client_id, sender_oid, &args)
        }
        _ => super::super::admin::moderation::admin_unban_chat(world, client_id, &args),
    }
}
fn get_used_world_chat(world: &World, player_oid: i32) -> i32 {
    helpers::player_var_int(
        world,
        player_oid,
        crate::model::components::WORLD_CHAT_USED,
        0,
    )
}
