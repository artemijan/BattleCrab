//! Clan academy (G18.6) — the half of Java's clan system that G18's eight
//! slices left behind.
//!
//! An **academy member** is a low-level, un-transferred character parked in a
//! clan's `SUBUNIT_ACADEMY` (-1) sub-pledge. They are second-class citizens by
//! design: rank 9, no rank promotion, no clan-leader nomination, exempt from
//! clan-war kill accounting. What makes them worth having is
//! [`graduate`]: when the character completes its **2nd class transfer** the
//! clan is paid reputation on a sliding scale and the graduate is ousted, free
//! of the usual rejoin penalty.
//!
//! **`lvl_joined_academy` is the whole state machine.** Java's
//! `isAcademyMember()` is literally `_lvlJoinedAcademy > 0`, and the graduation
//! reward scales off the level the character *joined* at, not the level they
//! graduate at — a level-10 recruit is worth 650 reputation and a level-39 one
//! 190. That is why the field is set at join time and cleared in exactly three
//! places (graduation, leaving the clan, clan dissolution).
//!
//! **Apprentice / sponsor** is the mentorship pair layered on top: a full
//! member sponsors one academy member, each row holding the other's object id.
//! Java writes both `characters` rows straight through *even when the players
//! are online* — "since both must match" — so the port does the same rather
//! than waiting for an autosave.
//!
//! Out of scope, with reasons: **squad skills** (`subPledgeSkillTree.xml`) need
//! clan level 8+ and Knight's Epaulettes (item 9910/9911), and the file's own
//! comment marks the tree "Confirmed CT2.5" — later-chronicle content that no
//! Interlude clan can reach (documented at the `RequestAcquireSkillInfo`
//! SUBPLEDGE arm rather than deferred — it is a *verified skip*, not a
//! gap).

use crate::game_loop::guard::clan_of;
use tracing::info;

use crate::db::DbCommand;
use crate::model::Player;
use crate::model::clan::SUBUNIT_ACADEMY;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::world::World;

/// Java `Player.setClassId`'s graduation gift — the Clan Academy Circlet.
const ACADEMY_CIRCLET: i32 = 8181;

/// Java `Player.isAcademyMember()` — `_lvlJoinedAcademy > 0`. Note this is
/// **not** "sits in the academy sub-pledge": the field is what the checks read,
/// and it outlives a sub-pledge move.
pub(crate) fn is_academy_member(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.lvl_joined_academy > 0)
}

/// Whether the clan member (online or not) is an academy member, for the checks
/// that run against a roster row rather than a live player.
pub(crate) fn member_is_academy(world: &World, clan_id: i32, char_id: i32) -> bool {
    if is_academy_member(world, char_id) {
        return true;
    }
    world
        .clans
        .get(&clan_id)
        .and_then(|c| c.members.iter().find(|m| m.char_id == char_id))
        .is_some_and(|m| m.pledge_type == SUBUNIT_ACADEMY)
}

/// Java `RequestAnswerJoinPledge`: `setLvlJoinedAcademy(player.getLevel())`
/// when the invite was for the academy sub-pledge. Called from the join path
/// once the member row exists.
pub(crate) fn on_join(world: &mut World, player_oid: i32, pledge_type: i32) {
    if pledge_type != SUBUNIT_ACADEMY {
        return;
    }
    let level = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.level)
        .unwrap_or(0);
    set_academy_level(world, player_oid, level);
}

/// Java `Player.setClan(null)`'s reset of the academy trio — leaving the clan,
/// being ousted, or the clan dissolving all clear it. Also drops the mentorship
/// link, which cannot outlive the shared clan.
pub(crate) fn on_leave_clan(world: &mut World, player_oid: i32) {
    let (was_academy, apprentice, sponsor) = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| (p.lvl_joined_academy > 0, p.apprentice, p.sponsor))
        .unwrap_or((false, 0, 0));
    if was_academy {
        set_academy_level(world, player_oid, 0);
    }
    if apprentice != 0 || sponsor != 0 {
        clear_mentorship(world, player_oid, apprentice, sponsor);
    }
}

/// Write `lvl_joined_academy` in memory and to the row it gates.
fn set_academy_level(world: &mut World, player_oid: i32, level: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.lvl_joined_academy = level;
    }
    let _ = world.db.send(DbCommand::UpdateCharAcademyLevel {
        char_id: player_oid,
        lvl_joined_academy: level,
    });
}

