//! Target selection handlers (`Action`, `RequestTargetCanceld`), the
//! `Player.setTarget` port, and (G8) the `NpcAction` interact path — talking
//! to a targeted NPC opens its chat window.

use crate::model::components::{Intent, Position, QueuedAction, TargetRef, Vitals};
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::broadcast_to_others;
use super::skills::cast::abort_cast;

/// `Npc.INTERACTION_DISTANCE`.
pub(crate) const INTERACTION_DISTANCE: f64 = 250.0;

/// Java `WorldObject.isAutoAttackable(attacker)` dispatched over the object
/// kinds the port models — the single gate behind the click-to-attack cursor,
/// the melee attack path, and offensive skill targeting. Keeping it in one
/// place stops the melee and cast paths from drifting apart.
///
/// * **Player** → the PvP/karma relation (`Player.isAutoAttackable`).
/// * **Door** → only a castle door during an active siege (`Door.isAuto
///   Attackable`; Interlude ships no always-`isAttackable` doors).
/// * **NPC** → an auto-attackable template (monsters), or a siege
///   control/flame tower, HQ flag, or stationed guard the attacker may engage.
pub(crate) fn is_auto_attackable(world: &World, attacker_oid: i32, target_oid: i32) -> bool {
    if world.objects.has_component::<crate::model::Player>(&target_oid) {
        return super::pvp::is_player_auto_attackable(world, attacker_oid, target_oid);
    }
    if world.objects.has_component::<crate::model::door::Door>(&target_oid) {
        return super::siege::attackable_door(world, target_oid);
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_auto_attackable())
        || super::siege::attackable_siege_tower(world, target_oid)
        || super::siege::attackable_siege_flag(world, target_oid)
        || super::siege::attackable_siege_guard(world, target_oid, attacker_oid)
}

/// `Npc.canInteract(player)`: plain 3D distance vs `INTERACTION_DISTANCE`
/// between two world objects. Shared by the interact path here and the
/// bypass router (Java re-checks it on every `npc_…` bypass).
pub(crate) fn can_interact(world: &World, player_object_id: i32, npc_object_id: i32) -> bool {
    let (Some(ppos), Some(npos)) = (
        world.objects.get_component::<Position>(&player_object_id),
        world.objects.get_component::<Position>(&npc_object_id),
    ) else {
        return false;
    };
    let (dx, dy, dz) = ((npos.x - ppos.x) as f64, (npos.y - ppos.y) as f64, (npos.z - ppos.z) as f64);
    dx * dx + dy * dy + dz * dz <= INTERACTION_DISTANCE * INTERACTION_DISTANCE
}

