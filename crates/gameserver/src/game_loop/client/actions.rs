//! `RequestActionUse` (0x45) and the `handlers/playeractions/*` family — every
//! non-skill button on the action bar, and the `/command` aliases that share
//! them (`/sit`, `/run`, `/socialbow`, the pet and servitor orders).
//!
//! **This module exists because the dispatch used to be an allow-list.** The
//! handler lived in `servitor/ai.rs` and matched seven hard-coded ids plus the
//! `ServitorSkillUse` table; every other id returned with no packet, no message
//! and no log line. That silently dropped the 20 `SocialAction` rows (all
//! emotes), `RunWalk`, and the whole pet command set — 66 of the 251 rows this
//! dist declares. None of it ever became a `TODO` marker, because nothing in
//! the code knew the rows existed.
//!
//! So the dispatch is now what Java's is: a **table**. `ActionData.xml` maps
//! `id → handler(option)`, this module maps handler name → behaviour, and a
//! name with no arm here says so in the log
//! (Java: `"Couldn't find handler with name: …"`). Adding a handler is one
//! match arm; forgetting one is visible.
//!
//! The handler bodies live next to the system they drive — the servitor orders
//! in `servitor/ai.rs`, the seated toggle in `sit_stand.rs`, the store windows
//! in `private_store.rs`/`crafting.rs` — and this module is the router, exactly
//! as `PlayerActionHandler` is in Java.

use crate::game_loop::helpers::{is_dead, send_action_failed, send_sm_bare_to_client};
use crate::model::Player;
use crate::model::components;

use crate::game_loop::net::broadcast;
use crate::network::client_packets::RequestActionUse;
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;
use tracing::warn;

/// The `ActionData.xml` ids the port names rather than reaches through the
/// handler table. Dispatch itself is by handler name, so this is deliberately
/// short — an id belongs here only when a *guard* singles it out.
pub mod action {
    /// `SitStand` — the one action a fake-dead player may still use, which is
    /// how a rogue gets up off the floor (`RequestActionUse.runImpl`'s
    /// `_actionId != 0`).
    pub const SIT_STAND: i32 = 0;
    /// `ServitorStop`. Dispatch reaches it by handler name like every other
    /// row; the id is kept so the tests that build a packet body can name it.
    #[allow(dead_code)]
    pub const SERVITOR_STOP: i32 = 23;
    /// General Manufacture (`/generalmanufacture`). The only id Java handles
    /// **outside** the table, in `runImpl`'s fallback switch — `ActionData.xml`
    /// declares no row for it.
    pub const GENERAL_MANUFACTURE: i32 = 51;
}

/// Java `Creature.isAlikeDead()` — really dead, or playing dead.
fn is_alike_dead(world: &World, object_id: i32) -> bool {
    is_dead(world, object_id) || is_fake_death(world, object_id)
}

fn is_fake_death(world: &World, object_id: i32) -> bool {
    crate::game_loop::abnormal::flags_of(world, object_id)
        & crate::model::skill::effect_flag::FAKE_DEATH
        != 0
}

/// Port of `RequestActionUse.runImpl`.
pub(crate) fn handle_request_action_use(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = RequestActionUse::read(body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };

    // Guard 1 — "Don't do anything if player is dead or confused". Note the
    // fake-death leg is *conditional*: action 0 stays legal so the player can
    // stand up out of it.
    if (is_fake_death(world, object_id) && pkt.action_id != action::SIT_STAND)
        || is_dead(world, object_id)
        || crate::game_loop::abnormal::is_control_blocked(world, object_id)
    {
        send_action_failed(world, client_id);
        return;
    }

    // Guard 2 — the bot-report penalty. Java walks the first `BOT_PENALTY`
    // buff's effects and asks each `checkCondition(actionId)`; `is_action_blocked`
    // is that predicate, already ported for `TradeRequest`.
    if crate::game_loop::moderation::bot_report::is_action_blocked(world, object_id, pkt.action_id)
    {
        send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_HAVE_BEEN_REPORTED_AS_AN_ILLEGAL_PROGRAM_USER_SO_YOUR_ACTIONS_HAVE_BEEN_RESTRICTED,
        );
        send_action_failed(world, client_id);
        return;
    }

    // Guard 3 — a transformed player may only use the actions the transform's
    // `<actions>` block lists. An empty list is Java's `allowedActions == null`
    // branch: **everything** is refused, not everything allowed.
    if !transform_allows(world, object_id, pkt.action_id) {
        send_action_failed(world, client_id);
        warn!(
            "player_actions: {object_id} used action {} which the transform does not grant",
            pkt.action_id
        );
        return;
    }

    let Some(row) = world.data.action_data.row(pkt.action_id).cloned() else {
        // No row at all — Java's fallback switch.
        if pkt.action_id == action::GENERAL_MANUFACTURE {
            general_manufacture(world, client_id, object_id);
        } else {
            warn!("player_actions: unhandled action type {}", pkt.action_id);
        }
        return;
    };

    dispatch(world, client_id, object_id, &row, &pkt);
}

