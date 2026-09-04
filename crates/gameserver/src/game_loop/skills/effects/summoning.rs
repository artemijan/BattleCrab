//! Summoning-flavored instant effects: NPC/totem summons and the servitor
//! betrayal flip, extracted from the `apply_skill_effects` match.

use super::servitor_owner_of;
use crate::game_loop::character::inventory;
use crate::game_loop::helpers;
use crate::game_loop::helpers::send_sm_to_player as send_sm_with;
use crate::game_loop::net::broadcast;
use crate::game_loop::space::position;
use crate::game_loop::space::position::maybe_position;
use crate::game_loop::space::position::pos_of;
use crate::model::components::stats::Vitals;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::world::World;
/// `SummonNpc.instant` — the `EffectPoint` branch drops the symbol totems
/// (PLAN_G19_SYMBOLS.md); every other template type takes Java's **default**
/// plain-spawn branch (the Holiday Trees and Squash/Watermelon seeds —
/// item-cast carriers, so "learnable" was never the right reachability test
/// here). SKIP(G19): the `Decoy` branch — no reachable skill on this dist
/// summons a template of type `Decoy` (Decoy 525 has no tree row or item
/// grant; item 13769's "Life-size Decoy" 32544 is type `Folk`; verified
/// 2026-08-06).
pub(crate) fn summon_npc(
    world: &mut World,
    target_oid: i32,
    skill: &Skill,
    npc_id: i32,
    npc_count: i32,
    despawn_delay: i32,
) {
    // Java: effected must be a live player (dead/observer gated).
    let effected_alive_player = world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
        && world
            .objects
            .get_component::<Vitals>(&target_oid)
            .is_some_and(|v| !v.dead);
    if !effected_alive_player {
        return;
    }
    // `if (player.isMounted()) return;`
    if world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .is_some_and(|p| p.is_mounted())
    {
        return;
    }
    // GROUND skills spawn at the stored world position; everything else at
    // the effected creature (`SummonNpc.instant`).
    let fallback = pos_of(world, target_oid).unwrap_or((0, 0, 0));
    let (x, y, z) = if skill.target_type == crate::model::skill::target::TargetType::Ground {
        world
            .objects
            .get_component::<crate::model::components::space::GroundSkillTarget>(&target_oid)
            .map(|g| (g.x, g.y, g.z))
            .unwrap_or(fallback)
    } else {
        fallback
    };
    let is_effect_point = world
        .data
        .npc_data
        .get(npc_id)
        .is_some_and(|t| t.type_name == "EffectPoint");
    for _ in 0..npc_count.max(1) {
        if is_effect_point {
            crate::game_loop::skills::effect_point::spawn_effect_point(
                world,
                target_oid,
                npc_id,
                x,
                y,
                z,
                despawn_delay,
            );
        } else {
            crate::game_loop::skills::effect_point::spawn_plain_summon(
                world,
                target_oid,
                npc_id,
                x,
                y,
                z,
                despawn_delay,
            );
        }
    }
}

/// `Betray.onStart` — the servitor turns on its owner. `canStart` requires a
/// player effector and a summon effected, so this is aimed at somebody
/// *else's* pet. The `BETRAYED` flag (which stops it obeying and makes it
/// auto-attackable) rides the landed buff; what happens here is the AI flip.
pub(crate) fn betray(world: &mut World, caster_oid: i32, target_oid: i32) {
    let Some(owner) = servitor_owner_of(world, target_oid) else {
        return; // not a summon — Java's `canStart` refuses
    };
    if !world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    // `getAI().setIntention(ATTACK, getActingPlayer())` — the servitor's own
    // owner becomes its target. Routed through the ordinary attack order so it
    // stops following, takes the top hate slot and arms the attack timeout
    // exactly like a commanded attack would.
    crate::game_loop::servitor::servitor_attack(world, owner, owner);
}

