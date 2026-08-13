//! Servitor AI and owner commands: the follow tick, attack/stop orders, the
//! action-bar packet, and ordered skill use.

use super::*;
/// How close a servitor trails its owner before it stops — Java's
/// `AI_INTENTION_FOLLOW` keeps roughly this spacing, and the port's own
/// `FOLLOW_RANGE` for GM-controlled mobs uses the same figure.
const FOLLOW_RANGE: f64 = 150.0;

/// Java `SummonAI.onIntentionActive` → `setIntention(AI_INTENTION_FOLLOW,
/// owner)`: an idle servitor trails its owner.
///
/// Run from the NPC AI tick. A servitor with an attack target is left alone —
/// the ordinary NPC attack think drives it from that point, exactly as it does
/// for a mob, because "attack whoever is on the aggro list" is the same
/// behaviour once the owner's order has seeded that list.
pub(crate) fn servitor_follow_tick(world: &mut World, servitor_oid: i32) {
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    if !link.following {
        return;
    }
    // Busy attacking? Leave it to the attack think.
    if world
        .objects
        .get_component::<crate::model::npc::NpcAi>(&servitor_oid)
        .is_some_and(|ai| ai.intention == crate::model::npc::NpcIntention::Attack)
    {
        return;
    }
    let (Some(owner), Some(me)) = (
        world
            .objects
            .get_component::<Position>(&link.owner_object_id)
            .copied(),
        maybe_position(world, servitor_oid),
    ) else {
        return;
    };
    let dx = (owner.x - me.x) as f64;
    let dy = (owner.y - me.y) as f64;
    if (dx * dx + dy * dy).sqrt() <= FOLLOW_RANGE {
        return;
    }
    crate::game_loop::ai::move_npc_to(world, servitor_oid, owner.x, owner.y, owner.z);
}

/// `ServitorAttack` (action 22) — order the servitor onto the owner's target.
///
/// Java bails to `AI_INTENTION_FOLLOW` when the target is more than 3000 units
/// off, so a stray click doesn't send the summon across the map. Otherwise it
/// seeds hate and switches the AI to attack, the same primitive `GetAgro` and
/// `Confuse` use — the ported NPC AI derives its target from the aggro list
/// each think rather than caching one.
pub(crate) fn servitor_attack(world: &mut World, owner_oid: i32, target_oid: i32) -> bool {
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return false;
    };
    let Some(distance) = crate::geo::distance::distance_3d(world, owner_oid, target_oid) else {
        return false;
    };
    if distance > 3000.0 {
        // Too far — Java falls back to following rather than obeying.
        if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
            l.following = true;
        }
        return false;
    }
    // An ordered attack stops the follow, or the servitor would drift home
    // between swings.
    if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
        l.following = false;
    }
    force_attack_target(world, servitor_oid, target_oid);
    true
}

/// `ServitorStop` (action 23) — `cancelAction()`: drop the target, stop moving,
/// and go back to trailing the owner.
pub(crate) fn servitor_stop(world: &mut World, owner_oid: i32) -> bool {
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return false;
    };
    if let Some(aggro) = world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&servitor_oid)
    {
        aggro.0.clear();
    }
    world
        .objects
        .remove_component::<crate::model::components::Movement>(&servitor_oid);
    crate::game_loop::helpers::set_active_intention(world, servitor_oid);
    if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
        l.following = true;
    }
    true
}

/// `ServitorHold` (action 21) — toggle "follow me" / "hold your ground".
/// Returns the new follow state.
pub(crate) fn servitor_toggle_follow(world: &mut World, owner_oid: i32) -> Option<bool> {
    let servitor_oid = servitor_of(world, owner_oid)?;
    let l = world
        .objects
        .get_component_mut::<ServitorOf>(&servitor_oid)?;
    l.following = !l.following;
    let now = l.following;
    if !now {
        // Holding ground: stop where you are.
        world
            .objects
            .remove_component::<crate::model::components::Movement>(&servitor_oid);
    }
    Some(now)
}

