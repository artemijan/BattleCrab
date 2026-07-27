//! GM petitions (G31) — port of Java `RequestPetition`/`RequestPetitionCancel`/
//! `RequestPetitionFeedback` + `PetitionManager` + `AdminPetition`. Petitions
//! are in-memory (`World.petitions`); only the post-consultation feedback
//! persists. The `PetitionManager` holds state; the notice/HTML orchestration
//! lives here, where the client sessions are reachable.

use crate::db::DbCommand;
use crate::enums::ChatType;
use crate::model::petition::{PetitionState, PetitionType};
use crate::model::Player;
use crate::network::server_packets::{self as sp, sm_ids, SmParam};
use crate::session::ClientSession;
use crate::world::World;
use commons::network::PacketReader;

use super::helpers::client_for_player;

/// Java `Petition`'s 255-char content cap.
const MAX_CONTENT_LEN: usize = 255;

// --- small send helpers -----------------------------------------------------

fn send_to(world: &World, object_id: i32, packet: Vec<u8>) {
    if let Some(cs) = client_for_player(world, object_id).and_then(|cid| world.clients.get(&cid)) {
        cs.send(packet);
    }
}

fn send_sm(world: &World, object_id: i32, message_id: i16, params: &[SmParam]) {
    send_to(
        world,
        object_id,
        sp::system_message_with(message_id, params),
    );
}

fn player_name(world: &World, object_id: i32) -> String {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

fn is_online(world: &World, object_id: i32) -> bool {
    client_for_player(world, object_id).is_some()
}

fn is_gm(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_gm(&world.data))
}

/// Java `AdminData.isGmOnline` — at least one GM is connected.
fn any_gm_online(world: &World) -> bool {
    world.clients.values().any(|cs| match cs {
        ClientSession::InGame(s) => is_gm(world, s.player_object_id()),
        _ => false,
    })
}

/// Java `AdminData.broadcastToGMs(new CreatureSay(...))` — a `HeroVoice` line to
/// every online GM, authored by `author`.
fn broadcast_to_gms(world: &World, author: &str, text: &str) {
    let say = sp::creature_say(0, ChatType::HeroVoice, author, text, None);
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            if is_gm(world, s.player_object_id()) {
                cs.send(say.clone());
            }
        }
    }
}

// --- client packets ---------------------------------------------------------

/// `RequestPetition` (0x89): a player files a petition (content + type 1-9).
pub(crate) fn on_request_petition(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return;
    };
    let sender = s.player_object_id();
    let mut r = PacketReader::new(body);
    let Some(content) = r.read_string() else {
        return;
    };
    let Some(type_id) = r.read_i32() else {
        return;
    };
    let Some(ptype) = PetitionType::from_wire(type_id) else {
        return; // Java: type <= 0 || type >= 10 → drop
    };

    if !any_gm_online(world) {
        send_sm(
            world,
            sender,
            sm_ids::THERE_ARE_NO_GMS_CURRENTLY_VISIBLE,
            &[],
        );
        return;
    }
    if !world.cfg.character.petitioning_allowed {
        send_sm(
            world,
            sender,
            sm_ids::THE_GAME_CLIENT_ENCOUNTERED_AN_ERROR_PETITION_SERVER,
            &[],
        );
        return;
    }
    if world.petitions.is_player_petition_pending(sender) {
        send_sm(
            world,
            sender,
            sm_ids::YOU_MAY_ONLY_SUBMIT_ONE_PETITION_ACTIVE_AT_A_TIME,
            &[],
        );
        return;
    }
    if world.petitions.pending_count() >= world.cfg.character.max_petitions_pending as usize {
        send_sm(
            world,
            sender,
            sm_ids::THE_PETITION_SERVICE_IS_CURRENTLY_UNAVAILABLE,
            &[],
        );
        return;
    }
    let total = world.petitions.player_total_petition_count(sender) as i32 + 1;
    if total > world.cfg.character.max_petitions_per_player {
        send_sm(
            world,
            sender,
            sm_ids::WE_HAVE_RECEIVED_S1_PETITIONS_FROM_YOU_TODAY_MAXIMUM,
            &[SmParam::Int(total)],
        );
        return;
    }
    if content.chars().count() > MAX_CONTENT_LEN {
        return; // Java answers a "800 chars" SM; the client caps input already.
    }

    let name = player_name(world, sender);
    let id = world.petitions.submit(sender, name.clone(), content, ptype);
    send_sm(
        world,
        sender,
        sm_ids::YOUR_PETITION_APPLICATION_HAS_BEEN_ACCEPTED_RECEIPT_NO_IS_S1,
        &[SmParam::Int(id)],
    );
    send_sm(
        world,
        sender,
        sm_ids::YOU_HAVE_SUBMITTED_S1_PETITIONS_YOU_MAY_SUBMIT_S2_MORE_TODAY,
        &[
            SmParam::Int(total),
            SmParam::Int(world.cfg.character.max_petitions_per_player - total),
        ],
    );
    send_sm(
        world,
        sender,
        sm_ids::THERE_ARE_S1_PETITIONS_CURRENTLY_ON_THE_WAITING_LIST,
        &[SmParam::Int(world.petitions.pending_count() as i32)],
    );
    broadcast_to_gms(
        world,
        "Petition System",
        &format!("{name} has submitted a new petition."),
    );
}

