//! Target selection handlers (`Action`, `RequestTargetCanceld`), the
//! `Player.setTarget` port, and (G8) the `NpcAction` interact path — talking
//! to a targeted NPC opens its chat window.

use crate::game_loop::combat::pvp;
use crate::game_loop::helpers;
use crate::game_loop::net::broadcast;
use crate::game_loop::space::position::maybe_position;
use crate::model::{components, npc};

use crate::game_loop::npc::{npc_template, teleporter, view};

use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::world::World;

use crate::game_loop::skills::cast::abort_cast;
use crate::model;

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
///   control/flame tower, HQ flag, or stationed guard the attacker may engage —
///   *unless* the attacker is itself a wild monster, which sees other monsters
///   as friendly.
pub(crate) fn is_auto_attackable(world: &World, attacker_oid: i32, target_oid: i32) -> bool {
    if world.objects.has_component::<model::Player>(&target_oid) {
        return pvp::is_player_auto_attackable(world, attacker_oid, target_oid);
    }
    if world
        .objects
        .has_component::<model::door::Door>(&target_oid)
    {
        return crate::game_loop::siege::attackable_door(world, target_oid);
    }
    // `Monster.isAutoAttackable`: `if (attacker.isMonster()) return
    // attacker.isFakePlayer();` — for a plain monster attacker that is `false`.
    // Only ever consulted for an NPC attacker, so no player path changes.
    //
    // This became reachable when NPC casts started resolving through the
    // target-type handlers (`npc_cast::resolve_npc_cast_target`): `Enemy.java`
    // asks exactly this question, and without the rule a mob's offensive skill
    // would happily accept a faction-mate the AI handed it. Siege towers, HQ
    // flags and stationed guards keep their own relations below — Java gates
    // those through `Npc.isAutoAttackable`'s clan checks, not through
    // `Monster`.
    let attacker_is_wild_monster = npc_template(world, attacker_oid)
        .is_some_and(|t| t.is_monster())
        // A summon/pet is an NPC here but `isSummon()` in Java, and Java's
        // `Npc.isAutoAttackable` lets summons attack NPCs outright.
        && !world
            .objects
            .has_component::<components::ServitorOf>(&attacker_oid)
        && !world
            .objects
            .has_component::<components::PetOf>(&attacker_oid);

    (!attacker_is_wild_monster
        && npc_template(world, target_oid).is_some_and(|t| t.is_auto_attackable()))
        || crate::game_loop::siege::attackable_siege_tower(world, target_oid)
        || crate::game_loop::siege::attackable_siege_flag(world, target_oid)
        || crate::game_loop::siege::attackable_siege_guard(world, target_oid, attacker_oid)
}

/// `Npc.canInteract(player)`: 3D distance vs `INTERACTION_DISTANCE` between two
/// world objects, plus the seated refusal. Shared by the interact path here and
/// the bypass router (Java re-checks it on every `npc_…` bypass).
pub(crate) fn can_interact(world: &World, player_object_id: i32, npc_object_id: i32) -> bool {
    // `else if (player.isSitting()) return false;` — you talk to nobody from a
    // chair. `combat::start_interact_intent` refuses the walk-over that a
    // failed `canInteract` would otherwise trigger, so the click is inert
    // rather than turning into an approach.
    if crate::game_loop::character::sit_stand::is_resting(world, player_object_id) {
        return false;
    }
    crate::geo::distance::within_3d(world, player_object_id, npc_object_id, INTERACTION_DISTANCE)
}