/// Java `Player.setClassId`'s academy block, run **before** the class actually
/// changes: pay the clan, oust the graduate, and hand over the circlet.
///
/// The caller has already established that the new class is in
/// `THIRD_CLASS_GROUP` (Java's name for the *2nd-transfer* classes — the third
/// group counting the base class as the first). Returns `true` when a
/// graduation happened, which is only interesting to tests.
pub(crate) fn graduate(world: &mut World, player_oid: i32) -> bool {
    let (clan_id, joined_at, name) = match world.objects.get_component::<Player>(&player_oid) {
        Some(p) => (p.clan_id, p.lvl_joined_academy, p.name.clone()),
        None => return false,
    };
    if joined_at == 0 || clan_id == 0 || !world.clans.contains_key(&clan_id) {
        return false;
    }

    // The sliding reward: max at ≤16, min at ≥39, `max - (joined - 16) * 20`
    // between. Java's own bracketing, kept as three arms rather than a clamp —
    // the middle arm is not `max`-anchored at 16 by accident, it *is* the
    // formula, and folding the ends into it changes the ≥39 value.
    let cfg = &world.cfg.feature;
    let points = if joined_at <= 16 {
        cfg.complete_academy_max_points
    } else if joined_at >= 39 {
        cfg.complete_academy_min_points
    } else {
        cfg.complete_academy_max_points - (joined_at - 16) * 20
    };
    super::clans::add_clan_reputation(world, clan_id, points);

    set_academy_level(world, player_oid, 0);

    // "Clan member $s1 has been expelled." + the roster delete, to the clan.
    let expelled = server_packets::system_message_with(
        sm_ids::CLAN_MEMBER_S1_HAS_BEEN_EXPELLED,
        &[SmParam::Text(name.clone())],
    );
    super::clans::broadcast_to_clan(world, clan_id, &expelled);
    super::clans::broadcast_to_clan(
        world,
        clan_id,
        &server_packets::pledge_show_member_list_delete(&name),
    );

    // `removeClanMember(objectId, 0)` — **expiry 0**: a graduate may join a new
    // clan immediately, which is the reward's other half.
    super::clans::remove_clan_member_for_academy(world, clan_id, player_oid);

    super::clans::send_sm_with(
        world,
        player_oid,
        sm_ids::CONGRATULATIONS_YOU_WILL_NOW_GRADUATE_FROM_THE_CLAN_ACADEMY,
        &[],
    );
    // The graduation gift.
    let _ = super::items::add_inventory_item(world, player_oid, ACADEMY_CIRCLET, 1);

    info!("GameLoop: '{name}' graduated from clan {clan_id}'s academy (+{points} rep).");
    true
}

// ---------------------------------------------------------------------------
// Apprentice / sponsor (`RequestPledgeSetAcademyMaster`)
// ---------------------------------------------------------------------------

/// Both ends of the mentorship, written to memory and straight to the rows.
fn set_mentorship(world: &mut World, char_id: i32, apprentice: i32, sponsor: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&char_id) {
        p.apprentice = apprentice;
        p.sponsor = sponsor;
    }
    // Mirror onto the clan roster, so an offline member's pane still shows
    // the pair (Java keeps it on `ClanMember` alongside the live player).
    for clan in world.clans.values_mut() {
        if let Some(m) = clan.members.iter_mut().find(|m| m.char_id == char_id) {
            m.apprentice = apprentice;
            m.sponsor = sponsor;
        }
    }
    // Java saves "even if online, since both must match".
    let _ = world.db.send(DbCommand::UpdateCharApprenticeSponsor {
        char_id,
        apprentice,
        sponsor,
    });
}

/// Break the link from `player_oid`'s side, and from whichever partner it named.
fn clear_mentorship(world: &mut World, player_oid: i32, apprentice: i32, sponsor: i32) {
    set_mentorship(world, player_oid, 0, 0);
    for partner in [apprentice, sponsor] {
        if partner != 0 {
            set_mentorship(world, partner, 0, 0);
        }
    }
}