/// Java action ids for the servitor commands (`dist/game/data/ActionData.xml`).
pub mod action {
    /// `SitStand` — `/sit`, `/stand` and the action-bar toggle.
    pub const SIT_STAND: i32 = 0;
    /// `ServitorHold` — follow me / hold your ground.
    pub const SERVITOR_HOLD: i32 = 21;
    /// `ServitorAttack` — attack my target.
    pub const SERVITOR_ATTACK: i32 = 22;
    /// `ServitorStop` — cancel what you are doing.
    pub const SERVITOR_STOP: i32 = 23;
    /// `Ride` — `/mount`, `/dismount`, `/mountdismount` → `mountPlayer`.
    pub const RIDE: i32 = 38;
    /// `BotReport` — `/AutoHuntingReport`, the report-a-bot button.
    pub const BOT_REPORT: i32 = crate::game_loop::bot_report::BOT_REPORT_ACTION_ID;
    /// `PrivateStore` option 8 — `/packagesale`, the package-sell manage window.
    /// (Its siblings 10 `/vendor` and 28 `/buy` reach the port through the
    /// dedicated `RequestPrivateStore*` packets the client also sends.)
    pub const PACKAGE_SALE: i32 = 61;
}

/// `RequestActionUse` — the servitor commands only. Other action ids (sit,
/// socials, the per-summon skill buttons) are not handled here yet.
pub(crate) fn handle_request_action_use(world: &mut World, client_id: u32, body: &[u8]) {
    use crate::network::server_packets::sm_ids;
    let Some(pkt) = crate::network::client_packets::RequestActionUse::read(body) else {
        return;
    };
    // `ServitorSkillUse` — the summon's own action-bar buttons, bound
    // id → skill in `ActionData.xml`. Looked up rather than matched, because
    // there are 105 of them.
    let servitor_skill = world.data.action_data.servitor_skill(pkt.action_id);
    if servitor_skill.is_none()
        && !matches!(
            pkt.action_id,
            action::SIT_STAND
                | action::SERVITOR_HOLD
                | action::SERVITOR_ATTACK
                | action::SERVITOR_STOP
                | action::RIDE
                | action::PACKAGE_SALE
                | action::BOT_REPORT
        )
    {
        return;
    }
    let Some(owner_oid) = (match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }) else {
        return;
    };
    // Java's shared guard: dead or control-blocked players issue no actions.
    if is_dead(world, owner_oid) || crate::game_loop::abnormal::is_control_blocked(world, owner_oid)
    {
        return;
    }
    // Action 0 (`SitStand` playeraction — `/sit`, `/stand`): the seated toggle.
    if pkt.action_id == action::SIT_STAND {
        crate::game_loop::sit_stand::handle_sit_stand(world, owner_oid);
        return;
    }
    // Action 65 (`BotReport` playeraction — `/AutoHuntingReport`): report the
    // current target as a bot.
    if pkt.action_id == action::BOT_REPORT {
        crate::game_loop::bot_report::handle_bot_report_action(world, client_id, owner_oid);
        return;
    }
    // Action 61 (`PrivateStore` playeraction, option 8 — `/packagesale`): the
    // only private-store action that has no client packet of its own, so the
    // manage window has to open from here (Java `PrivateStore.useAction`).
    if pkt.action_id == action::PACKAGE_SALE {
        crate::game_loop::private_store::open_manage_package(world, client_id);
        return;
    }
    // Action 38 (`Ride` playeraction — `/dismount`): dismounting a mounted
    // player is live; *mounting* an owned strider/wolf runs through the
    // `/mount` user command (`user_commands`, Java `mountPlayer(getPet())`
    // with its level/range/combat gates).
    if pkt.action_id == action::RIDE {
        if world
            .objects
            .get_component::<crate::model::Player>(&owner_oid)
            .is_some_and(crate::model::Player::is_mounted)
        {
            // Java checks NO_LANDING **before** the hungry branch: a wyvern
            // rider over a no-landing zone is refused outright. Unlike the
            // hungry branch below, this one really fires — `no_landing.xml`
            // covers the airspace around the four Grand Boss lairs.
            const MOUNT_WYVERN: u8 = 2;
            let over_no_landing = world
                .objects
                .get_component::<Position>(&owner_oid)
                .is_some_and(|p| world.data.zone_data.in_no_landing_zone(p.x, p.y, p.z));
            if over_no_landing
                && world
                    .objects
                    .get_component::<crate::model::Player>(&owner_oid)
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
            // dismounted. The branch never fires — `isHungry()` requires a live
            // pet and `mount()` unsummons it (see `mounts::is_hungry`) — but a
            // silently-omitted branch is how parity bugs start.
            if crate::game_loop::admin::mounts::is_hungry(world, owner_oid) {
                send_action_failed(world, client_id);
                send_sm_bare_to_client(
                    world,
                    client_id,
                    sm_ids::A_HUNGRY_STRIDER_CANNOT_BE_MOUNTED_OR_DISMOUNTED,
                );
                return;
            }
            crate::game_loop::admin::mounts::dismount(world, owner_oid);
        }
        return;
    }
    // Every handler opens with the same "do you even have one" check.
    if servitor_of(world, owner_oid).is_none() {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm_ids::YOU_DO_NOT_HAVE_A_SERVITOR, &[]),
        );
        return;
    }
    // `Summon.canAttack`'s `isBetrayed()` gate — a servitor under Betray
    // (1380) obeys nothing at all, and says so.
    if let Some(servitor) = servitor_of(world, owner_oid)
        && crate::game_loop::abnormal::flags_of(world, servitor)
            & crate::model::skill::effect_flag::BETRAYED
            != 0
    {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::YOUR_SERVITOR_IS_UNRESPONSIVE_AND_WILL_NOT_OBEY_ANY_ORDERS,
            &[],
        );
        return;
    }
    if let Some(skill_id) = servitor_skill {
        use_servitor_skill(world, owner_oid, skill_id);
        return;
    }
    match pkt.action_id {
        action::SERVITOR_ATTACK => {
            // `player.getTarget()` — no target, nothing to order.
            let Some(target_oid) = world
                .objects
                .get_component::<crate::model::components::TargetRef>(&owner_oid)
                .and_then(|t| t.0)
            else {
                return;
            };
            servitor_attack(world, owner_oid, target_oid);
        }
        action::SERVITOR_STOP => {
            servitor_stop(world, owner_oid);
        }
        _ => {
            servitor_toggle_follow(world, owner_oid);
        }
    }
}