/// Port of `clientpackets/Action.runImpl`, now resolving both players and NPCs.
/// Java's dispatch: a click on something that isn't your target selects it
/// (`Player.setTarget`); a second click on an NPC target interacts
/// (`NpcAction` — attack for monsters (G9), chat window for the rest).
///
/// `action_id == 1` is a **shift-click**. Java's `Action` case 1 routes it to
/// `onActionShift` (info) only for a GM, or for a real NPC when
/// `ALT_GAME_VIEWNPC` is set; otherwise it degrades to a plain select
/// (`onAction(player, false)` — target, no interact). We have no GM state on
/// the live player yet, so we take the `ALT_GAME_VIEWNPC` branch: shift-click
/// an NPC → the `NpcViewMod` info window (`npc_view::send_npc_view`), else a
/// plain select. Always terminates with `ActionFailed`, matching
/// `WorldObject.onAction`.
pub(crate) fn handle_action(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::Action::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    let shift = pkt.action_id == 1;

    if world.objects.has_component::<crate::model::components::GroundItem>(&pkt.object_id) {
        // `Item.onAction` → `Player.doPickupItem`: pick it straight up (the
        // walk-to-item approach path is a simplification).
        super::ground_items::pickup_ground_item(world, client_id, object_id, pkt.object_id);
    } else if world.objects.has_component::<crate::model::Player>(&pkt.object_id) {
        // A player running a private store, clicked while already targeted, opens
        // their store window for the customer (Java `Player.onAction`).
        let already_targeted = world.objects.get_component::<TargetRef>(&object_id).copied().unwrap_or_default().0 == Some(pkt.object_id);
        if already_targeted && pkt.object_id != object_id && super::private_store::is_store_owner(world, pkt.object_id) {
            super::private_store::open_buyer_view(world, client_id, object_id, pkt.object_id);
        } else if already_targeted && pkt.object_id != object_id && super::crafting::is_manufacture_owner(world, pkt.object_id) {
            super::crafting::open_sell_list(world, client_id, object_id, pkt.object_id);
        } else {
            set_target(world, client_id, object_id, Some(pkt.object_id));
        }
    } else if let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&pkt.object_id) {
        // Java `Npc.canTarget` → `WorldObject.isTargetable` (template flag).
        let targetable = npc.template(world).is_none_or(|t| t.targetable);
        if targetable {
            // `NpcAction.action`: every click on an NPC records it as the
            // player's last folk NPC (bare-bypass origin resolution).
            world.objects.add_components(&object_id, crate::model::components::LastFolkNpc(pkt.object_id));
            if shift && world.cfg.npc.alt_game_view_npc {
                // `NpcActionShift`: set the target, then open the info window.
                set_target(world, client_id, object_id, Some(pkt.object_id));
                super::npc_view::send_npc_view(world, client_id, pkt.object_id);
            } else {
                let already_targeted = world
                    .objects
                    .get_component::<TargetRef>(&object_id)
                    .copied()
                    .unwrap_or_default()
                    .0
                    == Some(pkt.object_id);
                if already_targeted {
                    interact_with_npc(world, client_id, object_id, pkt.object_id, shift);
                } else {
                    set_target(world, client_id, object_id, Some(pkt.object_id));
                }
            }
        }
    } else if world.objects.has_component::<crate::model::door::Door>(&pkt.object_id) {
        // `DoorAction.action`: the first click selects the door; a second click
        // (already targeted, non-shift `interact`) engages it when it's auto-
        // attackable — a castle gate during a siege — gated on the 400-unit
        // z-difference Java checks before `AI_INTENTION_ATTACK`.
        let already_targeted = world
            .objects
            .get_component::<TargetRef>(&object_id)
            .copied()
            .unwrap_or_default()
            .0
            == Some(pkt.object_id);
        let z_ok = matches!(
            (
                world.objects.get_component::<Position>(&object_id),
                world.objects.get_component::<Position>(&pkt.object_id),
            ),
            (Some(a), Some(d)) if (a.z - d.z).abs() < 400
        );
        if already_targeted && !shift && z_ok && is_auto_attackable(world, object_id, pkt.object_id) {
            super::combat::start_attack_intent(world, client_id, object_id, pkt.object_id, false);
        } else {
            set_target(world, client_id, object_id, Some(pkt.object_id));
        }
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::action_failed());
    }
}