/// Handler name → behaviour. Java's `PlayerActionHandler.getHandler(name)`.
fn dispatch(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    row: &crate::data::action_data::ActionRow,
    pkt: &RequestActionUse,
) {
    match row.handler.as_str() {
        // `None` is a real handler name in the file — 23 rows carry it, for
        // actions the client resolves by itself (/attack, /pickup, /invite).
        // Java registers nothing under it and logs "couldn't find handler";
        // the port says nothing, because there is nothing to say.
        "None" => {}
        "SitStand" => crate::game_loop::character::sit_stand::handle_sit_stand(world, object_id),
        "RunWalk" => run_walk(world, object_id),
        "SocialAction" => social_action(world, client_id, object_id, row.option),
        "BotReport" => crate::game_loop::moderation::bot_report::handle_bot_report_action(
            world, client_id, object_id,
        ),
        "TacticalSignUse" => crate::game_loop::party::handle_tactical_sign_use(
            world, client_id, object_id, row.option,
        ),
        "TacticalSignTarget" => crate::game_loop::party::handle_tactical_sign_target(
            world, client_id, object_id, row.option,
        ),
        "PrivateStore" => private_store(world, client_id, row.option),
        "Ride" => ride(world, client_id, object_id),
        "ServitorHold" | "ServitorAttack" | "ServitorStop" | "ServitorSkillUse"
        | "ServitorMove" | "ServitorMode" | "UnsummonServitor" => {
            crate::game_loop::servitor::handle_servitor_action(
                world,
                client_id,
                object_id,
                &row.handler,
                row.option,
            )
        }
        "PetHold" | "PetAttack" | "PetStop" | "PetMove" | "UnsummonPet" | "PetSkillUse" => {
            crate::game_loop::servitor::handle_pet_action(
                world,
                client_id,
                object_id,
                &row.handler,
                row.option,
            )
        }
        other => {
            // The point of the table: an unported handler names itself once
            // per press instead of vanishing. Java's own wording.
            warn!(
                "player_actions: couldn't find handler with name: {other} (action {}, option {}, ctrl={}, shift={})",
                pkt.action_id, row.option, pkt.ctrl_pressed, pkt.shift_pressed
            );
        }
    }
}

/// Java's transform gate: `template.getBasicActionList()`, searched for this
/// id. Untransformed players are unrestricted.
fn transform_allows(world: &World, object_id: i32, action_id: i32) -> bool {
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return true;
    };
    if player.transform_id == 0 {
        return true;
    }
    let is_female = player.is_female;
    let Some(tf) = world.data.transforms.get(player.transform_id) else {
        return true;
    };
    tf.template(is_female).actions.contains(&action_id)
}

// ---------------------------------------------------------------------------
// `handlers/playeractions/RunWalk`
// ---------------------------------------------------------------------------

/// `RunWalk.useAction` — `isRunning() ? setWalking() : setRunning()`.
///
/// The whole of Java's handler is that toggle; the behaviour is in
/// `Creature.setRunning(boolean)`, ported below.
pub(crate) fn run_walk(world: &mut World, object_id: i32) {
    let running = world
        .objects
        .get_component::<components::Speeds>(&object_id)
        .is_some_and(|s| s.running);
    set_running(world, object_id, !running);
}