/// `handlers/actionhandlers/ServitorSkillUse` — the owner presses one of the
/// summon's action-bar buttons and the **servitor** casts it.
///
/// The skill must be one the servitor actually knows: the bindings in
/// `ActionData.xml` cover every summon in the game, so most of the 105 rows
/// name a skill this particular servitor has never had. Casting one anyway
/// would let any summon use any other summon's abilities.
///
/// The cast itself goes through `npc_cast::start_cast`, the same path the AI
/// uses, so an ordered skill obeys the same MP cost, mute gates and cooldowns
/// as one the servitor chose itself.
pub(crate) fn use_servitor_skill(world: &mut World, owner_oid: i32, skill_id: i32) {
    use crate::network::server_packets::sm_ids;
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return;
    };

    let known_level = npc_template_id(world, servitor_oid)
        .and_then(|id| world.data.npc_data.get(id))
        .and_then(|t| {
            t.skill_list
                .iter()
                .find(|(sid, _)| *sid == skill_id)
                .map(|(_, lvl)| *lvl)
        });
    let Some(level) = known_level else {
        // Not this summon's skill — Java's handler simply finds nothing to
        // cast. Silent, as it is: the client only shows buttons the summon has.
        return;
    };
    let Some(skill) = skill_by_id(world, skill_id, level) else {
        return;
    };

    // A self/support skill targets the servitor; anything else needs the
    // owner's current target, exactly like the attack command.
    //
    // `OWNER_PET` is the exception Java writes out by hand ahead of target
    // resolution (`Summon.useMagic`: `if (targetType == OWNER_PET) target =
    // _owner`) — the skill aims at the owner whatever they have selected.
    // Master Recharge (4025) is the carrier: without this branch a Baby
    // Kookaburra recharged whatever mob its owner had clicked, and refused
    // with "invalid target" when they had clicked nothing at all.
    let target_oid = if matches!(
        skill.target_type,
        crate::model::skill::TargetType::Self_ | crate::model::skill::TargetType::None_
    ) {
        servitor_oid
    } else if skill.target_type == crate::model::skill::TargetType::OwnerPet {
        owner_oid
    } else {
        match world
            .objects
            .get_component::<crate::model::components::TargetRef>(&owner_oid)
            .and_then(|t| t.0)
        {
            Some(t) => t,
            None => {
                send_sm_bare_to_player(world, owner_oid, sm_ids::INVALID_TARGET);
                return;
            }
        }
    };

    crate::game_loop::npc::cast::cast_checked(world, servitor_oid, target_oid, &skill);
}