/// Port of `clientpackets/RequestTargetCanceld.runImpl`: clear a queued
/// skill (`setQueuedSkill(null, …)`), abort an in-flight cast (Java
/// `abortAllSkillCasters`, regardless of the `targetLost` flag), then clear
/// the target if `targetLost`. The locked-target/air-ship guards are
/// features that don't exist yet.
///
/// The client sends this packet on a plain target *switch* too, not just
/// Esc — Java's handler never touches the AI intention, so a walk-to-cast
/// must survive it (`thinkCast` drives the intention's snapshotted cast
/// target, not the player's current one). Only the attack loop ends, and
/// only when the target is actually cleared: Java's `thinkAttack` follows
/// the *current* target, which `setTarget(null)` just removed — our `Attack`
/// intent snapshots the target, so drop it explicitly to match.
pub(crate) fn handle_request_target_canceld(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::RequestTargetCanceld::read(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();
    if matches!(
        world.objects.get_component::<QueuedAction>(&object_id),
        Some(QueuedAction::Skill { .. })
    ) {
        world.objects.remove_component::<QueuedAction>(&object_id);
    }
    abort_cast(world, object_id);
    if !pkt.target_lost {
        return;
    }
    if matches!(
        world.objects.get_component::<Intent>(&object_id),
        Some(Intent(crate::model::PlayerIntent::Attack { .. }))
    ) {
        world.objects.remove_component::<Intent>(&object_id);
    }
    set_target(world, client_id, object_id, None);
}

/// What `set_target` needs to know about a prospective target, whichever
/// world registry it lives in.
struct TargetInfo {
    z: i32,
    max_hp: i32,
    cur_hp: i32,
    /// `MyTargetSelected` color: level diff for auto-attackable targets.
    color: i16,
    is_npc: bool,
    heading: i32,
    x: i32,
    y: i32,
}

fn target_info(world: &World, viewer_level: i32, target_id: i32) -> Option<TargetInfo> {
    if world.objects.get_component::<crate::model::Player>(&target_id).is_some() {
        let pos = world.objects.get_component::<Position>(&target_id)?;
        let vitals = world.objects.get_component::<Vitals>(&target_id)?;
        return Some(TargetInfo {
            z: pos.z,
            max_hp: vitals.max_hp,
            cur_hp: vitals.cur_hp as i32,
            color: 0,
            is_npc: false,
            heading: pos.heading,
            x: pos.x,
            y: pos.y,
        });
    }
    if let Some(door) = world.objects.get_component::<crate::model::door::Door>(&target_id) {
        // Doors validate-location and show an HP bar like NPCs (the siege attack
        // gate lives in the attack path, not here).
        let pos = world.objects.get_component::<Position>(&target_id)?;
        let max_hp = world.data.door_data.get(door.door_id).map(|t| t.hp_max).unwrap_or(1);
        return Some(TargetInfo {
            z: pos.z,
            max_hp,
            cur_hp: door.current_hp,
            color: 0,
            is_npc: true,
            heading: pos.heading,
            x: pos.x,
            y: pos.y,
        });
    }
    let npc = world.objects.get_component::<crate::model::npc::Npc>(&target_id)?;
    let pos = world.objects.get_component::<Position>(&target_id)?;
    let vitals = world.objects.get_component::<Vitals>(&target_id)?;
    let t = npc.template(world)?;
    Some(TargetInfo {
        z: pos.z,
        max_hp: vitals.max_hp,
        cur_hp: vitals.cur_hp as i32,
        color: if t.is_auto_attackable() { (viewer_level - t.level) as i16 } else { 0 },
        is_npc: true,
        heading: pos.heading,
        x: pos.x,
        y: pos.y,
    })
}

/// Port of `Player.setTarget`'s core over players and NPCs (no
/// vehicles/party checks yet). Same-target re-click is handled by the caller
/// (`handle_action` routes it to the interact path for NPCs; for players
/// Java only re-sends `ValidateLocation`, which we skip).
pub(crate) fn set_target(world: &mut World, client_id: u32, object_id: i32, new_target: Option<i32>) {
    let Some(player) = world.objects.get_component::<crate::model::Player>(&object_id) else { return };
    let current = world.objects.get_component::<TargetRef>(&object_id).copied().unwrap_or_default().0;
    if current == new_target {
        return;
    }
    let viewer_level = player.level;

    let Some(ppos) = world.objects.get_component::<Position>(&object_id).copied() else { return };
    // Prevents /target exploiting: reject targets too far away in Z.
    let new_target = new_target.filter(|&t| {
        target_info(world, viewer_level, t).map(|i| (i.z - ppos.z).abs() <= 1000).unwrap_or(false)
    });
    if current == new_target {
        return;
    }

    let (px, py, pz) = (ppos.x, ppos.y, ppos.z);
    if let Some(t) = new_target {
        let Some(info) = target_info(world, viewer_level, t) else { return };
        if let Some(cs) = world.clients.get(&client_id) {
            // Java sends ValidateLocation for any creature target; the
            // player→player path predates it and skips the (cosmetic)
            // correction, so it stays NPC-only here.
            if info.is_npc {
                cs.send(server_packets::validate_location(t, info.x, info.y, info.z, info.heading));
            }
            cs.send(server_packets::my_target_selected(t, info.color));
            cs.send(server_packets::status_update(
                t,
                &[
                    (server_packets::status_update_type::MAX_HP, info.max_hp),
                    (server_packets::status_update_type::CUR_HP, info.cur_hp),
                ],
            ));
            // Populate the target window's buff row if the new target already
            // carries (non-passive) buffs — Java sends this on the next
            // `updateEffectIcons`; we send it up front on select.
            let now = world.tick;
            if let Some(buffs) = world.objects.get_component::<crate::model::components::Buffs>(&t) {
                if buffs.0.iter().any(|b| !b.passive) {
                    cs.send(crate::network::enter_world::ex_abnormal_status_update_from_target(t, buffs, now));
                }
            }
        }
        broadcast_to_others(world, object_id, &server_packets::target_selected(object_id, t, px, py, pz));
    } else {
        // Java's clear path uses broadcastPacket(includeSelf=true): the
        // deselecting client must get TargetUnselected too, or its UI keeps
        // the target locked.
        let pkt = server_packets::target_unselected(object_id, px, py, pz);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(pkt.clone());
        }
        broadcast_to_others(world, object_id, &pkt);
    }

    if let Some(t) = world.objects.get_component_mut::<TargetRef>(&object_id) {
        t.0 = new_target;
    }
}