/// Port of `clientpackets/Action.runImpl`, now resolving both players and NPCs.
/// Java's dispatch: a click on something that isn't your target selects it
/// (`Player.setTarget`); a second click on an NPC target interacts
/// (`NpcAction` — attack for monsters (G9), chat window for the rest).
///
/// `action_id == 1` is a **shift-click**. Java's dispatch is
/// `if (!player.isGM() && (!(obj.isNpc() && ALT_GAME_VIEWNPC) || obj.isFakePlayer()))
/// obj.onAction(player, false); else obj.onActionShift(player);` — so a GM gets
/// the admin `npcinfo.htm` window, `ALT_GAME_VIEWNPC` gets the `NpcViewMod`
/// player view, and everyone else falls back to `onAction` with **interact
/// false**: a plain select. Note what that fallback skips — the entire
/// `else if (interact)` arm, which is where attacking and talking live. A
/// shift-click is therefore never an attack, let alone "an attack that refuses
/// to move" (there is no melee `dontMove` anywhere in Java). Always terminates
/// with `ActionFailed`, matching `WorldObject.onAction`.
///
/// Java `PlayerAction`'s `CURSED_WEAPON_VICTIM_MIN_LEVEL` pair: an attack is
/// refused when either side wields a cursed weapon and the *other* is below
/// level 21. Only applies player-vs-player; a monster has no such protection.
fn cursed_weapon_blocks_attack(world: &World, attacker: i32, target: i32) -> bool {
    /// `PlayerAction.CURSED_WEAPON_VICTIM_MIN_LEVEL`.
    const CURSED_WEAPON_VICTIM_MIN_LEVEL: i32 = 21;

    let Some(a) = world.objects.get_component::<model::Player>(&attacker) else {
        return false;
    };
    let Some(t) = world.objects.get_component::<model::Player>(&target) else {
        return false;
    };
    (t.cursed_weapon_equipped_id != 0 && a.level < CURSED_WEAPON_VICTIM_MIN_LEVEL)
        || (a.cursed_weapon_equipped_id != 0 && t.level < CURSED_WEAPON_VICTIM_MIN_LEVEL)
}

/// Java's `player.getTarget() == this` — the test that turns the *second*
/// click on an already-selected object into the interaction (open the store,
/// talk to the NPC, engage the door) rather than another select.
fn is_targeting(world: &World, object_id: i32, other_object_id: i32) -> bool {
    world
        .objects
        .get_component::<components::TargetRef>(&object_id)
        .copied()
        .unwrap_or_default()
        .0
        == Some(other_object_id)
}