/// Java `Creature.setRunning(boolean)`: no-op when unchanged, else flip the
/// flag, broadcast `ChangeMoveType` (guarded on a non-zero run speed, since a
/// rooted-to-zero creature has no gait to show), and refresh the player's own
/// info.
///
/// The port needs no stat recalculation here: `Speeds::move_speed` picks the
/// run or walk figure live off this same flag, so movement follows the moment
/// it changes.
pub(crate) fn set_running(world: &mut World, object_id: i32, running: bool) {
    let Some(speeds) = world
        .objects
        .get_component_mut::<components::Speeds>(&object_id)
    else {
        return;
    };
    if speeds.running == running {
        return;
    }
    speeds.running = running;
    let run_spd = speeds.run_spd;
    if run_spd != 0.0 {
        let pkt = server_packets::change_move_type(object_id, running);
        broadcast::broadcast_including_self(world, object_id, &pkt);
    }
    // `if (isPlayer()) getActingPlayer().broadcastUserInfo()`. The summon and
    // NPC legs of Java's method are not reachable from here — this is the
    // player action — and both already have their own callers in the port.
    if world.objects.has_component::<Player>(&object_id) {
        crate::game_loop::character::player_info::broadcast_user_info(world, object_id);
    }
}

// ---------------------------------------------------------------------------
// `handlers/playeractions/SocialAction`
// ---------------------------------------------------------------------------

/// `SocialAction.useAction` — the emotes.
///
/// Java splits the option ids three ways: the plain socials broadcast and
/// stop; id 30 (Show Off) additionally re-broadcasts the character's info; and
/// 16/17/18 are the *couple* socials, which negotiate with a partner.
fn social_action(world: &mut World, client_id: u32, object_id: i32, option: i32) {
    match option {
        // 2..=15, 28, 29 — Greeting, Victory, Advance, No, Yes, Bow, Unaware,
        // Waiting, Laugh, Applaud, Dance, Sorrow, Charm, Shyness, Propose,
        // Provoke.
        2..=15 | 28 | 29 => {
            use_social(world, client_id, object_id, option);
        }
        // 30 — Show Off (`/Socialbeauty`). `broadcastInfo()` on success, which
        // for a player is `broadcastUserInfo()`.
        30 => {
            if use_social(world, client_id, object_id, option) {
                crate::game_loop::character::player_info::broadcast_user_info(world, object_id);
            }
        }
        // SKIP(census): 16/17/18 — Exchange Bows, High Five, Couple Dance.
        // Java's `useCoupleSocial` asks the target to agree over
        // `ExAskCoupleAction` and completes on `RequestAnswerCoupleAction`,
        // neither of which exists before Gracia; an Interlude client has no
        // way to answer, so the negotiation could only ever time out. The
        // three rows ship in `ActionData.xml` regardless, which is why they
        // are named here rather than left to the fallback warn.
        16..=18 => {}
        other => {
            warn!("player_actions: SocialAction with unknown option {other}");
        }
    }
}

/// Java `SocialAction.useSocial` — the guarded broadcast. Returns whether the
/// emote played, which is what the Show Off branch keys its info refresh on.
fn use_social(world: &mut World, client_id: u32, object_id: i32, social_id: i32) -> bool {
    if world
        .objects
        .get_component::<components::FishingSession>(&object_id)
        .is_some_and(|f| f.is_fishing)
    {
        send_sm_bare_to_client(world, client_id, sm_ids::YOU_CANNOT_DO_THAT_WHILE_FISHING_3);
        return false;
    }
    if !can_make_social_action(world, object_id) {
        return false;
    }
    let pkt = server_packets::social_action(object_id, social_id);
    broadcast::broadcast_including_self(world, object_id, &pkt);
    // Java also fires `ON_PLAYER_SOCIAL_ACTION` here. Nothing on this dist
    // listens for it — `SocialAction.java` is the only file in the datapack
    // that names the event — so there is no listener list to notify.
    true
}

/// Java `Player.canMakeSocialAction()`:
/// `(_privateStoreType == NONE) && (getActiveRequester() == null) &&
///  !isAlikeDead() && !isAllSkillsDisabled() && !isCastingNow() &&
///  (getAI().getIntention() == AI_INTENTION_IDLE)`.
///
/// The last clause is the one that needs a mapping: the port has no single
/// intention slot for a player. IDLE is therefore read as its components —
/// not running an attack loop (`Intent`), not walking somewhere (`Movement`),
/// and not seated (`sit_stand::is_resting`, which stands in for
/// `AI_INTENTION_REST` exactly as that module documents). Together those are
/// every non-IDLE intention a player can hold here.
fn can_make_social_action(world: &World, object_id: i32) -> bool {
    let store_none = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.store_type == 0);
    store_none
        && !world
            .objects
            .has_component::<components::PendingTrade>(&object_id)
        && !is_alike_dead(world, object_id)
        && !crate::game_loop::abnormal::all_skills_disabled(world, object_id)
        && !world
            .objects
            .has_component::<components::Casting>(&object_id)
        && !world
            .objects
            .has_component::<components::Intent>(&object_id)
        && !world
            .objects
            .has_component::<components::Movement>(&object_id)
        && !crate::game_loop::character::sit_stand::is_resting(world, object_id)
}

