//! `BypassUserCmd` (0xB3) — the client `/command` bar — routed to ports of
//! `handlers/usercommandhandlers/*` (G15.5). Unknown ids answer the Java
//! GM-only "not implemented" message.
//!
//! Every handler this Java build registers is ported: `/loc` 0, `/unstuck` 52,
//! `/mount` 61, `/dismount` 62, `/time` 77, `/partyinfo` 81, the clan-war lists
//! 88/89, `/instancezone` 90, the command-channel trio 93/96/97,
//! `/siegestatus` 99, `/clanpenalty` 100, `/olympiadstat` 109 and
//! `/mybirthday` 126.
//!
//! **Id 90 belongs to `InstanceZone`, not `ClanWarsList`**: Java's
//! `MasterHandler` registers `ClanWarsList` (88/89/90) *before* `InstanceZone`
//! (90) and `registerHandler` overwrites by id, so the war-list's third id is
//! shadowed — the mutual-war list is unreachable in this build. Kept.

use super::helpers::{send_message, send_sm_to_client as send_sm};
use crate::game_loop::helpers;
use crate::game_loop::helpers::clan_of;

use crate::model::components::{Casting, Position};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

/// `Util.checkIfInRange(200, this, pet, true)` — how close the mount must be.
const MOUNT_RANGE: f64 = 200.0;

const USER_CMD_LOC: i32 = 0;
const USER_CMD_UNSTUCK: i32 = 52;
/// `usercommandhandlers/Mount.java` — `/mount` (id 61): ride the summoned pet.
const USER_CMD_MOUNT: i32 = 61;
/// `usercommandhandlers/Dismount.java` — `/dismount` (id 62).
const USER_CMD_DISMOUNT: i32 = 62;
const USER_CMD_TIME: i32 = 77;
const USER_CMD_PARTY_INFO: i32 = 81;
/// `ClanWarsList` — 88 attack list, 89 under-attack list. Its third id (90) is
/// shadowed by `InstanceZone` (see the module docs).
const USER_CMD_CLAN_WAR_ATTACK: i32 = 88;
const USER_CMD_CLAN_WAR_UNDER_ATTACK: i32 = 89;
const USER_CMD_INSTANCE_ZONE: i32 = 90;
const USER_CMD_CHANNEL_DELETE: i32 = 93;
const USER_CMD_CHANNEL_LEAVE: i32 = 96;
const USER_CMD_CHANNEL_INFO: i32 = 97;
const USER_CMD_SIEGE_STATUS: i32 = 99;
const USER_CMD_CLAN_PENALTY: i32 = 100;
const USER_CMD_OLYMPIAD_STAT: i32 = 109;
const USER_CMD_MY_BIRTHDAY: i32 = 126;

/// The 5-minute escape (`SkillData.getSkill(2099, 1)`) and the GM 1-second
/// escape (2100) — both `SELF`-targeted static skills with the
/// `Escape TOWN` effect.
const ESCAPE_SKILL_ID: i32 = 2099;
const GM_ESCAPE_SKILL_ID: i32 = 2100;