/// Server-initiated `Player.setTarget(null)` (target left the 3×3 visibility
/// block, logged out, …): clear `TargetRef` and broadcast `TargetUnselected`
/// **including the holder's own client** — Java's `broadcastPacket` defaults
/// to includeSelf, and the self-directed copy is load-bearing: our client
/// keeps a deleted object id locked as its selection, so the ground ring
/// re-attaches when the same id comes back via `NpcInfo`/`CharInfo`. Callers
/// must invoke this *before* sending the target's `DeleteObject`, matching
/// Java `World.switchRegion` (`setTarget(null)` runs first).
pub(crate) fn drop_target_notify(world: &mut World, holder_object_id: i32) {
    if !world.objects.get_component::<TargetRef>(&holder_object_id).copied().is_some_and(|t| t.0.is_some()) {
        return;
    }
    if let Some(t) = world.objects.get_component_mut::<TargetRef>(&holder_object_id) {
        t.0 = None;
    }
    let Some(pos) = world.objects.get_component::<Position>(&holder_object_id).copied() else { return };
    let pkt = server_packets::target_unselected(holder_object_id, pos.x, pos.y, pos.z);
    if let Some(cs) = super::helpers::client_for_player(world, holder_object_id).and_then(|cid| world.clients.get(&cid)) {
        cs.send(pkt.clone());
    }
    broadcast_to_others(world, holder_object_id, &pkt);
}

/// The `NpcAction` interact branch (second click on the current NPC target):
/// monsters start the auto-attack loop (G9); everything else in interaction
/// range opens its chat window (`Npc.showChatWindow`). Out of range, the
/// player walks in first (`combat::start_interact_intent`, Java's
/// `AI_INTENTION_INTERACT`) and this function is re-entered on arrival —
/// matching Java's `Player.doInteract` re-dispatching `onAction`.
pub(crate) fn interact_with_npc(world: &mut World, client_id: u32, object_id: i32, npc_object_id: i32, shift: bool) {
    if world.objects.get_component::<crate::model::Player>(&object_id).is_none() {
        return;
    }
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_object_id) else { return };
    let Some(t) = npc.template(world) else { return };
    // `Defender.onAction`: a siege guard is attacked on click (not talked to)
    // when the clicker is an attacker — same gate as the monster auto-attack.
    if t.is_auto_attackable() || super::siege::attackable_siege_guard(world, npc_object_id, object_id) {
        let dead = world.objects.get_component::<Vitals>(&object_id).is_some_and(|v| v.dead);
        if !dead {
            // Shift-click carries the dontMove modifier into the attack.
            super::combat::start_attack_intent(world, client_id, object_id, npc_object_id, shift);
        }
        return;
    }
    if !can_interact(world, object_id, npc_object_id) {
        super::combat::start_interact_intent(world, object_id, npc_object_id);
        return;
    }
    // `Artefact.onAction`: the throne-room Holy Artifact — an attacker touching
    // it during a siege captures the castle.
    if t.type_name == "Artefact" {
        super::siege::try_capture_artifact(world, object_id, npc_object_id);
        return;
    }
    // Everything below hands `world` out mutably, so take what the chat
    // window needs off the template first.
    let (npc_id, type_name, npc_name, talkable) = (t.id, t.type_name.clone(), t.name.clone(), t.talkable);
    // `NpcAction`: an `ON_NPC_FIRST_TALK` listener replaces the chat window
    // outright. The check sits *before* `showChatWindow` in Java, so it also
    // fires for a non-talkable NPC (where `showChatWindow` would have bailed).
    if super::quests::notify_first_talk(world, client_id, object_id, npc_object_id, npc_id) {
        return;
    }
    // `Npc.showChatWindow(player, 0)`.
    if !talkable {
        return;
    }
    let html = load_chat_window_html(&world.data.root, &type_name, npc_id)
        .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
        .replace("%objectId%", &npc_object_id.to_string())
        .replace("%npcname%", &npc_name);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}

/// `getHtmlPath` across the instance classes this slice can meet: each
/// subclass roots its dialogs in its own `data/html/<dir>/` (no fallback —
/// Java shows the "text is missing" stub); plain `Folk`/`Npc` use
/// `data/html/default/` falling back to `npcdefault.htm`. Java streams these
/// through `HtmCache`; a per-interaction disk read is fine at this scale
/// (TODO: cache if profiling ever cares).
fn load_chat_window_html(root: &str, type_name: &str, npc_id: i32) -> Option<String> {
    let dir = match type_name {
        "Merchant" => Some("merchant"),
        "Fisherman" => Some("fisherman"),
        "Teleporter" => Some("teleporter"),
        "Warehouse" => Some("warehouse"),
        "Guard" => Some("guard"),
        "PetManager" => Some("petmanager"),
        t if t.starts_with("VillageMaster") => Some("villagemaster"),
        _ => None,
    };
    match dir {
        Some(dir) => std::fs::read_to_string(format!("{root}data/html/{dir}/{npc_id}.htm")).ok(),
        None => std::fs::read_to_string(format!("{root}data/html/default/{npc_id}.htm"))
            .or_else(|_| std::fs::read_to_string(format!("{root}data/html/npcdefault.htm")))
            .ok(),
    }
}