// ---------------------------------------------------------------------------
// `handlers/playeractions/PrivateStore` + the General Manufacture fallback
// ---------------------------------------------------------------------------

/// `PrivateStore.useAction`, keyed on `PrivateStoreType.findById(optionId)`.
///
/// Options 1 (`/vendor`) and 3 (`/buy`) also have dedicated client packets
/// (`RequestPrivateStoreManageSell`/`Buy`) that the client sends alongside, and
/// those already route to the same windows — so both arms land on the ported
/// opener rather than duplicating its guards.
fn private_store(world: &mut World, client_id: u32, option: i32) {
    // Java's `!player.canOpenPrivateStore()` guard is not repeated here: each
    // opener below already runs `can_open_private_store`, which both refuses
    // and sends the no-store-zone message. Duplicating it would send the
    // message twice.
    match option {
        // SELL(1) / SELL_MANAGE(2) / PACKAGE_SELL(8)
        1 | 2 => crate::game_loop::commerce::private_store::open_manage(world, client_id),
        8 => crate::game_loop::commerce::private_store::open_manage_package(world, client_id),
        // BUY(3) / BUY_MANAGE(4)
        3 | 4 => crate::game_loop::commerce::private_store::open_manage_buy(world, client_id),
        // MANUFACTURE(5)
        5 => crate::game_loop::commerce::crafting::open_manage(world, client_id),
        other => warn!("player_actions: incorrect private store type: {other}"),
    }
}

/// `RequestActionUse.runImpl`'s fallback for action 51 — the manufacture
/// window opened by `/generalmanufacture` rather than by a store type.
///
/// SKIP(census): Java sends `new RecipeShopManageList(player, false)` — the
/// *common* book — where the port's opener derives the book from the player's
/// craft skill. The two agree for everyone without Create Item; a dwarf gets
/// their dwarven book here and the common one in Java.
fn general_manufacture(world: &mut World, client_id: u32, object_id: i32) {
    if is_alike_dead(world, object_id) {
        send_action_failed(world, client_id);
        return;
    }
    crate::game_loop::commerce::crafting::open_manage(world, client_id);
}

// ---------------------------------------------------------------------------
// `handlers/playeractions/Ride`
// ---------------------------------------------------------------------------

/// `Ride.useAction` — `/dismount`. Dismounting a mounted player is live;
/// *mounting* an owned strider/wolf runs through the `/mount` user command
/// (`user_commands`, Java `mountPlayer(getPet())` with its level/range/combat
/// gates).
fn ride(world: &mut World, client_id: u32, object_id: i32) {
    if !world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(Player::is_mounted)
    {
        return;
    }
    // Java checks NO_LANDING **before** the hungry branch: a wyvern rider over
    // a no-landing zone is refused outright. Unlike the hungry branch below,
    // this one really fires — `no_landing.xml` covers the airspace around the
    // four Grand Boss lairs.
    const MOUNT_WYVERN: u8 = 2;
    let over_no_landing = world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
        .is_some_and(|p| world.data.zone_data.in_no_landing_zone(p.x, p.y, p.z));
    if over_no_landing
        && world
            .objects
            .get_component::<Player>(&object_id)
            .is_some_and(|p| p.mount_type == MOUNT_WYVERN)
    {
        send_action_failed(world, client_id);
        send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_ARE_NOT_ALLOWED_TO_DISMOUNT_IN_THIS_LOCATION,
        );
        return;
    }
    // Java's other refusal, ported for shape: a hungry mount cannot be
    // dismounted. The branch never fires — `isHungry()` requires a live pet and
    // `mount()` unsummons it (see `mounts::is_hungry`) — but a silently-omitted
    // branch is how parity bugs start.
    if crate::game_loop::admin::mounts::is_hungry(world, object_id) {
        send_action_failed(world, client_id);
        send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::A_HUNGRY_STRIDER_CANNOT_BE_MOUNTED_OR_DISMOUNTED,
        );
        return;
    }
    crate::game_loop::admin::mounts::dismount(world, object_id);
}