pub(crate) fn handle_bypass_user_cmd(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(command_id) = cp::read_user_command(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };

    match command_id {
        USER_CMD_LOC => loc(world, client_id, object_id),
        USER_CMD_UNSTUCK => unstuck(world, client_id, object_id),
        // Java `Dismount.useUserCommand` -> `player.dismount()` (only when
        // riding; silently ignored otherwise, same as Java's isRentedPet=false
        // + isMounted=false fallthrough).
        USER_CMD_DISMOUNT => super::admin::mounts::dismount(world, object_id),
        USER_CMD_MOUNT => mount(world, client_id, object_id),
        USER_CMD_TIME => time(world, client_id),
        USER_CMD_PARTY_INFO => party_info(world, client_id, object_id),
        USER_CMD_CLAN_WAR_ATTACK | USER_CMD_CLAN_WAR_UNDER_ATTACK => {
            clan_wars_list(world, client_id, object_id, command_id)
        }
        USER_CMD_INSTANCE_ZONE => instance_zone(world, client_id, object_id),
        USER_CMD_CHANNEL_DELETE => channel_delete(world, client_id, object_id),
        USER_CMD_CHANNEL_LEAVE => channel_leave(world, client_id, object_id),
        USER_CMD_CHANNEL_INFO => channel_info(world, client_id, object_id),
        USER_CMD_SIEGE_STATUS => siege_status(world, client_id, object_id),
        USER_CMD_CLAN_PENALTY => clan_penalty(world, client_id, object_id),
        USER_CMD_OLYMPIAD_STAT => olympiad_stat(world, client_id, object_id),
        USER_CMD_MY_BIRTHDAY => my_birthday(world, client_id, object_id),
        _ => {
            // `BypassUserCmd.runImpl`'s missing-handler branch: GMs get told,
            // players get silence.
            let is_gm = world
                .objects
                .get_component::<crate::model::Player>(&object_id)
                .is_some_and(|p| p.is_gm(&world.data));
            if is_gm {
                send_message(
                    world,
                    client_id,
                    &format!("User commandID {command_id} not implemented yet."),
                );
            }
        }
    }
}

/// Port of `usercommandhandlers/Loc.java`: the map region's `locId` is itself
/// the "Current location: $s1 / $s2 / $s3 (near …)" system-message id.
/// Simplifications: no `RespawnZone` redirect (zone type unported), and the
/// three coordinate params are always attached — every `locId` this dist's
/// mapregion files reference is a 3-param location message (Java checks
/// `getParamCount() == 3` against its message table, which isn't ported).
fn loc(world: &World, client_id: u32, object_id: i32) {
    let Some(pos) = world.objects.get_component::<Position>(&object_id) else {
        return;
    };
    let loc_id = world
        .data
        .map_region
        .region_at(pos.x, pos.y)
        .map(|r| r.loc_id)
        .unwrap_or(0);
    let packet = if loc_id > 0 {
        server_packets::system_message_with(
            loc_id as i16,
            &[
                SmParam::Int(pos.x),
                SmParam::Int(pos.y),
                SmParam::Int(pos.z),
            ],
        )
    } else {
        server_packets::system_message_with(
            sm_ids::CURRENT_LOCATION_S1,
            &[SmParam::Text(format!("{}, {}, {}", pos.x, pos.y, pos.z))],
        )
    };
    helpers::send_to_client(world, client_id, packet);
}