/// `CallParty.instant` — Chant of Gate (1429).
///
/// Every *other* party member is pulled to the caster. There is deliberately no
/// `ConfirmDlg` here: unlike Summon Friend, Java calls `teleToLocation`
/// outright, so a party member gets no say in it.
///
/// Each member is gated by [`check_summon_target_status`], whose refusals are
/// **messaged to the caster**, not the member.
pub(crate) fn call_party(world: &mut World, caster_oid: i32) {
    let Some(members) = crate::game_loop::party::party_members(world, caster_oid) else {
        // `if (party == null) return` — solo, the cast is simply wasted.
        return;
    };
    let Some(dest) = maybe_position(world, caster_oid) else {
        return;
    };

    for member in members {
        // `effector != partyMember` — the caster is not recalled to itself.
        if member == caster_oid {
            continue;
        }
        if let Some((sm, params)) = check_summon_target_status(world, member) {
            send_sm_with(world, caster_oid, sm, &params);
            continue;
        }
        crate::game_loop::death::teleport_player(world, member, dest.x, dest.y, dest.z);
    }
}

/// `CallPc.checkSummonTargetStatus` — the shared gate every recall effect runs
/// over each candidate. `Some((message, params))` is a refusal; the message is
/// sent to the **caster**, never to the person who failed the check.
///
/// Java's order is load-bearing, because several of these states co-occur: a
/// player rooted *in* the olympiad reads the combat line, not the olympiad one.
/// The branches are kept in that order and not folded together for exactly
/// that reason.
///
/// The two "in an area which blocks" strings (1895 and 1908) carry identical
/// text but different ids, and Java feeds them `addString` rather than
/// `addPcName` — a plain string parameter, not the pc-name one the first three
/// branches use. That is a real wire difference, so it is reproduced.
///
/// Not ported: `isInTraingCamp` (Training Camp is off-chronicle here) and the
/// instance `isPlayerSummonAllowed` permission (instances exist, but the flag
/// is not on the template). Both are noted at the branch they belong to.
pub(crate) fn check_summon_target_status(
    world: &World,
    member: i32,
) -> Option<(i16, Vec<server_packets::SmParam>)> {
    use server_packets::{SmParam, sm_ids};

    let name = helpers::player_name_or_empty(world, member);
    let pc = || vec![SmParam::PlayerName(name.clone())];
    // `addString`, not `addPcName` — see the doc comment.
    let text = || vec![SmParam::Text(name.clone())];

    // `isAlikeDead()` — dead, or faking it.
    if helpers::is_dead(world, member)
        || crate::game_loop::abnormal::flags_of(world, member)
            & crate::model::skill::effect_flag::FAKE_DEATH
            != 0
    {
        return Some((
            sm_ids::C1_IS_DEAD_AT_THE_MOMENT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED,
            pc(),
        ));
    }
    if world
        .objects
        .get_component::<crate::model::Player>(&member)
        .is_some_and(|p| p.store_type != 0)
    {
        return Some((
            sm_ids::C1_IS_CURRENTLY_TRADING_OR_OPERATING_A_PRIVATE_STORE_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED,
            pc(),
        ));
    }
    // `isRooted() || isInCombat()`. Both read the *combat* line — a rooted
    // player who has not swung at anyone still gets told they are in combat.
    let rooted = crate::game_loop::abnormal::flags_of(world, member)
        & crate::model::skill::effect_flag::ROOTED
        != 0;
    if rooted || crate::game_loop::combat::has_attack_stance(world, member) {
        return Some((
            sm_ids::C1_IS_ENGAGED_IN_COMBAT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED,
            pc(),
        ));
    }
    // `isInOlympiadMode()` — in a running match, which is narrower than being
    // registered for one; the registered case falls to the observer branch
    // below, with a different message.
    if crate::game_loop::olympiad::in_match(world, member) {
        return Some((
            sm_ids::A_USER_PARTICIPATING_IN_THE_OLYMPIAD_CANNOT_USE_SUMMONING_OR_TELEPORTING,
            Vec::new(),
        ));
    }
    // `isOnEvent() || isFlyingMounted() || isCombatFlagEquipped() ||
    // isInTraingCamp()`. The Training Camp is off-chronicle for this build, so
    // three of the four are testable and the fourth can never be true.
    let flying = world
        .objects
        .get_component::<crate::model::Player>(&member)
        .is_some_and(crate::model::Player::is_flying);
    if crate::game_loop::events::tvt::is_on_event(world, member)
        || flying
        || crate::game_loop::npc::teleporter::has_combat_flag(world, member)
    {
        return Some((
            sm_ids::YOU_CANNOT_USE_SUMMONING_OR_TELEPORTING_IN_THIS_AREA,
            Vec::new(),
        ));
    }
    if world
        .objects
        .has_component::<crate::model::components::player::OlympiadObserver>(&member)
        || world.olympiad.is_registered(member)
    {
        return Some((
            sm_ids::C1_IS_IN_AN_AREA_WHICH_BLOCKS_SUMMONING_OR_TELEPORTING_2,
            text(),
        ));
    }
    // `ZoneId.NO_SUMMON_FRIEND` and `ZoneId.JAIL`. Neither zone kind exists in
    // the port's zone data, so the jail *state* stands in for the jail zone —
    // the same substitution `conditions::call_pc` already makes for the
    // caster-side gate. `NO_SUMMON_FRIEND` has no stand-in and is unreachable.
    if world
        .objects
        .get_component::<crate::model::Player>(&member)
        .is_some_and(|p| p.jailed)
    {
        return Some((
            sm_ids::C1_IS_IN_AN_AREA_WHICH_BLOCKS_SUMMONING_OR_TELEPORTING,
            text(),
        ));
    }
    // Java's last branch is the instance's `isPlayerSummonAllowed`, read off
    // the *caster's* instance world. The port's instance templates carry no
    // such flag, so there is nothing to test — noted rather than silently
    // dropped.
    None
}