/// `RequestPetitionCancel` (0x8A): a petitioner cancels a pending petition, or a
/// GM ends an active consultation.
pub(crate) fn on_request_petition_cancel(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return;
    };
    let sender = s.player_object_id();

    if world.petitions.is_player_in_consultation(sender) {
        if is_gm(world, sender) {
            // GM ends the consultation they're handling (COMPLETED).
            if let Some(id) = world.petitions.active_id_of(sender) {
                end_consultation(world, id, PetitionState::Completed);
            }
        } else {
            send_sm(world, sender, sm_ids::YOUR_PETITION_IS_BEING_PROCESSED, &[]);
        }
    } else if world.petitions.is_player_petition_pending(sender) {
        if cancel_pending(world, sender) {
            let remaining = world.cfg.character.max_petitions_per_player
                - world.petitions.player_total_petition_count(sender) as i32;
            send_sm(
                world,
                sender,
                sm_ids::THE_PETITION_WAS_CANCELED_YOU_MAY_SUBMIT_S1_MORE_TODAY,
                &[SmParam::Text(remaining.to_string())],
            );
            let name = player_name(world, sender);
            broadcast_to_gms(
                world,
                "Petition System",
                &format!("{name} has canceled a pending petition."),
            );
        } else {
            send_sm(world, sender, sm_ids::FAILED_TO_CANCEL_PETITION, &[]);
        }
    } else {
        send_sm(
            world,
            sender,
            sm_ids::YOU_HAVE_NOT_SUBMITTED_A_PETITION,
            &[],
        );
    }
}

/// `RequestPetitionFeedback` (0xC9): the petitioner's post-consultation rating —
/// the only petition state that persists (`petition_feedback`).
pub(crate) fn on_request_petition_feedback(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return;
    };
    let sender = s.player_object_id();
    let mut r = PacketReader::new(body);
    let _unknown = r.read_i32();
    let Some(rate) = r.read_i32() else { return };
    let message = r.read_string().unwrap_or_default();
    if !(0..=4).contains(&rate) {
        return;
    }
    let Some((char_name, gm_name)) = world
        .objects
        .get_component::<Player>(&sender)
        .and_then(|p| {
            p.last_petition_gm_name
                .clone()
                .map(|gm| (p.name.clone(), gm))
        })
    else {
        return; // Java: no lastPetitionGmName → drop
    };
    let _ = world.db.send(DbCommand::StorePetitionFeedback {
        char_name,
        gm_name,
        rate,
        message,
        date: commons::util::now_millis(),
    });
}

// --- petition chat (Say2 PETITION_PLAYER / PETITION_GM) ----------------------

/// Java `PetitionManager.sendActivePetitionMessage`: route a consultation chat
/// line to both participants and append it to the transcript. Returns whether
/// the speaker was a participant in some pending petition.
pub(crate) fn send_active_petition_message(world: &mut World, speaker: i32, text: &str) -> bool {
    let name = player_name(world, speaker);
    // Find the petition the speaker participates in and its role.
    let Some((id, as_petitioner)) = world.petitions.pending.values().find_map(|p| {
        if p.petitioner == speaker {
            Some((p.id, true))
        } else if p.responder == Some(speaker) {
            Some((p.id, false))
        } else {
            None
        }
    }) else {
        return false;
    };
    let chat_type = if as_petitioner {
        ChatType::PetitionPlayer
    } else {
        ChatType::PetitionGm
    };
    let cs = sp::creature_say(speaker, chat_type, &name, text, None);
    let (petitioner, responder) = {
        let p = world.petitions.pending.get_mut(&id).expect("found above");
        p.log.push(cs.clone());
        (p.petitioner, p.responder)
    };
    send_to(world, petitioner, cs.clone());
    if let Some(r) = responder {
        send_to(world, r, cs);
    }
    true
}