/// Port of `usercommandhandlers/Unstuck.java`: cast the escape skill and let
/// its `Escape TOWN` effect do the teleport on landing. Skipped guards
/// (systems don't exist): jail, faction, olympiad, mute, observer, combat
/// flag, `isMovementDisabled` beyond death.
fn unstuck(world: &mut World, client_id: u32, object_id: i32) {
    // `isCastingNow || isAlikeDead` → refuse silently (Java returns false).
    if world.objects.has_component::<Casting>(&object_id) || helpers::is_dead(world, object_id) {
        return;
    }
    let (is_gm, interval) = {
        let Some(p) = world
            .objects
            .get_component::<crate::model::Player>(&object_id)
        else {
            return;
        };
        (p.is_gm(&world.data), world.cfg.character.unstuck_interval)
    };

    if is_gm {
        // GM: the stock 1-second escape.
        if let Some(skill) = helpers::skill_by_id(world, GM_ESCAPE_SKILL_ID, 1) {
            super::skills::cast::start_casting(world, client_id, object_id, &skill, object_id);
        } else {
            send_message(world, client_id, "You use Escape: 1 second.");
        }
        return;
    }
    let Some(mut skill) = helpers::skill_by_id(world, ESCAPE_SKILL_ID, 1) else {
        return;
    };
    if interval == 300 {
        // The stock 5-minute skill matches the config — cast unmodified.
        super::skills::cast::start_casting(world, client_id, object_id, &skill, object_id);
        return;
    }
    // Custom interval (30 s on this dist): Java forces the cast time via
    // `SkillCaster.castSkill(..., unstuckTimer)`. The skill is static
    // (`isMagic=2`), so the overridden hitTime is used verbatim.
    let unstuck_ms = interval * 1000;
    skill.hit_time = unstuck_ms;
    // The cast goes FIRST. Java's `SkillCaster.castSkill` runs phase 0
    // synchronously (`skillCaster.run()`), so `startCasting`'s YOU_USE_S1 +
    // SetupGauge reach the client *before* the handler's own chat line — the
    // player reads "You use Escape (5-minute)." (the client's own name for
    // skill 2099; the 5 minutes is a lie, the forced hit time is 30 s) and
    // then "You use Escape: 30 seconds." Sending the chat line first inverts
    // the pair.
    super::skills::cast::start_casting(world, client_id, object_id, &skill, object_id);
    // ...and the chat line is gated on the cast having started: Java answers a
    // null `SkillCaster` with ActionFailed + `setIntention(AI_INTENTION_ACTIVE)`
    // and *no* message, so a refused escape never claims to have worked.
    // `start_casting` returns `()`, but it is the only thing that adds
    // `Casting` and the guard at the top of this fn already proved the slot
    // was empty — so the component is the "cast started" signal.
    if !world.objects.has_component::<Casting>(&object_id) {
        helpers::send_action_failed(world, client_id);
        world
            .objects
            .remove_component::<crate::model::components::Intent>(&object_id);
        return;
    }
    if interval > 100 {
        send_message(
            world,
            client_id,
            &format!("You use Escape: {} minutes.", unstuck_ms / 60000),
        );
    } else {
        send_message(
            world,
            client_id,
            &format!("You use Escape: {} seconds.", unstuck_ms / 1000),
        );
    }
}

/// Port of `usercommandhandlers/Time.java` — the in-game clock, in the client's
/// own "current time is $s1:$s2" message (a night variant of the same string).
/// `DisplayServerTime`'s extra line is config-gated off on this dist.
fn time(world: &World, client_id: u32) {
    let (message, hour, minute) = time_message(commons::util::now_millis());
    // Java pads the minutes to two digits and passes both as *strings*.
    let packet =
        server_packets::system_message_with(message, &[SmParam::Text(hour), SmParam::Text(minute)]);
    helpers::send_to_client(world, client_id, packet);
}

/// The message id and the two string params `/time` sends for `now_millis`:
/// the hour, the zero-padded minute, and the night variant of the string after
/// dark (Java `GameTimeTaskManager.isNight()`).
pub(super) fn time_message(now_millis: i64) -> (i16, String, String) {
    let t = super::game_time::game_time_minutes_at(now_millis);
    let message = if super::game_time::is_night_at(now_millis) {
        sm_ids::THE_CURRENT_TIME_IS_S1_S2_NIGHT
    } else {
        sm_ids::THE_CURRENT_TIME_IS_S1_S2
    };
    (
        message,
        ((t / 60) % 24).to_string(),
        format!("{:02}", t % 60),
    )
}

/// Port of `usercommandhandlers/PartyInfo.java`: the header, the loot rule (only
/// when in a party), and Java's trailing blank line.
fn party_info(world: &World, client_id: u32, object_id: i32) {
    use crate::model::party::LootRule;

    let mut messages = vec![sm_ids::PARTY_INFORMATION];
    if let Some(rule) = super::command_channel::party_id_of(world, object_id)
        .and_then(|id| world.parties.get(&id))
        .map(|p| p.distribution)
    {
        messages.push(match rule {
            LootRule::FindersKeepers => sm_ids::LOOTING_METHOD_FINDERS_KEEPERS,
            LootRule::Random => sm_ids::LOOTING_METHOD_RANDOM,
            LootRule::RandomIncludingSpoil => sm_ids::LOOTING_METHOD_RANDOM_INCLUDING_SPOIL,
            LootRule::ByTurn => sm_ids::LOOTING_METHOD_BY_TURN,
            LootRule::ByTurnIncludingSpoil => sm_ids::LOOTING_METHOD_BY_TURN_INCLUDING_SPOIL,
        });
    }
    messages.push(sm_ids::EMPTY_3);
    for id in messages {
        send_sm(world, client_id, id, &[]);
    }
}