/// `CallPc.instant`'s **player** half — Summon Friend, Word of Invitation and
/// friends.
///
/// Nothing teleports here: the target is asked. Java stashes a
/// `SummonRequestHolder` on them and sends a `ConfirmDlg`; the answer arrives
/// as a `DlgAnswer` and `death`-style dispatch routes it to
/// [`accept_summon_request`]. The dialog carries a 30 s auto-decline and the
/// summoner's object id, which the client echoes back — that echo is what
/// stops a second summoner's prompt from being answered by the first's.
///
/// The toll is charged to the **target**, not the caster, and is charged
/// *before* the prompt — so refusing the summon still costs the Spirit Ore.
/// That is Java's order and it is deliberate here: charging on accept instead
/// would make a declined summon free and let a party spam prompts for nothing.
pub(crate) fn call_pc_player(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    item_id: i32,
    item_count: i64,
) {
    use server_packets::{SmParam, sm_ids};

    // `if (effector == effected) return` and the player-effector gate.
    if caster_oid == target_oid
        || !world
            .objects
            .has_component::<crate::model::Player>(&caster_oid)
        || !world
            .objects
            .has_component::<crate::model::Player>(&target_oid)
    {
        return;
    }
    if let Some((sm, params)) = check_summon_target_status(world, target_oid) {
        send_sm_with(world, caster_oid, sm, &params);
        return;
    }
    if item_id != 0 && item_count != 0 {
        let held = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&target_oid)
            .map_or(0, |inv| inv.count_of(item_id));
        if held < item_count {
            // Java tells the **target** they are short, not the summoner.
            send_sm_with(
                world,
                target_oid,
                sm_ids::S1_IS_REQUIRED_FOR_SUMMONING,
                &[SmParam::ItemName(item_id)],
            );
            return;
        }
        // The quest engine's take-items path is the port's one item-removal
        // primitive that also sends the `InventoryUpdate`; the toll needs that,
        // or the client keeps drawing the ore it no longer has.
        let target_client = helpers::client_for_player(world, target_oid).unwrap_or(0);
        inventory::take_items(world, target_client, target_oid, item_id, item_count);
        send_sm_with(
            world,
            target_oid,
            sm_ids::S1_DISAPPEARED,
            &[SmParam::ItemName(item_id)],
        );
    }

    let (x, y, z) = position::pos_of(world, caster_oid).unwrap_or((0, 0, 0));
    let name = helpers::player_name_or_empty(world, caster_oid);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    {
        p.summon_request = Some(crate::model::SummonRequest {
            summoner_object_id: caster_oid,
            x,
            y,
            z,
        });
    }
    helpers::send_to_player(
        world,
        target_oid,
        server_packets::confirm_dlg_with(
            sm_ids::C1_WISHES_TO_SUMMON_YOU_FROM_S2_DO_YOU_ACCEPT as i32,
            &[SmParam::Text(name), SmParam::ZoneName { x, y, z }],
            30_000,
            caster_oid,
        ),
    );
}