pub(crate) fn handle_action(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::combat::Action::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    let shift = pkt.action_id == 1;

    // `Action.runImpl`: a spectator clicks nothing. The Broadcasting Tower's
    // free-look camera would otherwise let a viewer target and act on whatever
    // it is pointed at.
    if crate::game_loop::space::observation::is_observing(world, object_id) {
        helpers::send_action_failed(world, client_id);
        return;
    }

    // `Npc.canTarget`: `if (player.isLockedTarget() && getLockedTarget() != this)`
    // — a taunted playable may not click *away* from the taunter onto another
    // NPC. Java refuses with "Failed to change enmity" and an ActionFailed,
    // and the check is NPC-side only, so the victim can still click players
    // and items (G34 S4).
    if let Some(locked) = world
        .objects
        .get_component::<components::LockedTarget>(&object_id)
        .map(|l| l.0)
        && locked != pkt.object_id
        && world.objects.has_component::<npc::Npc>(&pkt.object_id)
    {
        helpers::send_sm_and_action_failed(
            world,
            client_id,
            server_packets::sm_ids::FAILED_TO_CHANGE_ENMITY,
            &[],
        );
        return;
    }
    if world
        .objects
        .has_component::<components::GroundItem>(&pkt.object_id)
    {
        // `handlers.actionhandlers.ItemAction`: `if (!player.isFlying())
        // player.getAI().setIntention(AI_INTENTION_PICK_UP, target)`. The click
        // does *not* pick the item up — it starts a walk to it, and
        // `PlayerAI.thinkPickUp` lifts it once inside `maybeMoveToPawn(target,
        // 36)`. A wyvern rider gets nothing at all (no ActionFailed either;
        // the handler returns `true` having done nothing).
        //
        // First, though, `ItemAction` refuses a mercenary posting ticket lying
        // inside a castle's zone (`CastleManager.getCastle` = the siege zone)
        // to anyone who is not in the owning clan with `CS_MERCENARIES` — the
        // hiring system isn't modelled, but a bought ticket dropped on castle
        // grounds must still be protected. No siege-active check, as in Java.
        let ticket_refused = (|| {
            let pos = world
                .objects
                .get_component::<components::Position>(&pkt.object_id)?;
            let castle_id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z)?;
            let item_id = world
                .objects
                .get_component::<components::GroundItem>(&pkt.object_id)
                .map(|g| g.item_id)?;
            if !world
                .data
                .castle_siege_guards
                .is_ticket_of(castle_id, item_id)
            {
                return None;
            }
            let clan_id = world
                .objects
                .get_component::<model::Player>(&object_id)
                .map(|p| p.clan_id)?;
            let owns = clan_id != 0
                && world
                    .clans
                    .get(&clan_id)
                    .is_some_and(|c| c.castle_id == castle_id);
            let privileged = owns
                && crate::game_loop::clans::has_clan_privilege(
                    world,
                    object_id,
                    model::clan::CS_MERCENARIES,
                );
            (!privileged).then_some(())
        })()
        .is_some();
        if ticket_refused {
            helpers::send_sm_and_action_failed(
                world,
                client_id,
                server_packets::sm_ids::YOU_DO_NOT_HAVE_THE_AUTHORITY_TO_CANCEL_MERCENARY_POSITIONING,
                &[],
            );
        } else if !world
            .objects
            .get_component::<model::Player>(&object_id)
            .is_some_and(model::Player::is_flying)
        {
            crate::game_loop::combat::start_pickup_intent(world, object_id, pkt.object_id);
        }
    } else if world
        .objects
        .get_component::<components::AdminFlags>(&pkt.object_id)
        .is_some_and(|f| f.untargetable)
        // Java `Action`: `if ((!obj.isTargetable() || player.isTargetingDisabled())
        // && !canOverrideCond(TARGET_ALL))` — the two effect-driven halves of the
        // same gate the `//settargetable` admin flag already stands in for
        // (G34 S3). `UNTARGETABLE` is on the *clicked* object,
        // `TARGETING_DISABLED` on the *clicker*.
        || crate::game_loop::abnormal::is_untargetable(world, pkt.object_id)
        || crate::game_loop::abnormal::is_targeting_disabled(world, object_id)
    {
        // `//settargetable` off — Java's `isTargetable()` gate in `canTarget`.
        helpers::send_action_failed(world, client_id);
    } else if world.objects.has_component::<model::Player>(&pkt.object_id) {
        // A player running a private store, clicked while already targeted, opens
        // their store window for the customer (Java `Player.onAction`).
        let already_targeted = is_targeting(world, object_id, pkt.object_id);
        // Java `Player.onActionRequest` tests the sell-buff shop *before* the
        // ordinary store, because a buff seller wears the `PACKAGE_SELL` type
        // and would otherwise open an empty package-sale window.
        if already_targeted
            && pkt.object_id != object_id
            && crate::game_loop::commerce::sell_buffs::on_action(
                world,
                client_id,
                object_id,
                pkt.object_id,
            )
        {
            // handled — the buff menu went out
        } else if already_targeted
            && pkt.object_id != object_id
            && crate::game_loop::commerce::private_store::is_store_owner(world, pkt.object_id)
        {
            crate::game_loop::commerce::private_store::open_buyer_view(
                world,
                client_id,
                object_id,
                pkt.object_id,
            );
        } else if already_targeted
            && pkt.object_id != object_id
            && crate::game_loop::commerce::private_store::is_buy_store_owner(world, pkt.object_id)
        {
            // A buy store reads the other way round: the clicker is the seller.
            crate::game_loop::commerce::private_store::open_seller_view(
                world,
                client_id,
                object_id,
                pkt.object_id,
            );
        } else if already_targeted
            && pkt.object_id != object_id
            && crate::game_loop::commerce::crafting::is_manufacture_owner(world, pkt.object_id)
        {
            crate::game_loop::commerce::crafting::open_sell_list(
                world,
                client_id,
                object_id,
                pkt.object_id,
            );
        } else {
            set_target(world, client_id, object_id, Some(pkt.object_id));
        }
    } else if let Some(npc) = world.objects.get_component::<npc::Npc>(&pkt.object_id) {
        // Java `Npc.canTarget` → `WorldObject.isTargetable` (template flag).
        let targetable = npc.template(world).is_none_or(|t| t.targetable);
        if targetable {
            // `NpcAction.action`: every click on an NPC records it as the
            // player's last folk NPC (bare-bypass origin resolution).
            world
                .objects
                .add_components(&object_id, components::LastFolkNpc(pkt.object_id));
            // `Action` case 1 → `Npc.onActionShift` → `NpcActionShift`: a GM
            // always gets the admin `npcinfo.htm` window (whatever
            // `AltGameViewNpc` says), everyone else only the player-facing
            // view, and only when that config is on.
            let is_gm = world
                .objects
                .get_component::<model::Player>(&object_id)
                .is_some_and(|p| p.is_gm(&world.data));
            if shift && is_gm {
                set_target(world, client_id, object_id, Some(pkt.object_id));
                crate::game_loop::admin::npc_info::send_npc_info(
                    world,
                    client_id,
                    object_id,
                    pkt.object_id,
                );
            } else if shift && world.cfg.npc.alt_game_view_npc {
                // `NpcActionShift`: set the target, then open the info window.
                set_target(world, client_id, object_id, Some(pkt.object_id));
                view::send_npc_view(world, client_id, pkt.object_id);
            } else {
                let already_targeted = is_targeting(world, object_id, pkt.object_id);
                // `NpcAction.action(player, target, interact)`: an unselected
                // NPC is selected, and only an **interact** click goes on to
                // attack or talk (`else if (interact)`). A shift-click that
                // misses both `onActionShift` branches lands here through
                // `Action`'s own `obj.onAction(player, false)` — select only,
                // never an attack. So shift is not "an attack that refuses to
                // move"; it is not an attack at all.
                if !already_targeted {
                    set_target(world, client_id, object_id, Some(pkt.object_id));
                } else if !shift {
                    interact_with_npc(world, client_id, object_id, pkt.object_id);
                }
            }
        }
    } else if world
        .objects
        .has_component::<model::door::Door>(&pkt.object_id)
    {
        // `DoorAction.action`: the first click selects the door; a second click
        // (already targeted, non-shift `interact`) engages it when it's auto-
        // attackable — a castle gate during a siege — gated on the 400-unit
        // z-difference Java checks before `AI_INTENTION_ATTACK`.
        let already_targeted = is_targeting(world, object_id, pkt.object_id);
        let z_ok = matches!(
            (
                world.objects.get_component::<components::Position>(&object_id),
                world.objects.get_component::<components::Position>(&pkt.object_id),
            ),
            (Some(a), Some(d)) if (a.z - d.z).abs() < 400
        );
        if already_targeted && !shift && z_ok && is_auto_attackable(world, object_id, pkt.object_id)
        {
            // Java `PlayerAction`: a cursed-weapon wielder and anyone under
            // level 21 are mutually untouchable — the newbie can't attack the
            // demon, and the demon can't farm newbies. Java answers with a bare
            // ActionFailed (sent unconditionally at the end of this handler),
            // so the click is simply swallowed.
            if !cursed_weapon_blocks_attack(world, object_id, pkt.object_id) {
                crate::game_loop::combat::start_attack_intent(
                    world,
                    client_id,
                    object_id,
                    pkt.object_id,
                );
            }
        } else {
            set_target(world, client_id, object_id, Some(pkt.object_id));
        }
    }
    helpers::send_action_failed(world, client_id);
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
    let Some(pkt) = cp::combat::RequestTargetCanceld::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    if matches!(
        world
            .objects
            .get_component::<components::QueuedAction>(&object_id),
        Some(components::QueuedAction::Skill { .. })
    ) {
        world
            .objects
            .remove_component::<components::QueuedAction>(&object_id);
    }
    abort_cast(world, object_id);
    if !pkt.target_lost {
        return;
    }
    if matches!(
        world
            .objects
            .get_component::<components::Intent>(&object_id),
        Some(components::Intent(
            crate::model::PlayerIntent::Attack { .. }
        ))
    ) {
        world
            .objects
            .remove_component::<components::Intent>(&object_id);
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
    if world
        .objects
        .get_component::<model::Player>(&target_id)
        .is_some()
    {
        let pos = world
            .objects
            .get_component::<components::Position>(&target_id)?;
        let vitals = world
            .objects
            .get_component::<components::Vitals>(&target_id)?;
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
    if let Some(door) = world.objects.get_component::<model::door::Door>(&target_id) {
        // Doors validate-location and show an HP bar like NPCs (the siege attack
        // gate lives in the attack path, not here).
        let pos = world
            .objects
            .get_component::<components::Position>(&target_id)?;
        let max_hp = world
            .data
            .door_data
            .get(door.door_id)
            .map(|t| t.hp_max)
            .unwrap_or(1);
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
    let npc = world.objects.get_component::<npc::Npc>(&target_id)?;
    let pos = world
        .objects
        .get_component::<components::Position>(&target_id)?;
    let vitals = world
        .objects
        .get_component::<components::Vitals>(&target_id)?;
    let t = npc.template(world)?;
    Some(TargetInfo {
        z: pos.z,
        max_hp: vitals.max_hp,
        cur_hp: vitals.cur_hp as i32,
        color: if t.is_auto_attackable() {
            (viewer_level - t.level) as i16
        } else {
            0
        },
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
pub(crate) fn set_target(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    new_target: Option<i32>,
) {
    let Some(player) = world.objects.get_component::<model::Player>(&object_id) else {
        return;
    };
    let current = world
        .objects
        .get_component::<components::TargetRef>(&object_id)
        .copied()
        .unwrap_or_default()
        .0;
    if current == new_target {
        return;
    }
    let viewer_level = player.level;

    let Some(ppos) = maybe_position(world, object_id) else {
        return;
    };
    // Prevents /target exploiting: reject targets too far away in Z.
    let new_target = new_target.filter(|&t| {
        target_info(world, viewer_level, t)
            .map(|i| (i.z - ppos.z).abs() <= 1000)
            .unwrap_or(false)
    });
    if current == new_target {
        return;
    }

    let (px, py, pz) = (ppos.x, ppos.y, ppos.z);
    if let Some(t) = new_target {
        let Some(info) = target_info(world, viewer_level, t) else {
            return;
        };
        // Java sends ValidateLocation for any creature target; the
        // player→player path predates it and skips the (cosmetic)
        // correction, so it stays NPC-only here.
        if info.is_npc {
            helpers::send_to_client(
                world,
                client_id,
                server_packets::validate_location(t, info.x, info.y, info.z, info.heading),
            );
        }
        helpers::send_to_client(
            world,
            client_id,
            server_packets::my_target_selected(t, info.color),
        );
        helpers::send_to_client(
            world,
            client_id,
            server_packets::status_update(
                t,
                &[
                    (server_packets::status_update_type::MAX_HP, info.max_hp),
                    (server_packets::status_update_type::CUR_HP, info.cur_hp),
                ],
            ),
        );
        // Populate the target window's buff row if the new target already
        // carries (non-passive) buffs — Java sends this on the next
        // `updateEffectIcons`; we send it up front on select.
        let now = world.tick;
        if let Some(buffs) = world.objects.get_component::<components::Buffs>(&t)
            && buffs.0.iter().any(|b| !b.passive)
        {
            helpers::send_to_client(
                world,
                client_id,
                crate::network::enter_world::ex_abnormal_status_update_from_target(t, buffs, now),
            );
        }
        broadcast::broadcast_to_others(
            world,
            object_id,
            &server_packets::target_selected(object_id, t, px, py, pz),
        );
    } else {
        // Java's clear path uses broadcastPacket(includeSelf=true): the
        // deselecting client must get TargetUnselected too, or its UI keeps
        // the target locked.
        let pkt = server_packets::target_unselected(object_id, px, py, pz);
        helpers::send_to_client(world, client_id, pkt.clone());
        broadcast::broadcast_to_others(world, object_id, &pkt);
    }

    if let Some(t) = world
        .objects
        .get_component_mut::<components::TargetRef>(&object_id)
    {
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
    if !world
        .objects
        .get_component::<components::TargetRef>(&holder_object_id)
        .copied()
        .is_some_and(|t| t.0.is_some())
    {
        return;
    }
    if let Some(t) = world
        .objects
        .get_component_mut::<components::TargetRef>(&holder_object_id)
    {
        t.0 = None;
    }
    let Some(pos) = maybe_position(world, holder_object_id) else {
        return;
    };
    let pkt = server_packets::target_unselected(holder_object_id, pos.x, pos.y, pos.z);
    helpers::send_to_player(world, holder_object_id, pkt.clone());
    broadcast::broadcast_to_others(world, holder_object_id, &pkt);
}

/// Java `World.removeVisibleObject`'s selection sweep: release every player
/// holding `object_id` as their target, each through [`drop_target_notify`] so
/// the holder and its neighbours both see the `TargetUnselected`. The object
/// itself is skipped — Java walks the *other* visible objects, which matters
/// for a self-targeted GM going invisible (`WorldObject.setInvisible`).
///
/// Call this *before* broadcasting the object's `DeleteObject`, per
/// [`drop_target_notify`]'s contract. Shared by corpse decay / `//delete`
/// ([`npc::despawn_npc`]), NPC teleports ([`npc::relocate_npc`])
/// and `//invis`.
pub(crate) fn release_target_holders(world: &mut World, object_id: i32) {
    let mut holders: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&model::Player, &components::TargetRef)>(|(p, t)| {
            if t.0 == Some(object_id) && p.object_id != object_id {
                holders.push(p.object_id);
            }
        });
    for holder_oid in holders {
        drop_target_notify(world, holder_oid);
    }
}

/// The `NpcAction` interact branch (second click on the current NPC target):
/// monsters start the auto-attack loop (G9); everything else in interaction
/// range opens its chat window (`Npc.showChatWindow`). Out of range, the
/// player walks in first (`combat::start_interact_intent`, Java's
/// `AI_INTENTION_INTERACT`) and this function is re-entered on arrival —
/// matching Java's `Player.doInteract` re-dispatching `onAction`.
pub(crate) fn interact_with_npc(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    npc_object_id: i32,
) {
    if world
        .objects
        .get_component::<model::Player>(&object_id)
        .is_none()
    {
        return;
    }
    // `PetAction`/`SummonAction`: the owner interacting with their **own**
    // summon never reaches the NPC talk/attack flow — Java shows the status
    // window and fires `ON_PLAYER_SUMMON_TALK`, whose only listener on this
    // dist is the Sin Eater's grumbling.
    if world
        .objects
        .get_component::<components::ServitorOf>(&npc_object_id)
        .is_some_and(|s| s.owner_object_id == object_id)
    {
        crate::scripts::sin_eater::on_summon_talk(world, npc_object_id);
        return;
    }
    let Some(npc) = world.objects.get_component::<npc::Npc>(&npc_object_id) else {
        return;
    };
    let Some(t) = npc.template(world) else { return };
    // `Defender.onAction`: a siege guard is attacked on click (not talked to)
    // when the clicker is an attacker — same gate as the monster auto-attack.
    if t.is_auto_attackable()
        || crate::game_loop::siege::attackable_siege_guard(world, npc_object_id, object_id)
    {
        let dead = helpers::is_dead(world, object_id);
        if !dead {
            // No dontMove for melee: Java's `onAction` path has no shift
            // to carry (case 1 goes to `onActionShift`), and `AttackRequest`
            // reads its shift byte only to discard it.
            crate::game_loop::combat::start_attack_intent(
                world,
                client_id,
                object_id,
                npc_object_id,
            );
        }
        return;
    }
    if !can_interact(world, object_id, npc_object_id) {
        crate::game_loop::combat::start_interact_intent(world, object_id, npc_object_id);
        return;
    }
    // `Artefact.onAction`: the throne-room Holy Artifact — an attacker touching
    // it during a siege captures the castle.
    if t.type_name == "Artefact" {
        crate::game_loop::siege::try_capture_artifact(world, object_id, npc_object_id);
        return;
    }
    // Everything below hands `world` out mutably, so take what we need off the
    // template first.
    let npc_id = t.id;
    // `NpcAction`: an `ON_NPC_FIRST_TALK` listener replaces the chat window
    // outright. The check sits *before* `showChatWindow` in Java, so it also
    // fires for a non-talkable NPC (where `showChatWindow` would have bailed).
    if crate::game_loop::quests::notify_first_talk(
        world,
        client_id,
        object_id,
        npc_object_id,
        npc_id,
    ) {
        return;
    }
    // `Npc.showChatWindow(player, 0)`.
    show_chat_window(world, client_id, npc_object_id, 0);
}

/// Port of `Npc.showChatWindow(player, value)`: send the NPC dialog page
/// `value` (0 = the NPC's landing page, N = the `<id>-<N>.htm` follow-up that
/// the `Chat N` bypass buttons walk to). Java also gates PK players out of
/// merchant/teleporter/warehouse dialogs via the `-pk.htm` pages
/// (`showPkDenyChatWindow`).
pub(crate) fn show_chat_window(world: &mut World, client_id: u32, npc_object_id: i32, value: i32) {
    let Some(npc) = world.objects.get_component::<npc::Npc>(&npc_object_id) else {
        return;
    };
    let Some(t) = npc.template(world) else { return };
    if !t.talkable {
        return;
    }
    // `showChatWindow`'s reputation gate, before anything else it does. Java
    // writes it as an `if / else if` chain over the config-and-type pairs, but
    // an NPC has exactly one type, so a match over the type is the same thing
    // and says so more plainly.
    let viewer_oid = match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => s.player_object_id(),
        _ => 0,
    };
    let reputation = world
        .objects
        .get_component::<model::Player>(&viewer_oid)
        .map_or(0, |p| p.reputation);
    if reputation < 0 {
        let cfg = &world.cfg.character;
        let denied_dir = match t.type_name.as_str() {
            "Merchant" if !cfg.alt_karma_player_can_shop => Some("merchant"),
            "Teleporter" if !cfg.alt_karma_player_can_use_gk => Some("teleporter"),
            "Warehouse" if !cfg.alt_karma_player_can_use_warehouse => Some("warehouse"),
            "Fisherman" if !cfg.alt_karma_player_can_shop => Some("fisherman"),
            _ => None,
        };
        // `showPkDenyChatWindow` returns false when the page is absent, and
        // Java then falls through to the ordinary dialog — so a criminal is
        // refused only at the NPCs the datapack wrote a refusal for.
        if let Some(dir) = denied_dir
            && let Some(html) = crate::data::htm_cache::read_htm_for_client(
                world,
                client_id,
                format!("{}data/html/{dir}/{}-pk.htm", world.data.root, t.id),
            )
        {
            let html = html.replace("%objectId%", &npc_object_id.to_string());
            helpers::send_to_client(
                world,
                client_id,
                server_packets::npc_html_message(npc_object_id, &html),
            );
            helpers::send_action_failed(world, client_id);
            return;
        }
    }
    // Java bails on the landing page of an `Auctioneer`, and on the id ranges
    // that belong to NPCs driven entirely by their own script windows.
    if (t.type_name == "Auctioneer" && value == 0)
        || matches!(t.id, 31093..=31094 | 31172..=31201 | 31239..=31254)
    {
        return;
    }
    // `Teleporter.showChatWindow`: a gatekeeper standing on castle ground has
    // three landing pages — the owner clan's, the "busy" page while that
    // castle's siege runs, and the "no" page for everyone else.
    if t.type_name == "Teleporter"
        && value == 0
        && let Some(file) = teleporter::castle_landing_page(world, npc_object_id, viewer_oid)
    {
        teleporter::send_teleporter_html(world, client_id, npc_object_id, &file);
        return;
    }
    let html = load_chat_window_html(world, client_id, &t.type_name, t.id, value)
        .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
        .replace("%objectId%", &npc_object_id.to_string())
        .replace("%npcname%", &t.name);
    helpers::send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_object_id, &html),
    );
}

/// `getHtmlPath` across the instance classes this slice can meet: each
/// subclass roots its dialogs in its own `data/html/<dir>/` (no fallback —
/// Java shows the "text is missing" stub); plain `Folk`/`Npc` use
/// `data/html/default/` falling back to `npcdefault.htm`. Page `value` picks
/// `<id>.htm` (0) or `<id>-<value>.htm`. Java streams these through
/// `HtmCache`; this port reads per interaction and applies the same
/// normalization via [`read_htm`] — a deliberate choice with identical output,
/// documented in [`crate::data::htm_cache`], not a deferral.
fn load_chat_window_html(
    world: &World,
    client_id: u32,
    type_name: &str,
    npc_id: i32,
    value: i32,
) -> Option<String> {
    let root = &world.data.root;
    let pom = if value == 0 {
        npc_id.to_string()
    } else {
        format!("{npc_id}-{value}")
    };
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
    let read = |p: String| crate::data::htm_cache::read_htm_for_client(world, client_id, p);
    match dir {
        Some(dir) => read(format!("{root}data/html/{dir}/{pom}.htm")),
        None => read(format!("{root}data/html/default/{pom}.htm"))
            .or_else(|| read(format!("{root}data/html/npcdefault.htm"))),
    }
}

/// The object id this creature currently has selected, if any.
pub(crate) fn current(world: &World, object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<components::TargetRef>(&object_id)
        .and_then(|t| t.0)
}

/// The current target, but only when it is a player — Java's
/// `target == null || !target.isPlayer()` guard in one call.
pub(crate) fn current_player(world: &World, object_id: i32) -> Option<i32> {
    current(world, object_id).filter(|oid| world.objects.has_component::<model::Player>(oid))
}

/// The current target, but only when it is an NPC.
pub(crate) fn current_npc(world: &World, object_id: i32) -> Option<i32> {
    current(world, object_id).filter(|oid| world.objects.has_component::<npc::Npc>(oid))
}