/// Port of `usercommandhandlers/ClanWarsList.java` (ids 88/89 — 90 is shadowed,
/// see the module docs). Java's SQL reads the war rows and excludes the mutual
/// ones; the port filters `World.clan_wars` the same way. Each row is one
/// system message naming the other clan, with its alliance when it has one.
fn clan_wars_list(world: &World, client_id: u32, object_id: i32, command_id: i32) {
    let Some(clan_id) = clan_of(world, object_id) else {
        send_sm(world, client_id, sm_ids::NOT_JOINED_IN_ANY_CLAN, &[]);
        return;
    };
    let attacking = command_id == USER_CMD_CLAN_WAR_ATTACK;
    // Java's `clan2 NOT IN (SELECT clan1 …)` — the other side hasn't declared
    // back, i.e. the war is one-directional.
    let mutual = |a: i32, b: i32| {
        world
            .clan_wars
            .iter()
            .any(|w| w.attacker_id == b && w.attacked_id == a)
    };
    let others: Vec<i32> = world
        .clan_wars
        .iter()
        .filter_map(|w| {
            if attacking && w.attacker_id == clan_id && !mutual(clan_id, w.attacked_id) {
                Some(w.attacked_id)
            } else if !attacking && w.attacked_id == clan_id && !mutual(clan_id, w.attacker_id) {
                Some(w.attacker_id)
            } else {
                None
            }
        })
        .collect();

    let header = if attacking {
        sm_ids::CLANS_YOU_VE_DECLARED_WAR_ON
    } else {
        sm_ids::CLANS_THAT_HAVE_DECLARED_WAR_ON_YOU
    };
    send_sm(world, client_id, header, &[]);
    for other in others {
        let Some(clan) = world.clans.get(&other) else {
            continue;
        };
        let packet = if clan.ally_id > 0 {
            server_packets::system_message_with(
                sm_ids::S1_S2_ALLIANCE,
                &[
                    SmParam::Text(clan.name.clone()),
                    SmParam::Text(clan.ally_name.clone()),
                ],
            )
        } else {
            server_packets::system_message_with(
                sm_ids::S1_NO_ALLIANCE_EXISTS,
                &[SmParam::Text(clan.name.clone())],
            )
        };
        helpers::send_to_client(world, client_id, packet);
    }
    send_sm(world, client_id, sm_ids::EMPTY_3, &[]);
}

/// Port of `usercommandhandlers/InstanceZone.java` — `ExInzoneWaiting`, the
/// instance re-enter window. The penalty list is empty, and that is exact
/// parity: the only template with a `<reenter>` block on this dist
/// (LastImperialTomb 136) declares no `apply` attribute, which Java parses as
/// `InstanceReenterType.NONE` — so `setReenterTime` never fires anywhere,
/// `character_instance_time` stays empty, and Java's own list is permanently
/// empty too (verified 2026-08-06). The current instance's template id is
/// real.
fn instance_zone(world: &World, client_id: u32, object_id: i32) {
    // Java `InstanceManager.getPlayerInstance(player, false).getTemplateId()`;
    // -1 when the player is in the overworld.
    let instance_id = super::helpers::instance_of(world, object_id);
    let template_id = world
        .instances
        .get(instance_id)
        .map(|i| i.template_id)
        .filter(|&id| id >= 0)
        .unwrap_or(-1);
    helpers::send_to_client(
        world,
        client_id,
        server_packets::ex_inzone_waiting(template_id, &[]),
    );
}