// --- GM actions (Java `AdminPetition` + the manager) ------------------------

/// Java `AdminPetition` accept path + `PetitionManager.acceptPetition`: a GM
/// takes a pending petition into consultation.
pub(crate) fn accept_petition(world: &mut World, gm: i32, id: i32) {
    if world.petitions.is_player_in_consultation(gm) {
        send_sm(
            world,
            gm,
            sm_ids::YOU_MAY_ONLY_SUBMIT_ONE_PETITION_ACTIVE_AT_A_TIME,
            &[],
        );
        return;
    }
    // Already under consultation (has a responder)?
    let taken = world
        .petitions
        .pending
        .get(&id)
        .map(|p| p.state == PetitionState::InProcess || p.responder.is_some());
    match taken {
        None => {
            send_sm(world, gm, sm_ids::NOT_UNDER_PETITION_CONSULTATION, &[]);
            return;
        }
        Some(true) => {
            send_sm(world, gm, sm_ids::YOUR_PETITION_IS_BEING_PROCESSED, &[]);
            return;
        }
        Some(false) => {}
    }

    let gm_name = player_name(world, gm);
    let petitioner = {
        let p = world.petitions.pending.get_mut(&id).expect("checked above");
        p.responder = Some(gm);
        p.responder_name = Some(gm_name.clone());
        p.state = PetitionState::InProcess;
        p.petitioner
    };
    // Petitioner: application accepted. Responder: receipt + consultation start.
    send_sm(
        world,
        petitioner,
        sm_ids::PETITION_APPLICATION_ACCEPTED,
        &[],
    );
    send_sm(
        world,
        gm,
        sm_ids::YOUR_PETITION_APPLICATION_HAS_BEEN_ACCEPTED_RECEIPT_NO_IS_S1,
        &[SmParam::Int(id)],
    );
    let petitioner_name = player_name(world, petitioner);
    send_sm(
        world,
        gm,
        sm_ids::STARTING_PETITION_CONSULTATION_WITH_C1,
        &[SmParam::Text(petitioner_name)],
    );
    if let Some(p) = world.objects.get_component_mut::<Player>(&petitioner) {
        p.last_petition_gm_name = Some(gm_name);
    }
}

/// Java `AdminPetition` reject path + `PetitionManager.rejectPetition`.
pub(crate) fn reject_petition(world: &mut World, gm: i32, id: i32) {
    let ok = world
        .petitions
        .pending
        .get(&id)
        .is_some_and(|p| p.responder.is_none());
    if !ok {
        send_sm(world, gm, sm_ids::FAILED_TO_CANCEL_PETITION, &[]);
    } else {
        let gm_name = player_name(world, gm);
        if let Some(p) = world.petitions.pending.get_mut(&id) {
            p.responder = Some(gm);
            p.responder_name = Some(gm_name);
        }
        end_consultation(world, id, PetitionState::ResponderReject);
    }
    send_pending_list(world, gm);
}

/// Java `AdminPetition` `admin_reset_petitions`: clear the pending queue (unless
/// one is under consultation).
pub(crate) fn reset_petitions(world: &mut World, gm: i32) {
    if world.petitions.any_in_process() {
        send_sm(world, gm, sm_ids::YOUR_PETITION_IS_BEING_PROCESSED, &[]);
        return;
    }
    world.petitions.pending.clear();
    send_pending_list(world, gm);
}

/// Java `PetitionManager.viewPetition`: show the petition's content to the GM.
pub(crate) fn view_petition(world: &World, gm: i32, id: i32) {
    if !is_gm(world, gm) {
        return;
    }
    let Some(p) = world.petitions.pending.get(&id) else {
        return;
    };
    let html = format!(
        "<html><body><center>Petition #{id}</center><br>\
         Petitioner: <font color=\"LEVEL\">{name}</font><br>\
         Type: {ptype}<br><br>{content}<br><br>\
         <center>\
         <button value=\"Accept\" action=\"bypass -h admin_accept_petition {id}\" width=80 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">\
         <button value=\"Reject\" action=\"bypass -h admin_reject_petition {id}\" width=80 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">\
         <button value=\"Back\" action=\"bypass -h admin_view_petitions\" width=80 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">\
         </center></body></html>",
        name = p.petitioner_name,
        ptype = p.ptype.as_label(),
        content = p.content,
    );
    send_to(world, gm, sp::npc_html_message(0, &html));
}