/// The `DlgAnswer` leg. Returns true when the reply was a summon answer, so the
/// shared dispatch can stop offering it to the other claimants.
///
/// Java re-checks `holder.getSummoner().getObjectId() == _requesterId` — the
/// stashed summoner must match the id the *client* echoed. Without that, a
/// prompt from one summoner could be answered into a teleport to another, and
/// the request is removed either way so a stale one cannot be replayed.
pub(crate) fn accept_summon_request(
    world: &mut World,
    target_oid: i32,
    requester_id: i32,
    accepted: bool,
) -> bool {
    let Some(req) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
        .and_then(|p| p.summon_request.take())
    else {
        return false;
    };
    if accepted && req.summoner_object_id == requester_id {
        crate::game_loop::death::teleport_player(world, target_oid, req.x, req.y, req.z);
    }
    true
}

/// `handlers/effecthandlers/CallPc.java`, the `player == null` branch — a
/// **monster** dragging its victim to itself. This is Porta's (20213) "Summon"
/// (4161), and Java's body is five lines:
///
/// ```text
/// effected.abortCast();
/// effected.abortAttack();
/// effected.stopMove(null);
/// effected.sendPacket(new FlyToLocation(effected, effector, FlyType.DUMMY, …));
/// effected.setLocation(effector.getLocation());
/// ```
///
/// Note `setLocation`, **not** `teleToLocation`: no fade, no decay/respawn, no
/// `Appearing` round trip. The victim slides across on the client and the
/// server just moves the point. The whole hop is bounded by the skill's
/// `castRange` (600 for 4161), so it never crosses more than one world region
/// and the ordinary visibility sweep picks up the new neighbourhood.
///
/// The `TargetType::Enemy` gate is Java's: `CallPc` on any other target type
/// from a non-player effector falls to the `teleToLocation` branch, which is
/// the *player* being recalled — not something a monster does.
/// `TeleportToTarget.instant` — the mirror of [`call_pc`]: instead of dragging
/// the victim to the caster, the **caster** dashes to a point 25 units behind
/// the target.
///
/// "Behind" is the target's own heading flipped 180°, so the caster lands at
/// its back — a gap-closer, not a swap. Carrier on this dist is skill 4671,
/// used by the Splendor mobs (21524/21531/21539) to catch a runner.
///
/// Java's `canStart` gates on `canSeeTarget(effected, effector)`; the NPC cast
/// pipeline has already run that geodata check by the time an effect applies,
/// so it is not repeated here.
pub(crate) fn teleport_to_target(world: &mut World, caster_oid: i32, target_oid: i32) {
    let (Some(from), Some(to)) = (
        maybe_position(world, caster_oid),
        maybe_position(world, target_oid),
    ) else {
        return;
    };

    // Java: `convertHeadingToDegree(heading)` (heading / 182.044444444, i.e.
    // heading * 360 / 65536), + 180 wrapped into 0..360, then to radians.
    let mut degrees = to.heading as f64 * 360.0 / 65536.0 + 180.0;
    if degrees > 360.0 {
        degrees -= 360.0;
    }
    let radians = degrees.to_radians();
    let x = (to.x as f64 + 25.0 * radians.cos()) as i32;
    let y = (to.y as f64 + 25.0 * radians.sin()) as i32;
    let dest = world
        .geo
        .get_valid_location(from.x, from.y, from.z, x, y, to.z);

    // `setIntention(AI_INTENTION_IDLE)` + `abortAttack()` + `abortCast()`: the
    // dash interrupts whatever the caster was doing, its own cast included.
    world
        .objects
        .remove_component::<crate::model::components::space::Movement>(&caster_oid);
    world
        .objects
        .remove_component::<crate::model::components::combat::Intent>(&caster_oid);
    world
        .objects
        .remove_component::<crate::model::components::combat::AttackState>(&caster_oid);
    crate::game_loop::skills::cast::abort_cast_when_untargeted(world, caster_oid);

    // Java broadcasts `FlyToLocation` *before* `setXYZ` and `ValidateLocation`
    // after it: the client animates the slide, then has the landing confirmed.
    broadcast::broadcast_including_self(
        world,
        caster_oid,
        &server_packets::fly_to_location(
            caster_oid,
            (from.x, from.y, from.z),
            dest,
            server_packets::FlyType::Dummy,
        ),
    );
    position::set_position(world, caster_oid, (dest.0, dest.1, dest.2));
    world.set_player_region(caster_oid, crate::world::region_of(dest.0, dest.1));
    broadcast::broadcast_including_self(
        world,
        caster_oid,
        &server_packets::validate_location(caster_oid, dest.0, dest.1, dest.2, from.heading),
    );
}