/// Port of `usercommandhandlers/ChannelDelete.java`: only the channel leader,
/// who must also lead their own party, may disband it.
fn channel_delete(world: &mut World, client_id: u32, object_id: i32) {
    let Some(cc_id) = leading_channel(world, object_id) else {
        return;
    };
    super::command_channel::broadcast_sm_to_cc(
        world,
        cc_id,
        sm_ids::THE_COMMAND_CHANNEL_HAS_BEEN_DISBANDED,
        &[],
    );
    super::command_channel::disband_channel(world, cc_id);
    let _ = client_id;
}

/// Port of `usercommandhandlers/ChannelLeave.java`: a *party* leader takes their
/// party out of the channel.
fn channel_leave(world: &mut World, client_id: u32, object_id: i32) {
    let party_id = super::command_channel::party_id_of(world, object_id);
    let is_party_leader = party_id
        .and_then(|id| world.parties.get(&id))
        .is_some_and(|p| p.leader() == object_id);
    if !is_party_leader {
        send_sm(
            world,
            client_id,
            sm_ids::ONLY_A_PARTY_LEADER_CAN_LEAVE_A_COMMAND_CHANNEL,
            &[],
        );
        return;
    }
    let party_id = party_id.expect("checked above");
    let Some(cc_id) = super::command_channel::cc_id_of_party(world, party_id) else {
        return;
    };
    let leader_name = helpers::player_name_or_empty(world, object_id);
    // Java: the leaving party's leader is told, and the channel is told whose
    // party left — both *before* the party is unhooked.
    send_sm(
        world,
        client_id,
        sm_ids::YOU_HAVE_QUIT_THE_COMMAND_CHANNEL,
        &[],
    );
    super::command_channel::broadcast_sm_to_cc(
        world,
        cc_id,
        sm_ids::C1_S_PARTY_HAS_LEFT_THE_COMMAND_CHANNEL,
        &[SmParam::PlayerName(leader_name)],
    );
    super::command_channel::remove_party_from_channel(world, cc_id, party_id);
}

/// Port of `usercommandhandlers/ChannelInfo.java` — the channel roster window.
/// Silent when the player isn't in a channel (Java returns false).
fn channel_info(world: &World, client_id: u32, object_id: i32) {
    let Some(cc_id) = super::command_channel::party_id_of(world, object_id)
        .and_then(|party_id| super::command_channel::cc_id_of_party(world, party_id))
    else {
        return;
    };
    let Some(cc) = world.command_channels.get(&cc_id) else {
        return;
    };
    let parties: Vec<(String, i32, i32)> = cc
        .parties
        .iter()
        .filter_map(|party_id| world.parties.get(party_id))
        .map(|p| {
            let leader = p.leader();
            let name = world
                .objects
                .get_component::<crate::model::Player>(&leader)
                .map(|pl| pl.name.clone())
                .unwrap_or_default();
            (name, leader, p.members.len() as i32)
        })
        .collect();
    let leader_name = world
        .objects
        .get_component::<crate::model::Player>(&cc.leader)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let member_count = parties.iter().map(|(_, _, n)| n).sum::<i32>();
    let packet =
        server_packets::ex_multi_party_command_channel_info(&leader_name, member_count, &parties);
    helpers::send_to_client(world, client_id, packet);
}

/// The command channel this player leads — Java's compound guard for
/// `/channeldelete`: in a party, leading it, in a channel, and leading that.
fn leading_channel(world: &World, object_id: i32) -> Option<u32> {
    let party_id = super::command_channel::party_id_of(world, object_id)?;
    if world.parties.get(&party_id)?.leader() != object_id {
        return None;
    }
    let cc_id = super::command_channel::cc_id_of_party(world, party_id)?;
    world
        .command_channels
        .get(&cc_id)
        .filter(|cc| cc.is_leader(object_id))
        .map(|_| cc_id)
}