/// Java `PetitionManager.sendPendingPetitionList`: the GM's pending-petition
/// menu. Trimmed of Java's table styling; the actions are the same.
pub(crate) fn send_pending_list(world: &World, gm: i32) {
    let mut body = String::from(
        "<html><body><center>Petition Menu</center><br>\
         <button value=\"Reset\" action=\"bypass -h admin_reset_petitions\" width=80 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">\
         <button value=\"Refresh\" action=\"bypass -h admin_view_petitions\" width=80 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><br>",
    );
    if world.petitions.pending.is_empty() {
        body.push_str("There are no currently pending petitions.");
    } else {
        body.push_str("<font color=\"LEVEL\">Current Petitions:</font><br>");
        let mut ids: Vec<i32> = world.petitions.pending.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let p = &world.petitions.pending[&id];
            if p.state == PetitionState::InProcess {
                body.push_str(&format!(
                    "#{id} {} — in process ({})<br>",
                    p.petitioner_name,
                    p.responder_name.as_deref().unwrap_or("?"),
                ));
            } else {
                body.push_str(&format!(
                    "#{id} {} [{}] <a action=\"bypass -h admin_view_petition {id}\">View</a> \
                     <a action=\"bypass -h admin_reject_petition {id}\">Reject</a><br>",
                    p.petitioner_name,
                    p.ptype.as_label(),
                ));
            }
        }
    }
    body.push_str("</body></html>");
    send_to(world, gm, sp::npc_html_message(0, &body));
}

// --- shared internals -------------------------------------------------------

/// Java `PetitionManager.cancelActivePetition`: end the pending petition the
/// player owns (or, for a responding GM, is handling) with the matching cancel
/// state. Returns whether one was ended.
fn cancel_pending(world: &mut World, player: i32) -> bool {
    let Some((id, as_petitioner)) = world.petitions.pending.values().find_map(|p| {
        if p.petitioner == player {
            Some((p.id, true))
        } else if p.responder == Some(player) {
            Some((p.id, false))
        } else {
            None
        }
    }) else {
        return false;
    };
    let end_state = if as_petitioner {
        PetitionState::PetitionerCancel
    } else {
        PetitionState::ResponderCancel
    };
    end_consultation(world, id, end_state);
    true
}

/// Java `Petition.endPetitionConsultation`: notify the participants, prompt the
/// petitioner for feedback, and move the petition from pending to completed.
fn end_consultation(world: &mut World, id: i32, end_state: PetitionState) {
    let Some(mut petition) = world.petitions.take_pending(id) else {
        return;
    };
    petition.state = end_state;
    let petitioner = petition.petitioner;
    let petitioner_name = petition.petitioner_name.clone();
    let responder = petition.responder;

    if let Some(r) = responder {
        if is_online(world, r) {
            if end_state == PetitionState::ResponderReject {
                // Java sends the petitioner a plain "rejected" line here.
                send_sm(
                    world,
                    petitioner,
                    sm_ids::S1_TEXT,
                    &[SmParam::Text(
                        "Your petition was rejected. Please try again later.".to_string(),
                    )],
                );
            } else {
                send_sm(
                    world,
                    r,
                    sm_ids::PETITION_CONSULTATION_WITH_C1_HAS_ENDED,
                    &[SmParam::Text(petitioner_name)],
                );
                if end_state == PetitionState::PetitionerCancel {
                    send_sm(
                        world,
                        r,
                        sm_ids::RECEIPT_NO_S1_PETITION_CANCELLED,
                        &[SmParam::Int(id)],
                    );
                }
            }
        }
    }

    if is_online(world, petitioner) {
        send_sm(
            world,
            petitioner,
            sm_ids::THIS_ENDS_THE_GM_PETITION_CONSULTATION,
            &[],
        );
        send_to(world, petitioner, sp::petition_vote());
    }

    world.petitions.completed.insert(id, petition);
}