pub(crate) fn call_pc(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    // "if (effector == effected) return" — a mob can't summon itself.
    if caster_oid == target_oid {
        return;
    }
    // The ported half is the NPC one; a player effector wants the Summon
    // Friend `ConfirmDlg` round trip, which isn't built (see
    // `SkillEffect::CallPc`).
    if world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    if skill.target_type != crate::model::skill::target::TargetType::Enemy {
        return;
    }
    // `effected.getActingPlayer()` — the branch is player-only; a servitor
    // caught in the cast is left where it stands, as in Java.
    if !world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
    {
        return;
    }
    let Some(dest) = maybe_position(world, caster_oid) else {
        return;
    };
    let Some(from) = maybe_position(world, target_oid) else {
        return;
    };

    // `abortCast()` / `abortAttack()` / `stopMove(null)`.
    //
    // `abortCast()` is `SkillCaster.canAbortCast`-gated — a *target* check, not
    // the phase check its Java comment claims — so it takes the same helper the
    // teleport prologue uses, not [`crate::game_loop::skills::cast::abort_cast`], whose `!launched`
    // guard would swallow the `MagicSkillCanceled` that stops the victim's own
    // cast animation client-side.
    crate::game_loop::skills::cast::abort_cast_when_untargeted(world, target_oid);
    world
        .objects
        .remove_component::<crate::model::components::combat::AttackState>(&target_oid);
    world
        .objects
        .remove_component::<crate::model::components::space::Movement>(&target_oid);
    world
        .objects
        .remove_component::<crate::model::components::combat::Intent>(&target_oid);
    // Java's `stopMove(null)` ends with `broadcastPacket(new StopMove(this))`.
    // Dropping the `Movement` component only stops the *server* walking the
    // victim; without the packet every client keeps animating the run toward
    // the old destination, so the drag leaves the character sliding. Java
    // broadcasts it before `setLocation`, i.e. at the old point.
    broadcast::broadcast_including_self(
        world,
        target_oid,
        &server_packets::stop_move(target_oid, from.x, from.y, from.z, from.heading),
    );

    // Java's `FlyToLocation` constructor arms `blinkActive` for a player
    // target, which makes the next `ValidatePosition` skip its out-of-sync
    // snap — otherwise the victim's own stale position report drags it back
    // out of the mob's lap.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    {
        p.blink_active = true;
    }
    // Java sends `FlyToLocation` to the effected player only; everyone else
    // learns the new position from the movement/validate-position stream. The
    // port broadcasts it so bystanders see the yank rather than a silent
    // teleport — the packet is a pure animation and the client ignores it for
    // objects it can't see.
    broadcast::broadcast_including_self(
        world,
        target_oid,
        &server_packets::fly_to_location(
            target_oid,
            (from.x, from.y, from.z),
            (dest.x, dest.y, dest.z),
            server_packets::FlyType::Dummy,
        ),
    );

    position::set_position(world, target_oid, (dest.x, dest.y, dest.z));
    // Same reason as the respawn teleport: the region index has to move with
    // the cell. No-op on the index for a non-player target.
    world.set_player_region(target_oid, crate::world::region_of(dest.x, dest.y));
    // Java sends nothing else here — in particular no `MagicSkillCanceled` for
    // the caster. A cancel would end the summoning FX the client keeps drawing
    // for the skill's own (skillgrp) duration, past the 2 s cast; Java has that
    // leftover too, so the port keeps it rather than inventing a packet.
}

// --- arms extracted from the `apply_skill_effects` match -------------------