/// Port of `usercommandhandlers/SiegeStatus.java`: a noble clan leader whose
/// clan is attacking or defending a running siege sees where each online member
/// stands relative to the battlefield.
///
/// The kill/death counters are Java's `Clan.getSiegeKills()`/`getSiegeDeaths()`,
/// which **nothing in this build ever increments** (`increaseSiegeKills` has no
/// caller), so the page reports 0/0 there exactly as Java does.
fn siege_status(world: &World, client_id: u32, object_id: i32) {
    let refuse = |world: &World| {
        send_sm(
            world,
            client_id,
            sm_ids::ONLY_A_NOBLE_CLAN_LEADER_CAN_VIEW_THE_SIEGE_STATUS,
            &[],
        );
    };
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    let (clan_id, is_noble, is_leader) = (player.clan_id, player.is_noble, player.clan_leader);
    if !is_noble || !is_leader || clan_id == 0 {
        refuse(world);
        return;
    }
    // The first running siege this clan is registered in, either side.
    let Some(castle_id) = world
        .sieges
        .values()
        .filter(|s| s.in_progress)
        .find(|s| {
            s.clans.iter().any(|c| {
                c.clan_id == clan_id
                    && matches!(
                        c.kind,
                        crate::model::siege::SiegeClanType::Attacker
                            | crate::model::siege::SiegeClanType::Defender
                            | crate::model::siege::SiegeClanType::Owner
                    )
            })
        })
        .map(|s| s.castle_id)
    else {
        refuse(world);
        return;
    };
    let Some(page) = crate::data::htm_cache::read_htm_for(
        world,
        object_id,
        format!("{}data/html/siege/siege_status.htm", world.data.root),
    ) else {
        return;
    };
    let mut rows = String::new();
    for (name, inside) in online_clan_members(world, clan_id, castle_id) {
        rows.push_str(&format!(
            "<tr><td width=170>{name}</td><td width=100>{}</td></tr>",
            if inside {
                "In the siege zone"
            } else {
                "Not in the siege zone"
            }
        ));
    }
    let page = page
        .replace("%kill_count%", "0")
        .replace("%death_count%", "0")
        .replace("%member_list%", &rows);
    helpers::send_to_client(world, client_id, server_packets::npc_html_message(0, &page));
}

/// Every online member of `clan_id` with whether they stand in **this castle's**
/// siege zone (Java `siege.getCastle().getZone().isInsideZone(member)`).
fn online_clan_members(world: &World, clan_id: i32, castle_id: i32) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for session in world.clients.values() {
        let ClientSession::InGame(s) = session else {
            continue;
        };
        let oid = s.player_object_id();
        let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) else {
            continue;
        };
        if p.clan_id != clan_id {
            continue;
        }
        let inside = world
            .objects
            .get_component::<Position>(&oid)
            .is_some_and(|pos| {
                world.data.zone_data.zones_at(pos.x, pos.y, pos.z).any(|z| {
                    z.kind == crate::data::zone_data::ZoneKind::Siege && z.castle_id == castle_id
                })
            });
        out.push((p.name.clone(), inside));
    }
    out
}

/// Port of `usercommandhandlers/ClanPenalty.java` — the clan penalty table,
/// built as raw html (Java composes the same string inline).
fn clan_penalty(world: &World, client_id: u32, object_id: i32) {
    let now = commons::util::now_millis();
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    let mut rows = String::new();
    let mut penalty = false;
    let add = |label: &str, expiry: i64, rows: &mut String| {
        if expiry > now {
            rows.push_str(&format!(
                "<td width=170>{label}</td><td width=100 align=center>{}</td>",
                commons::util::format_date(expiry)
            ));
            true
        } else {
            false
        }
    };
    penalty |= add(
        "Unable to join a clan.",
        player.clan_join_expiry_time,
        &mut rows,
    );
    penalty |= add(
        "Unable to create a clan.",
        player.clan_create_expiry_time,
        &mut rows,
    );
    let clan_penalty_expiry = world
        .clans
        .get(&player.clan_id)
        .map_or(0, |c| c.char_penalty_expiry_time);
    penalty |= add(
        "Unable to invite a clan member.",
        clan_penalty_expiry,
        &mut rows,
    );
    if !penalty {
        rows.push_str("<td width=170>No penalty is imposed.</td><td width=100 align=center></td>");
    }
    let html = format!(
        "<html><body><center><table width=270 border=0 bgcolor=111111><tr>\
<td width=170>Penalty</td><td width=100 align=center>Expiration Date</td></tr></table>\
<table width=270 border=0><tr>{rows}</tr></table>\
<img src=\"L2UI.SquareWhite\" width=270 height=1></center></body></html>"
    );
    helpers::send_to_client(world, client_id, server_packets::npc_html_message(0, &html));
}