/// Port of `clientpackets/RequestPledgeSetAcademyMaster` (0x?? ex): pair an
/// academy member with a sponsor, or break the pair.
///
/// The packet names the two players by *name* and does not say which is which —
/// Java decides by looking at whose pledge type is the academy, so a client that
/// sends them in either order works. `set` is 1 to pair, 0 to unpair.
pub(crate) fn handle_set_academy_master(world: &mut World, client_id: u32, body: &[u8]) {
    use commons::network::PacketReader;

    let Some(player_oid) = world.player_oid(client_id) else {
        return;
    };
    let mut r = PacketReader::new(body);
    let (Some(set), Some(current_name), Some(target_name)) =
        (r.read_i32(), r.read_string(), r.read_string())
    else {
        return;
    };
    let Some(clan_id) = clan_of(world, player_oid) else {
        return;
    };
    // `ClanPrivilege.CL_APPRENTICE`.
    if !super::clans::has_clan_privilege(world, player_oid, crate::model::clan::CL_APPRENTICE) {
        super::clans::send_sm_with(
            world,
            player_oid,
            sm_ids::YOU_DO_NOT_HAVE_THE_RIGHT_TO_DISMISS_AN_APPRENTICE,
            &[],
        );
        return;
    }
    let find = |world: &World, name: &str| -> Option<(i32, i32)> {
        world.clans.get(&clan_id).and_then(|c| {
            c.members
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(name))
                .map(|m| (m.char_id, m.pledge_type))
        })
    };
    let (Some(current), Some(target)) = (find(world, &current_name), find(world, &target_name))
    else {
        return; // Java: either member missing → silent return.
    };
    // Whichever of the two sits in the academy is the apprentice.
    let (apprentice, sponsor) = if current.1 == SUBUNIT_ACADEMY {
        (current.0, target.0)
    } else {
        (target.0, current.0)
    };
    let (apprentice_name, sponsor_name) = if apprentice == current.0 {
        (current_name.clone(), target_name.clone())
    } else {
        (target_name.clone(), current_name.clone())
    };

    let links_of = |world: &World, id: i32| -> (i32, i32) {
        world
            .objects
            .get_component::<Player>(&id)
            .map(|p| (p.apprentice, p.sponsor))
            .unwrap_or((0, 0))
    };

    let message = if set == 0 {
        clear_mentorship(world, apprentice, links_of(world, apprentice).0, sponsor);
        set_mentorship(world, sponsor, 0, 0);
        server_packets::system_message_with(
            sm_ids::S2_CLAN_MEMBER_C1_S_APPRENTICE_HAS_BEEN_REMOVED,
            &[
                SmParam::Text(sponsor_name.clone()),
                SmParam::Text(apprentice_name.clone()),
            ],
        )
    } else {
        // Java refuses if either side already has *any* link — an apprentice
        // with a sponsor, a sponsor with an apprentice, and (its own words)
        // the crossed cases too.
        let (a_app, a_spon) = links_of(world, apprentice);
        let (s_app, s_spon) = links_of(world, sponsor);
        if a_spon != 0 || s_app != 0 || a_app != 0 || s_spon != 0 {
            // Java has no retail message here and sends plain text.
            super::clans::send_to_member(
                world,
                player_oid,
                server_packets::system_message_with(
                    sm_ids::S1_TEXT,
                    &[SmParam::Text("Remove previous connections first.".into())],
                ),
            );
            return;
        }
        set_mentorship(world, apprentice, 0, sponsor);
        set_mentorship(world, sponsor, apprentice, 0);
        server_packets::system_message_with(
            sm_ids::S2_HAS_BEEN_DESIGNATED_AS_THE_APPRENTICE_OF_CLAN_MEMBER_S1,
            &[
                SmParam::Text(sponsor_name.clone()),
                SmParam::Text(apprentice_name.clone()),
            ],
        )
    };

    // Java tells the actor only when they are neither party.
    if player_oid != sponsor && player_oid != apprentice {
        super::clans::send_to_member(world, player_oid, message.clone());
    }
    super::clans::send_to_member(world, sponsor, message.clone());
    super::clans::send_to_member(world, apprentice, message);
}

/// Java `EnterWorld.notifySponsorOrApprentice`: tell the partner you're on.
/// Note the `else if` — a character with both links (which the pairing rules
/// forbid) would only notify the sponsor, and the port keeps that shape.
pub(crate) fn notify_partner_on_login(world: &mut World, player_oid: i32) {
    let (apprentice, sponsor, name) = match world.objects.get_component::<Player>(&player_oid) {
        Some(p) => (p.apprentice, p.sponsor, p.name.clone()),
        None => return,
    };
    if sponsor != 0 {
        let msg = server_packets::system_message_with(
            sm_ids::YOUR_APPRENTICE_S1_HAS_LOGGED_IN,
            &[SmParam::Text(name)],
        );
        super::clans::send_to_member(world, sponsor, msg);
    } else if apprentice != 0 {
        let msg = server_packets::system_message_with(
            sm_ids::YOUR_SPONSOR_C1_HAS_LOGGED_IN,
            &[SmParam::Text(name)],
        );
        super::clans::send_to_member(world, apprentice, msg);
    }
}

/// Java `ClanMember.getApprenticeOrSponsorName()` — the name shown in the clan
/// window's member pane: this member's **apprentice** if they sponsor one,
/// otherwise their **sponsor**, otherwise empty. Java reads the live player
/// first when one is online, which is what keeps a just-made pairing visible
/// before any reload.
pub(crate) fn partner_name(world: &World, clan_id: i32, char_id: i32) -> String {
    let (apprentice, sponsor) = match world.objects.get_component::<Player>(&char_id) {
        Some(p) => (p.apprentice, p.sponsor),
        // Offline: the roster carries the pair (loaded with the clan and kept
        // in step by `set_mentorship`), exactly like Java's `ClanMember`.
        None => world
            .clans
            .get(&clan_id)
            .and_then(|c| c.members.iter().find(|m| m.char_id == char_id))
            .map_or((0, 0), |m| (m.apprentice, m.sponsor)),
    };
    let partner = if apprentice != 0 { apprentice } else { sponsor };
    if partner == 0 {
        return String::new();
    }
    world
        .clans
        .get(&clan_id)
        .and_then(|c| c.members.iter().find(|m| m.char_id == partner))
        .map(|m| m.name.clone())
        // Java literally returns the string "Error" when the id names nobody
        // in the clan; kept, since it is what a GM would see and chase.
        .unwrap_or_else(|| "Error".to_string())
}