/// Port of `usercommandhandlers/OlympiadStat.java`: the *target's* Olympiad
/// record (Java reads the command user's own match counts but the **target's**
/// points — a quirk kept), plus this week's remaining matches.
fn olympiad_stat(world: &World, client_id: u32, object_id: i32) {
    use crate::model::components::TargetRef;

    if !crate::model::olympiad::OLYMPIAD_ENABLED {
        send_sm(
            world,
            client_id,
            sm_ids::THE_OLYMPIAD_GAMES_ARE_NOT_CURRENTLY_IN_PROGRESS,
            &[],
        );
        return;
    }
    // Java: the target must be a player who has completed the 2nd class transfer.
    let target = world
        .objects
        .get_component::<TargetRef>(&object_id)
        .and_then(|t| t.0)
        .filter(|oid| {
            world
                .objects
                .get_component::<crate::model::Player>(oid)
                .is_some_and(|p| helpers::class_level(world, p.class_id) >= 2)
        });
    let Some(target) = target else {
        send_sm(
            world,
            client_id,
            sm_ids::COMMAND_AVAILABLE_AFTER_THE_2ND_CLASS_TRANSFER,
            &[],
        );
        return;
    };
    let stats = world.olympiad.nobles.get(&object_id);
    let (done, won, lost) = stats.map_or((0, 0, 0), |n| (n.comp_done, n.comp_won, n.comp_lost));
    let points = world
        .olympiad
        .nobles
        .get(&target)
        .map_or(world.cfg.olympiad.start_points, |n| n.points);
    send_sm(
        world,
        client_id,
        sm_ids::FOR_THE_CURRENT_OLYMPIAD_YOU_HAVE_PARTICIPATED,
        &[
            SmParam::Int(done),
            SmParam::Int(won),
            SmParam::Int(lost),
            SmParam::Int(points),
        ],
    );
    send_sm(
        world,
        client_id,
        sm_ids::THE_MATCHES_THIS_WEEK_ARE_ALL_CLASS_BATTLES,
        &[SmParam::Int(world.olympiad.remaining_weekly_matches(
            object_id,
            world.cfg.olympiad.max_weekly_matches,
        ))],
    );
}

/// Port of `usercommandhandlers/MyBirthday.java` — the character's creation date
/// (`characters.create_date`, stored `YYYY-MM-DD`).
fn my_birthday(world: &World, client_id: u32, object_id: i32) {
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    let mut parts = player.create_date.split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return;
    };
    // Java passes the numbers as strings, month/day unpadded.
    let trim = |v: &str| v.trim_start_matches('0').to_string();
    send_sm(
        world,
        client_id,
        sm_ids::C1_S_BIRTHDAY_IS_S3_S4_S2,
        &[
            SmParam::PlayerName(player.name.clone()),
            SmParam::Text(year.to_string()),
            SmParam::Text(trim(month)),
            SmParam::Text(trim(day)),
        ],
    );
}

/// Port of `usercommandhandlers/Mount.java` → `Player.mountPlayer(getPet())`:
/// ride the summoned pet. The gate ladder is Java's, in order; each refusal has
/// its own "strider" message. A player who is *already* mounted dismounts
/// instead (Java's `else if (isMounted())` branch), minus its NO_LANDING /
/// hungry guards — no landing zones are loaded and mount feeding is unported.
pub(crate) fn mount(world: &mut World, client_id: u32, object_id: i32) {
    use crate::model::Player;

    if world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(Player::is_mounted)
    {
        super::admin::mounts::dismount(world, object_id);
        return;
    }
    let Some(pet) = super::servitor::pet_of(world, object_id) else {
        return; // Java: no pet → the whole branch is skipped
    };
    let Some(pet_npc_id) = helpers::npc_id_of(world, pet) else {
        return;
    };
    let Some(mount_type) = mount_type_of(world, pet_npc_id) else {
        return; // `!pet.isMountable()`
    };

    let refuse = |world: &World, sm: i16| {
        helpers::send_action_failed(world, client_id);
        send_sm(world, client_id, sm, &[]);
    };
    let dead = |world: &World, oid: i32| helpers::is_dead(world, oid);
    if dead(world, object_id) {
        refuse(world, sm_ids::A_STRIDER_CANNOT_BE_RIDDEN_WHEN_DEAD);
        return;
    }
    if dead(world, pet) {
        refuse(world, sm_ids::A_DEAD_STRIDER_CANNOT_BE_RIDDEN);
        return;
    }
    if in_combat(world, pet) {
        refuse(world, sm_ids::A_STRIDER_IN_BATTLE_CANNOT_BE_RIDDEN);
        return;
    }
    if in_combat(world, object_id) {
        refuse(world, sm_ids::A_STRIDER_CANNOT_BE_RIDDEN_WHILE_IN_BATTLE);
        return;
    }
    // A seated rider is refused with its own message.
    if crate::game_loop::sit_stand::is_sitting(world, object_id) {
        refuse(world, sm_ids::A_STRIDER_CAN_BE_RIDDEN_ONLY_WHEN_STANDING);
        return;
    }
    //
    // Fishing and transformation refuse with a bare `ActionFailed`, as in Java.
    if world
        .objects
        .has_component::<crate::model::components::FishingSession>(&object_id)
        || world
            .objects
            .get_component::<Player>(&object_id)
            .is_some_and(|p| p.transform_id != 0)
    {
        helpers::send_action_failed(world, client_id);
        return;
    }
    if !crate::geo::distance::within_3d(world, object_id, pet, MOUNT_RANGE) {
        refuse(world, sm_ids::YOU_ARE_TOO_FAR_AWAY_FROM_YOUR_MOUNT_TO_RIDE);
        return;
    }

    // Java `Player.mount(pet)`: mount on the pet's template, then unsummon it.
    // `setMountObjectID(pet.getControlObjectId())` — remembered so the
    // dismount can `storePetFood` the drained gauge back onto the collar row.
    let collar = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet)
        .map_or(0, |p| p.collar_object_id);
    if super::admin::mounts::mount_player(world, object_id, pet_npc_id, mount_type) {
        if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
            p.mount_collar_object_id = collar;
        }
        // Capture the pet's state before the entity goes away, like every
        // other unsummon site — without this the ride dropped the pet's
        // hp/exp/fed deltas since its summon.
        super::servitor::sync_pet_row(world, object_id);
        super::servitor::unsummon_servitor(world, object_id);
    }
}

/// Java `MountType.findByNpcId` — the pet template's mount category
/// (`STRIDER` / `WYVERN_GROUP` / `WOLF_GROUP`), `None` for an unrideable pet.
fn mount_type_of(world: &World, npc_id: i32) -> Option<u8> {
    let c = &world.data.categories;
    if c.contains("STRIDER", npc_id) {
        Some(1)
    } else if c.contains("WYVERN_GROUP", npc_id) {
        Some(2)
    } else if c.contains("WOLF_GROUP", npc_id) {
        Some(3)
    } else {
        None
    }
}

/// `Creature.isInCombat()` — the attack stance is still up.
pub(crate) fn in_combat(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::AttackState>(&object_id)
        .is_some_and(|a| a.stance_until_tick > world.tick)
}
