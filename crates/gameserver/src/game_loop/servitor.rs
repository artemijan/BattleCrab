//! Servitors — the summoned-creature half of G29.
//!
//! Java models a servitor as a `Summon` (a `Creature` subclass) owned by a
//! player. This port reuses the existing NPC entity and marks it with a
//! [`ServitorOf`] component instead: a servitor is already "an NPC with a
//! template, stats, position and an AI", and the only genuinely new state is
//! the owner link and the lifetime.
//!
//! **Scope of this slice:** summoning, ownership, unsummon and the owner's
//! `PetInfo` view. The servitor stands where it was summoned; follow/attack AI
//! and the `SummonInfo` packet that shows it to *other* players are separate
//! slices (see `PLAN_G29_SERVITOR_SUMMON.md`).

use crate::game_loop::guard::position;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::item_id_of;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::npc_name_or_empty;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::send_sm_bare_to_player;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::skill_by_id;
use crate::model::components::{Collision, CombatStats, Position, ServitorOf, Speeds, Vitals};
use crate::network::server_packets;
use crate::world::World;
use commons::network::PacketWriter;

/// Game ticks in one second (the loop runs at [`crate::game_loop::TICK`],
/// 100 ms).
const TICKS_PER_SECOND: u64 = 10;

/// Java's `Servitor.run()` period — a fixed `usedtime = 5000` ms.
const LIFE_TICK_SECS: u64 = 5;

/// Java's default `consumeItemInterval` for a non-siege-weapon servitor: 240 s
/// (siege weapons use 60). The per-skill override is rare on this dist, so the
/// default is what almost every summon runs on.
const CONSUME_INTERVAL_SECS: u64 = 240;

/// Java's leash: further than this from its owner and the servitor is forced
/// back into follow, regardless of what it was doing.
const LEASH_DISTANCE: f64 = 2000.0;

/// The Sin Eater's display id — the one species Java summons at its *owner's*
/// level rather than its template level (`Pet`'s three-arg constructor).
const SIN_EATER_DISPLAY_ID: i32 = 12564;

use super::helpers::client_for_player;

/// Java `Player.getServitors()` — this port scans rather than caching a second
/// index, because a player has at most one servitor on this dist.
pub(crate) fn servitor_of(world: &World, owner_oid: i32) -> Option<i32> {
    let oid = world
        .objects
        .get_component::<crate::model::components::SummonRef>(&owner_oid)?
        .servitor?;
    // Validated on read: a despawn path that forgot to clear the link yields
    // `None` here rather than a dangling id.
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .map(|_| oid)
}

/// `Summon.instant` — spawn a servitor for `owner_oid`.
///
/// Java unsummons any existing servitors first (`player.getServitors().values()
/// .forEach(s -> s.unSummon(player))`), so re-casting swaps rather than stacks.
#[allow(clippy::too_many_arguments)]
pub(crate) fn summon_servitor(
    world: &mut World,
    owner_oid: i32,
    npc_id: i32,
    reference_skill: i32,
    life_time: i32,
    consume_item_id: i32,
    consume_item_count: i64,
) -> Option<i32> {
    // Players only (Java's `if (!effected.isPlayer()) return`).
    world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)?;
    unsummon_servitor(world, owner_oid);

    let pos = position(world, owner_oid)?;
    let servitor_oid =
        crate::model::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, pos.heading)?;

    // `lifeTime <= 0` → Java's `Integer.MAX_VALUE` ("Classic hack. Resummon
    // upon entering game."), i.e. no expiry while the session lasts.
    let expires_at_tick = if life_time > 0 {
        world.tick + (life_time as u64) * TICKS_PER_SECOND
    } else {
        u64::MAX
    };
    world.objects.add_components(
        &servitor_oid,
        ServitorOf {
            owner_object_id: owner_oid,
            reference_skill,
            expires_at_tick,
            life_time_secs: life_time,
            // Java: a fresh summon follows (`getFollowStatus()` defaults true).
            following: true,
            consume_item_id,
            consume_item_count,
            next_consume_tick: if consume_item_id > 0 {
                world.tick + CONSUME_INTERVAL_SECS * TICKS_PER_SECOND
            } else {
                u64::MAX
            },
        },
    );
    // `summon.setCurrentHp(getMaxHp()); setCurrentMp(getMaxMp())`.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&servitor_oid) {
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
    }
    set_summon_link(world, owner_oid, Some(servitor_oid), None, false);
    // Java arms `_summonLifeTask` at a fixed 5 s period.
    world.scheduler.schedule(
        world.tick + LIFE_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::ServitorLifeTick { servitor_oid },
    );
    send_pet_info(world, owner_oid, servitor_oid, PetInfoKind::Summoned);
    // Everyone else nearby gets `SummonInfo` with the spawn animation — Java's
    // `setShowSummonAnimation(true)` before `spawnMe()`.
    broadcast_summon_info(world, servitor_oid, true);
    Some(servitor_oid)
}

/// `Summon.unSummon` — remove the owner's servitor from the world.
///
/// Returns the object id that went away, so callers can report it.
pub(crate) fn unsummon_servitor(world: &mut World, owner_oid: i32) -> Option<i32> {
    // A pet also carries `ServitorOf`, so this one path retires either kind;
    // clear both halves of the link rather than guessing which it was.
    let servitor_oid = servitor_of(world, owner_oid).or_else(|| pet_of(world, owner_oid))?;
    set_summon_link(world, owner_oid, None, None, false);
    set_summon_link(world, owner_oid, None, None, true);
    let region = world
        .objects
        .get_component::<crate::model::components::RegionCell>(&servitor_oid)?
        .0;
    crate::game_loop::death::despawn_npc(world, servitor_oid, region);
    Some(servitor_oid)
}

/// The `value` byte of `PetSummonInfo`: 0 = teleported, 1 = default,
/// 2 = summoned (Java sends 2 whenever `isShowSummonAnimation()`).
#[derive(Clone, Copy)]
pub(crate) enum PetInfoKind {
    Summoned,
    Default,
}

/// `Summon.sendInfo` for the owner: `PetSummonInfo` (`PET_INFO`, 0xB2).
///
/// Other players get `SummonInfo` (0x8B, a masked packet) — not ported in this
/// slice, so a servitor is currently visible only to the player who summoned
/// it. That is a deliberate, documented narrowing, not an oversight.
pub(crate) fn send_pet_info(world: &World, owner_oid: i32, servitor_oid: i32, kind: PetInfoKind) {
    let Some(client_id) = client_for_player(world, owner_oid) else {
        return;
    };
    let Some(cs) = world.clients.get(&client_id) else {
        return;
    };
    let Some(pkt) = build_pet_info(world, owner_oid, servitor_oid, kind) else {
        return;
    };
    cs.send(pkt);
}

fn build_pet_info(
    world: &World,
    owner_oid: i32,
    servitor_oid: i32,
    kind: PetInfoKind,
) -> Option<Vec<u8>> {
    let npc = world
        .objects
        .get_component::<crate::model::npc::Npc>(&servitor_oid)?;
    let template = npc.template(world)?;
    let pos = world.objects.get_component::<Position>(&servitor_oid)?;
    let vitals = world.objects.get_component::<Vitals>(&servitor_oid)?;
    let cs = world.objects.get_component::<CombatStats>(&servitor_oid)?;
    let speeds = world.objects.get_component::<Speeds>(&servitor_oid)?;
    let collision = world.objects.get_component::<Collision>(&servitor_oid)?;
    let servitor = world.objects.get_component::<ServitorOf>(&servitor_oid)?;
    let pet = world
        .objects
        .get_component::<crate::model::components::PetOf>(&servitor_oid)
        .copied();
    let owner_name = world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)
        .map(|p| p.name.clone())?;

    // Java divides the wire speeds by the move multiplier (the client
    // multiplies them back) — the same treatment `UserInfo`/`CharInfo` already
    // get on this port.
    let mult = speeds.move_multiplier;
    let run = (speeds.run_spd / mult).round() as i16;
    let walk = (speeds.walk_spd / mult).round() as i16;

    // `getLifeTimeRemaining()` / `getLifeTime()` ride in the fed/max-fed pair
    // for a servitor — this is what draws the summon's remaining-time bar.
    // For a **pet** this pair is the real food bar; for a servitor Java reuses
    // the same two fields for its remaining lifetime, which is what draws the
    // summon time bar.
    let (cur_fed, max_fed) = match pet {
        Some(p) => (p.fed, p.max_fed),
        None if servitor.life_time_secs > 0 => {
            let remaining = servitor.expires_at_tick.saturating_sub(world.tick) / TICKS_PER_SECOND;
            (remaining as i32, servitor.life_time_secs)
        }
        None => (0, 0),
    };

    let mut w = PacketWriter::new();
    w.write_u8(server_packets::opcodes::PET_INFO);
    // `getSummonType()`: 1 = pet, 2 = servitor. The client uses it to decide
    // whether to offer the pet inventory and food bar.
    w.write_u8(if pet.is_some() { 1 } else { 2 });
    w.write_i32(servitor_oid);
    w.write_i32(template.display_id + 1_000_000);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(pos.heading);
    w.write_i32(cs.m_atk_spd);
    w.write_i32(cs.p_atk_spd);
    for v in [run, walk, 0, 0, 0, 0, 0, 0] {
        w.write_i16(v); // run/walk, swim run/walk, fly-ground run/walk, fly run/walk
    }
    w.write_f64(mult);
    w.write_f64(1.0); // attack speed multiplier
    w.write_f64(collision.radius);
    w.write_f64(collision.height);
    w.write_i32(0); // right hand weapon
    w.write_i32(0); // body armor
    w.write_i32(0); // left hand weapon
    w.write_u8(match kind {
        PetInfoKind::Summoned => 2,
        PetInfoKind::Default => 1,
    });
    w.write_i32(-1); // NPCString id
    // A servitor sends its name only when the template is server-side named;
    // otherwise the client uses the template's own.
    w.write_string(if template.server_side_name {
        &template.name
    } else {
        ""
    });
    w.write_i32(-1); // NPCString id
    w.write_string(&owner_name); // the title slot carries the owner's name
    w.write_u8(0); // pvp flag
    w.write_i32(0); // reputation
    w.write_i32(cur_fed);
    w.write_i32(max_fed);
    w.write_i32(vitals.cur_hp as i32);
    w.write_i32(vitals.max_hp);
    w.write_i32(vitals.cur_mp as i32);
    w.write_i32(vitals.max_mp);
    w.write_i64(0); // sp
    w.write_u8(template.level as u8);
    w.write_i64(0); // exp
    w.write_i64(0); // exp at this level
    w.write_i64(0); // exp for next level
    w.write_i32(0); // carried weight (pets only)
    w.write_i32(0); // max load
    w.write_i32(cs.p_atk as i32);
    w.write_i32(cs.p_def as i32);
    w.write_i32(cs.accuracy);
    w.write_i32(cs.evasion);
    w.write_i32(cs.crit_hit as i32);
    w.write_i32(cs.m_atk as i32);
    w.write_i32(cs.m_def as i32);
    w.write_i32(cs.magic_accuracy);
    w.write_i32(cs.magic_evasion);
    w.write_i32(cs.m_crit_hit as i32);
    w.write_i32(speeds.move_speed() as i32);
    w.write_i32(cs.p_atk_spd);
    w.write_i32(cs.m_atk_spd);
    w.write_u8(0); // ride status
    w.write_u8(0); // team
    w.write_u8(0); // soulshots per hit
    w.write_u8(0); // spiritshots per hit
    w.write_i32(0);
    w.write_i32(0); // transformation id
    w.write_u8(0); // used summon points
    w.write_u8(0); // max summon points
    let aves = crate::game_loop::abnormal::visual_effects(world, servitor_oid);
    w.write_i16(aves.len() as i16);
    for id in aves {
        w.write_i16(id);
    }
    // `_statusMask`: 0x02 "can be chatted with" is unconditional in Java;
    // 0x04 is "running", which a freshly summoned servitor is
    // (`summon.setRunning()`).
    let mut status = 0x02u8 | 0x04;
    if vitals.dead {
        status |= 0x10;
    }
    w.write_u8(status);
    Some(w.into_bytes())
}

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
        position(world, servitor_oid),
    ) else {
        return;
    };
    let dx = (owner.x - me.x) as f64;
    let dy = (owner.y - me.y) as f64;
    if (dx * dx + dy * dy).sqrt() <= FOLLOW_RANGE {
        return;
    }
    crate::game_loop::npc_ai::move_npc_to(world, servitor_oid, owner.x, owner.y, owner.z);
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
    let (Some(owner), Some(target)) = (position(world, owner_oid), position(world, target_oid))
    else {
        return false;
    };
    let dx = (owner.x - target.x) as f64;
    let dy = (owner.y - target.y) as f64;
    let dz = (owner.z - target.z) as f64;
    if (dx * dx + dy * dy + dz * dz).sqrt() > 3000.0 {
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
    let max_hate = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&servitor_oid)
        .map(|a| a.0.values().map(|i| i.hate).fold(0.0_f64, f64::max))
        .unwrap_or(0.0);
    if let Some(aggro) = world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&servitor_oid)
    {
        aggro.0.entry(target_oid).or_default().hate = max_hate + 1.0;
    }
    if let Some(ai) = world
        .objects
        .get_component_mut::<crate::model::npc::NpcAi>(&servitor_oid)
    {
        ai.intention = crate::model::npc::NpcIntention::Attack;
        ai.attack_timeout_tick = world.tick + crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
    }
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
    if let Some(ai) = world
        .objects
        .get_component_mut::<crate::model::npc::NpcAi>(&servitor_oid)
    {
        ai.intention = crate::model::npc::NpcIntention::Active;
    }
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
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::action_failed());
                    cs.send(server_packets::system_message_with(
                        sm_ids::YOU_ARE_NOT_ALLOWED_TO_DISMOUNT_IN_THIS_LOCATION,
                        &[],
                    ));
                }
                return;
            }
            // Java's other refusal, ported for shape: a hungry mount cannot be
            // dismounted. The branch never fires — `isHungry()` requires a live
            // pet and `mount()` unsummons it (see `mounts::is_hungry`) — but a
            // silently-omitted branch is how parity bugs start.
            if crate::game_loop::admin::mounts::is_hungry(world, owner_oid) {
                if let Some(cs) = world.clients.get(&client_id) {
                    cs.send(server_packets::action_failed());
                    cs.send(server_packets::system_message_with(
                        sm_ids::A_HUNGRY_STRIDER_CANNOT_BE_MOUNTED_OR_DISMOUNTED,
                        &[],
                    ));
                }
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
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::YOUR_SERVITOR_IS_UNRESPONSIVE_AND_WILL_NOT_OBEY_ANY_ORDERS,
                &[],
            ));
            cs.send(server_packets::action_failed());
        }
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

/// `SummonInfo` to every nearby player except the owner (who has the
/// `PetInfo` view). Used when the servitor first appears.
pub(crate) fn broadcast_summon_info(world: &mut World, servitor_oid: i32, summoned: bool) {
    use crate::model::components::RegionCell;
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    let Some(region) = region_cell_of(world, servitor_oid) else {
        return;
    };
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&servitor_oid)
    else {
        return;
    };
    let Some(t) = npc.template(world) else { return };
    let (Some(pos), Some(vitals), Some(speeds), Some(combat)) = (
        world.objects.get_component::<Position>(&servitor_oid),
        world.objects.get_component::<Vitals>(&servitor_oid),
        world.objects.get_component::<Speeds>(&servitor_oid),
        world.objects.get_component::<CombatStats>(&servitor_oid),
    ) else {
        return;
    };
    let owner_name = world
        .objects
        .get_component::<crate::model::Player>(&link.owner_object_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    // `Summon.isBetrayed()` — read before the borrow, since the packet build
    // holds references into `world.objects`.
    let betrayed = crate::game_loop::abnormal::flags_of(world, servitor_oid)
        & crate::model::skill::effect_flag::BETRAYED
        != 0;
    let pkt = server_packets::summon_info(
        servitor_oid,
        t,
        pos,
        vitals,
        speeds,
        combat,
        &owner_name,
        0,
        summoned,
        betrayed,
    );
    for cs in world.clients.values() {
        let crate::session::ClientSession::InGame(session) = cs else {
            continue;
        };
        let viewer = session.player_object_id();
        if viewer == link.owner_object_id {
            continue; // the owner has the PetInfo view
        }
        let Some(vr) = world.objects.get_component::<RegionCell>(&viewer) else {
            continue;
        };
        if crate::world::regions_adjacent(region, vr.0) {
            cs.send(pkt.clone());
        }
    }
}

/// Java `Servitor.run()` — the 5-second upkeep tick.
///
/// In order: lifetime countdown (expiry → "Your servitor passed away" +
/// unsummon), the periodic upkeep item (missing → "not enough items" +
/// unsummon), the remain-time bar, and the far-from-owner leash. Reschedules
/// itself while the servitor lives, which is Java's `_summonLifeTask` cancelled
/// on death/despawn.
pub(crate) fn handle_life_tick(world: &mut World, servitor_oid: i32) {
    use crate::network::server_packets::{SmParam, sm_ids};
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    // Dead or already gone → the chain ends (Java cancels the task).
    if is_dead(world, servitor_oid) {
        return;
    }
    let owner = link.owner_object_id;

    // 1. Lifetime.
    if world.tick >= link.expires_at_tick {
        notify_owner(world, owner, sm_ids::YOUR_SERVITOR_PASSED_AWAY, &[]);
        unsummon_servitor(world, owner);
        return;
    }

    // 2. Upkeep item.
    if link.consume_item_id > 0 && world.tick >= link.next_consume_tick {
        // `destroyItemByItemId` — take the upkeep item, or fail.
        use crate::model::inventory::Inventory;
        let have = world
            .objects
            .get_component::<Inventory>(&owner)
            .map(|inv| inv.count_of(link.consume_item_id))
            .unwrap_or(0);
        let taken = have >= link.consume_item_count;
        if taken {
            let changes = world
                .objects
                .get_component_mut::<Inventory>(&owner)
                .map(|inv| inv.remove_item(link.consume_item_id, link.consume_item_count))
                .unwrap_or_default();
            if let Some(cid) = client_for_player(world, owner) {
                let iu =
                    crate::network::enter_world::inventory_update_changes(&world.data, &changes);
                crate::game_loop::helpers::send_inventory_update(world, cid, owner, iu);
            }
            notify_owner(
                world,
                owner,
                sm_ids::A_SUMMONED_MONSTER_USES_S1,
                &[SmParam::ItemName(link.consume_item_id)],
            );
            if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
                l.next_consume_tick = world.tick + CONSUME_INTERVAL_SECS * TICKS_PER_SECOND;
            }
        } else {
            notify_owner(
                world,
                owner,
                sm_ids::NOT_ENOUGH_ITEMS_TO_MAINTAIN_SERVITOR,
                &[],
            );
            unsummon_servitor(world, owner);
            return;
        }
    }

    // 3. The remaining-time bar.
    if link.life_time_secs > 0 {
        let remaining = (link.expires_at_tick.saturating_sub(world.tick) / TICKS_PER_SECOND) as i32;
        if let Some(cid) = client_for_player(world, owner)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(server_packets::set_summon_remain_time(
                link.life_time_secs,
                remaining,
            ));
        }
    }

    // 4. The leash — "using same task to check if owner is in visible range".
    // A servitor left too far behind is dragged back into follow whatever it
    // was doing, so an ordered attack can't strand it across the map.
    if let (Some(me), Some(o)) = (position(world, servitor_oid), position(world, owner)) {
        let dx = (me.x - o.x) as f64;
        let dy = (me.y - o.y) as f64;
        let dz = (me.z - o.z) as f64;
        if (dx * dx + dy * dy + dz * dz).sqrt() > LEASH_DISTANCE {
            if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
                l.following = true;
            }
            if let Some(ai) = world
                .objects
                .get_component_mut::<crate::model::npc::NpcAi>(&servitor_oid)
            {
                ai.intention = crate::model::npc::NpcIntention::Active;
            }
        }
    }

    world.scheduler.schedule(
        world.tick + LIFE_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::ServitorLifeTick { servitor_oid },
    );
}

fn notify_owner(
    world: &World,
    owner_oid: i32,
    sm: i16,
    params: &[crate::network::server_packets::SmParam],
) {
    let Some(cid) = client_for_player(world, owner_oid) else {
        return;
    };
    let Some(cs) = world.clients.get(&cid) else {
        return;
    };
    cs.send(server_packets::system_message_with(sm, params));
}

/// The owner left the world (logout/disconnect) — their servitor goes with
/// them. Java stores it in `CharSummonTable` for `RestoreServitorOnReconnect`;
/// persistence is a later slice, so this just removes it.
pub(crate) fn on_owner_leave_world(world: &mut World, owner_oid: i32) {
    // Capture the summon's state before the entity goes away — after
    // `unsummon_servitor` there is nothing left to read it from.
    sync_pet_row(world, owner_oid);
    sync_summon_row(world, owner_oid);
    unsummon_servitor(world, owner_oid);
}

// ---------------------------------------------------------------------------
// Pets
// ---------------------------------------------------------------------------

/// The object id of the collar that summoned the player's currently-out pet.
///
/// Java reads this as `player.getPet().getControlObjectId()` at each use site;
/// here it is one lookup so the sell/trade lists cannot drift apart. `None`
/// when no pet is out — which is also the case for a *servitor* owner, since a
/// skill-summoned servitor has no collar.
pub(crate) fn active_pet_collar(world: &World, owner_oid: i32) -> Option<i32> {
    let pet = pet_of(world, owner_oid)?;
    world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet)
        .map(|p| p.collar_object_id)
}

/// Java `Player.getPet()` — a player has at most one.
pub(crate) fn pet_of(world: &World, owner_oid: i32) -> Option<i32> {
    let oid = world
        .objects
        .get_component::<crate::model::components::SummonRef>(&owner_oid)?
        .pet?;
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .map(|_| oid)
}

/// Set or clear the owner's summon link. Every spawn and despawn path goes
/// through here, so the link can never be updated in only one direction.
fn set_summon_link(
    world: &mut World,
    owner_oid: i32,
    servitor: Option<i32>,
    pet: Option<i32>,
    is_pet: bool,
) {
    if world
        .objects
        .get_component::<crate::model::components::SummonRef>(&owner_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&owner_oid, crate::model::components::SummonRef::default());
    }
    if let Some(r) = world
        .objects
        .get_component_mut::<crate::model::components::SummonRef>(&owner_oid)
    {
        if is_pet {
            r.pet = pet;
        } else {
            r.servitor = servitor;
        }
    }
}

/// `SummonPet.instant` — bring out the pet bound to the collar the player just
/// used.
///
/// The collar arrives through `Player.pending_pet_collar` (Java's
/// `PetItemHolder`) and is **taken**, so a stale value can never summon a
/// second pet. Every stat comes from `PetData`, keyed by the collar's *item*
/// id; the collar's *object* id becomes the pet's identity.
///
/// A pet reuses [`ServitorOf`] for the owner link and follow state, so it
/// inherits follow/attack/leash from the servitor AI — "owned summon" is the
/// same relationship whether it came from a skill or a collar. Its own state
/// (collar, food bar) lives in `PetOf`.
/// Fold a live pet's state back into its owner's `PlayerPets` map (Java
/// `Pet.storeMe`, minus the DB write — the row rides out with the character's
/// next flush).
///
/// Called before every character save and on unsummon. A no-op when the player
/// has no pet out, which is the common case, so it is cheap to call
/// unconditionally rather than tracking a dirty flag.
pub(crate) fn sync_pet_row(world: &mut World, owner_oid: i32) {
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        return;
    };
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    let (cur_hp, cur_mp) = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .map(|v| (v.cur_hp, v.cur_mp))
        .unwrap_or((0.0, 0.0));
    // Java stores `getName()`, which for an unnamed pet is the template name.
    let name = npc_name_or_empty(world, pet_oid);
    let row = crate::db::PetRow {
        collar_object_id: pet.collar_object_id,
        name,
        level: pet.level,
        cur_hp,
        cur_mp,
        exp: pet.exp,
        sp: pet.sp,
        fed: pet.fed,
        // The pet is alive in the world at this moment, so if the owner is on
        // their way out it should come back next login. `on_owner_leave_world`
        // calls this *before* the unsummon precisely so this reads true.
        restore: world.cfg.character.restore_pet_on_reconnect,
    };
    if let Some(pets) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerPets>(&owner_oid)
    {
        pets.0.insert(row.collar_object_id, row);
    }
}

pub(crate) fn summon_pet(world: &mut World, owner_oid: i32) -> Option<i32> {
    use crate::model::components::PetOf;
    use crate::network::server_packets::sm_ids;

    world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)?;
    // `if (player.hasPet() || player.isMounted())` → "You already have a pet."
    if pet_of(world, owner_oid).is_some()
        || world
            .objects
            .get_component::<crate::model::Player>(&owner_oid)
            .is_some_and(crate::model::Player::is_mounted)
    {
        send_sm_bare_to_player(world, owner_oid, sm_ids::YOU_ALREADY_HAVE_A_PET);
        return None;
    }
    // Java logs and bails when the holder is missing — the effect was reached
    // without going through the item handler.
    let collar_object_id = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner_oid)
        .and_then(|p| p.pending_pet_collar.take())?;

    // The collar must still be in the owner's inventory (Java re-checks).
    let collar_item_id = item_id_of(world, owner_oid, collar_object_id)?;

    let npc_id = world.data.pet_data.by_item_id(collar_item_id)?.npc_id;

    // Java `Pet.restore`: the saved row keyed by this collar, or a fresh pet.
    // Here the row is already in memory (`PlayerPets`, loaded at login).
    let saved = world
        .objects
        .get_component::<crate::model::components::PlayerPets>(&owner_oid)
        .and_then(|p| p.0.get(&collar_object_id).cloned());

    let owner_level = world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)
        .map(|p| p.level)
        .unwrap_or(1);
    let (template_level, display_id) = world
        .data
        .npc_data
        .get(npc_id)
        .map(|t| (t.level, t.display_id))
        .unwrap_or((1, npc_id));

    let level = match &saved {
        Some(row) => row.level,
        // `new Pet(template, owner, control)`: the Sin Eater (display id 12564)
        // is summoned at its *owner's* level; every other species starts at its
        // template level.
        None if display_id == SIN_EATER_DISPLAY_ID => owner_level,
        None => template_level,
    };
    let (level, max_fed, exp_floor) = {
        let t = world.data.pet_data.by_item_id(collar_item_id)?;
        // `Math.max(level, getPetMinLevel(id))` — see `PetTemplate::min_level`.
        let level = level.max(t.min_level());
        (level, t.max_meal(level), t.exp_for_level(level))
    };

    // Java spawns the pet beside its owner, not on top of them.
    let pos = position(world, owner_oid)?;
    let pet_oid = crate::model::npc::spawn_npc_at(
        world,
        npc_id,
        pos.x + 50,
        pos.y + 100,
        pos.z,
        pos.heading,
    )?;

    world.objects.add_components(
        &pet_oid,
        ServitorOf {
            owner_object_id: owner_oid,
            // A pet is not tied to a skill and never expires or pays upkeep —
            // it is fed instead, which `PetOf` tracks.
            reference_skill: 0,
            expires_at_tick: u64::MAX,
            life_time_secs: 0,
            following: true,
            consume_item_id: 0,
            consume_item_count: 0,
            next_consume_tick: u64::MAX,
        },
    );
    world.objects.add_components(
        &pet_oid,
        PetOf {
            collar_object_id,
            fed: saved
                .as_ref()
                .map(|r| r.fed.min(max_fed))
                .unwrap_or(max_fed),
            max_fed,
            level,
            // "DS: update experience based by level. Avoiding pet delevels due
            // to exp per level values changed." — a stored exp below what this
            // level now costs is raised to the level's floor, so a retuned
            // datapack curve can't demote a pet the player already levelled.
            exp: saved
                .as_ref()
                .map(|r| r.exp.max(exp_floor))
                .unwrap_or(exp_floor),
            sp: saved.as_ref().map(|r| r.sp).unwrap_or(0),
            exp_before_death: 0,
        },
    );

    // Stats first: they set max HP/MP, which the vitals below are measured
    // against. A pet's stats come from its per-level pet row, not the NPC
    // template, so this must run before either branch.
    recalculate_pet_stats(world, pet_oid);

    // A fresh pet spawns full; a restored one keeps the vitals it was stored
    // with (Java `setCurrentHp/Mp` from the row).
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid) {
        match &saved {
            Some(row) => {
                v.cur_hp = row.cur_hp.min(v.max_hp as f64);
                v.cur_mp = row.cur_mp.min(v.max_mp as f64);
            }
            None => {
                v.cur_hp = v.max_hp as f64;
                v.cur_mp = v.max_mp as f64;
            }
        }
    }
    // Java's restore marks a pet stored with `curHp < 1` as dead
    // (`setDead(true)` + `stopHpMpRegeneration()`) and summons the corpse.
    // Reachable now that pets can die (slice 14).
    if saved.as_ref().is_some_and(|r| r.cur_hp < 1.0)
        && let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid)
    {
        v.dead = true;
        v.cur_hp = 0.0;
    }
    set_summon_link(world, owner_oid, None, Some(pet_oid), true);
    // Java `Pet.spawnMe` → `startFeed()`: the food clock runs from summon.
    start_feed(world, pet_oid);
    send_pet_info(world, owner_oid, pet_oid, PetInfoKind::Summoned);
    broadcast_summon_info(world, pet_oid, true);
    send_pet_item_list(world, owner_oid);
    // `ai/others/Servitors/SinEater.onSummonSpawn` — the one pet with a voice.
    crate::scripts::sin_eater::on_spawn(world, pet_oid);
    Some(pet_oid)
}

// ---------------------------------------------------------------------------
// Pet feeding (slice 8)
// ---------------------------------------------------------------------------

/// Java `Pet.FeedTask`'s fixed period: `scheduleAtFixedRate(..., 10000, 10000)`.
const FEED_TICK_SECS: u64 = 10;

/// Arm the feed chain for a freshly summoned pet (Java `startFeed`).
pub(crate) fn start_feed(world: &mut World, pet_oid: i32) {
    world.scheduler.schedule(
        world.tick + FEED_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::PetFeedTick { pet_oid },
    );
}

/// Java `Pet.isHungry()` — below `hungryLimit`% of the level's `maxMeal`.
/// A hungry pet is what triggers auto-eating; it is *not* the same as
/// [`is_uncontrollable`].
pub(crate) fn is_hungry(world: &World, pet_oid: i32) -> bool {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
    else {
        return false;
    };
    let limit = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .map(|t| t.hungry_limit)
        .unwrap_or(0);
    (pet.fed as f64) < (limit as f64 / 100.0) * pet.max_fed as f64
}

/// Java `Pet.isUncontrollable()` — a starving (empty-bar) pet stops obeying.
pub(crate) fn is_uncontrollable(world: &World, pet_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .is_some_and(|p| p.fed <= 0)
}

fn npc_template_id(world: &World, oid: i32) -> Option<i32> {
    npc_id_of(world, oid)
}

/// `effecthandlers/Feed.instant` for a pet: `setCurrentFed(fed + normal * rate)`.
///
/// `setCurrentFed` clamps at `getMaxFed()`, so over-feeding is capped rather
/// than banked — that clamp is why a "feeding restores N" test must measure
/// from a bar with room in it.
pub(crate) fn apply_feed(world: &mut World, pet_oid: i32, normal: i32) {
    let rate = world.cfg.rates.pet_food_rate;
    if let Some(pet) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        pet.fed = (pet.fed + normal * rate).min(pet.max_fed);
    }
}

/// Java `Pet.FeedTask.run()` — burn one interval's food, then let the pet eat
/// from its own inventory if it's hungry.
pub(crate) fn handle_feed_tick(world: &mut World, pet_oid: i32) {
    use crate::network::server_packets::{SmParam, sm_ids};

    // "dead or gone → the chain ends", the same contract the life tick uses.
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    if is_dead(world, pet_oid) {
        return;
    }
    let Some(owner) = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)
        .map(|s| s.owner_object_id)
    else {
        return;
    };

    // `_curFed > getFeedConsume() ? fed - consume : 0` — note Java burns the
    // *battle* rate while attacking.
    let (normal_cost, battle_cost) = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .and_then(|t| t.levels.get(&pet.level))
        .map(|l| (l.consume_meal_in_normal, l.consume_meal_in_battle))
        .unwrap_or((0, 0));
    // Java's `isAttackingNow()` — the battle rate applies mid-swing.
    let attacking = world
        .objects
        .get_component::<crate::model::components::AttackState>(&pet_oid)
        .is_some_and(|a| world.tick < a.attack_end_tick);
    let cost = if attacking { battle_cost } else { normal_cost };
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        p.fed = if p.fed > cost { p.fed - cost } else { 0 };
    }

    // Auto-eat: the food lives in the *pet's* inventory, not the owner's.
    let food_id = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .map(|t| t.food_item_id)
        .unwrap_or(0);
    let has_food = food_id > 0
        && world
            .objects
            .get_component::<crate::model::inventory::PetInventory>(&owner)
            .is_some_and(|pi| pi.0.count_of(food_id) > 0);

    if is_hungry(world, pet_oid) && has_food {
        // `handler.useItem(pet, food, false)` → destroy one, apply the skill.
        if let Some(pi) = world
            .objects
            .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
        {
            pi.0.remove_item(food_id, 1);
        }
        for (skill_id, skill_level) in item_skills(world, food_id) {
            apply_food_skill(world, pet_oid, skill_id, skill_level);
        }
        notify_owner(
            world,
            owner,
            sm_ids::YOUR_PET_WAS_HUNGRY_SO_IT_ATE_S1,
            &[SmParam::ItemName(food_id)],
        );
        send_pet_item_list(world, owner);
        // Still hungry after one helping — Java says so explicitly.
        if is_hungry(world, pet_oid) {
            notify_owner(
                world,
                owner,
                sm_ids::YOUR_PET_ATE_A_LITTLE_BUT_IS_STILL_HUNGRY,
                &[],
            );
        }
    } else if is_uncontrollable(world, pet_oid) {
        // Java `deleteMe` only when the species has *no* food ids at all;
        // otherwise it nags. A starving pet with a defined food item keeps
        // sulking until fed rather than vanishing.
        if food_id == 0 {
            notify_owner(world, owner, sm_ids::THE_PET_IS_NOW_LEAVING, &[]);
            sync_pet_row(world, owner);
            unsummon_servitor(world, owner);
            return;
        }
        notify_owner(world, owner, sm_ids::YOUR_PET_IS_STARVING, &[]);
    } else if is_hungry(world, pet_oid) {
        notify_owner(
            world,
            owner,
            sm_ids::THERE_IS_NOT_MUCH_TIME_REMAINING_UNTIL_THE_PET_LEAVES,
            &[],
        );
    }

    send_pet_info(world, owner, pet_oid, PetInfoKind::Default);
    world.scheduler.schedule(
        world.tick + FEED_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::PetFeedTick { pet_oid },
    );
}

/// `PetItemList` for the owner — the pet's inventory is only ever shown to the
/// player who owns it.
pub(crate) fn send_pet_item_list(world: &World, owner_oid: i32) {
    let Some(client_id) = client_for_player(world, owner_oid) else {
        return;
    };
    let Some(cs) = world.clients.get(&client_id) else {
        return;
    };
    let Some(pi) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner_oid)
    else {
        return;
    };
    cs.send(server_packets::pet_item_list(&pi.0, &world.data));
}

/// The `NORMAL` item-skill list Java's `PetFood` handler runs.
fn item_skills(world: &World, item_id: i32) -> Vec<(i32, i32)> {
    world
        .data
        .item_data
        .get(item_id)
        .map(|t| t.item_skills.clone())
        .unwrap_or_default()
}

/// Run one food skill's effects on the pet. Only `Feed` is meaningful today;
/// going through the skill (rather than hard-coding a bar bump) is what lets a
/// food item that also heals work when those effects land.
fn apply_food_skill(world: &mut World, pet_oid: i32, skill_id: i32, skill_level: i32) {
    let Some(skill) = world.data.skill_data.get(skill_id, skill_level) else {
        return;
    };
    for effect in skill.effects.clone() {
        if let crate::model::skill::SkillEffect::Feed { normal, .. } = effect {
            apply_feed(world, pet_oid, normal);
        }
    }
}

/// `RequestGiveItemToPet` (0x95) — move an item from the owner's inventory into
/// the pet's. This is how food reaches the pet at all: Java's `PetFood` handler
/// refuses an unmounted *player*, so the owner cannot eat it on the pet's
/// behalf.
pub(crate) fn handle_give_item_to_pet(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((object_id, amount)) = read_oid_and_count(body) else {
        return;
    };
    let Some(owner) = player_for_client(world, client_id) else {
        return;
    };
    if amount <= 0 || pet_of(world, owner).is_none() {
        return;
    }
    // Java refuses to hand over equipped gear or a quest item.
    let Some((item_id, held)) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&owner)
        .and_then(|inv| inv.by_object_id(object_id).map(|i| (i.item_id, i.count)))
    else {
        return;
    };
    if world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&owner)
        .is_some_and(|inv| inv.paperdoll_slot_of(object_id).is_some())
    {
        return;
    }
    if world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_quest_item)
    {
        return;
    }
    // The collar itself must not go into the pet it summons — Java blocks it,
    // and it would otherwise be unreachable when the pet is unsummoned.
    if world.data.pet_data.is_pet_collar(item_id) {
        return;
    }
    // Java: asking for more than the stack holds punishes.
    if amount > held {
        super::punishment::illegal_action(
            world,
            owner,
            &format!(
                "RequestGiveItemToPet: player {owner} tried to give item with oid {object_id} to pet but has invalid count {amount} item count: {held}"
            ),
        );
        return;
    }
    let count = amount.min(held);
    let changes = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&owner)
        .map(|inv| inv.remove_item(item_id, count))
        .unwrap_or_default();
    let Some(next_oid) = world.alloc_object_id() else {
        return;
    };
    let World { data, objects, .. } = world;
    if let Some(pi) = objects.get_component_mut::<crate::model::inventory::PetInventory>(&owner) {
        pi.0.add_item(&data.item_data, next_oid, item_id, count);
    }
    let packet = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
    crate::game_loop::helpers::send_inventory_update(world, client_id, owner, packet);
    send_pet_item_list(world, owner);
}

/// `RequestGetItemFromPet` (0x2C) — the reverse transfer.
pub(crate) fn handle_get_item_from_pet(world: &mut World, client_id: u32, body: &[u8]) {
    let Some((object_id, amount)) = read_oid_and_count(body) else {
        return;
    };
    let Some(owner) = player_for_client(world, client_id) else {
        return;
    };
    if amount <= 0 || pet_of(world, owner).is_none() {
        return;
    }
    let Some((item_id, held)) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner)
        .and_then(|pi| pi.0.by_object_id(object_id).map(|i| (i.item_id, i.count)))
    else {
        return;
    };
    // Java: asking for more than the stack holds punishes.
    if amount > held {
        super::punishment::illegal_action(
            world,
            owner,
            &format!(
                "RequestGetItemFromPet: player {owner} tried to get item with oid {object_id} from pet but has invalid count {amount} item count: {held}"
            ),
        );
        return;
    }
    let count = amount.min(held);
    if let Some(pi) = world
        .objects
        .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
    {
        pi.0.remove_item(item_id, count);
    }
    let Some(next_oid) = world.alloc_object_id() else {
        return;
    };
    let World { data, objects, .. } = world;
    let changes = objects
        .get_component_mut::<crate::model::inventory::Inventory>(&owner)
        .map(|inv| {
            let oid = inv.add_item(&data.item_data, next_oid, item_id, count);
            inv.by_object_id(oid)
                .cloned()
                .map(crate::model::inventory::ItemChange::Modified)
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let packet = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
    crate::game_loop::helpers::send_inventory_update(world, client_id, owner, packet);
    send_pet_item_list(world, owner);
}

/// `RequestPetUseItem` (0x94) — the owner clicks an item in the pet's window.
/// Only the `PetFood` handler is ported; anything else is ignored rather than
/// silently consumed.
pub(crate) fn handle_pet_use_item(world: &mut World, client_id: u32, body: &[u8]) {
    use crate::network::server_packets::sm_ids;
    if body.len() < 4 {
        return;
    }
    let object_id = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let Some(owner) = player_for_client(world, client_id) else {
        return;
    };
    let Some(pet_oid) = pet_of(world, owner) else {
        return;
    };
    let Some(item_id) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner)
        .and_then(|pi| pi.0.by_object_id(object_id).map(|i| i.item_id))
    else {
        return;
    };

    // Java `RequestPetUseItem`: an **equippable** item is worn rather than
    // consumed (`useEquippableItem`), which is how a battle pet gets its
    // armour. 96 pet-armour items ship on this dist.
    if world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_equipable())
    {
        equip_pet_item(world, owner, pet_oid, object_id);
        return;
    }

    // `if (playable.isPet() && !canEatFoodId(item.getId()))` → refuse.
    let eats = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .is_some_and(|t| t.food_item_id == item_id);
    if !eats {
        notify_owner(world, owner, sm_ids::THIS_PET_CANNOT_USE_THIS_ITEM, &[]);
        return;
    }

    if let Some(pi) = world
        .objects
        .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
    {
        pi.0.remove_item(item_id, 1);
    }
    for (skill_id, skill_level) in item_skills(world, item_id) {
        apply_food_skill(world, pet_oid, skill_id, skill_level);
    }
    if is_hungry(world, pet_oid) {
        notify_owner(
            world,
            owner,
            sm_ids::YOUR_PET_ATE_A_LITTLE_BUT_IS_STILL_HUNGRY,
            &[],
        );
    }
    send_pet_item_list(world, owner);
    send_pet_info(world, owner, pet_oid, PetInfoKind::Default);
}

/// `(objectId: i32, count: i64)` — the layout both transfer packets share.
fn read_oid_and_count(body: &[u8]) -> Option<(i32, i64)> {
    if body.len() < 12 {
        return None;
    }
    let oid = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let count = i64::from_le_bytes(body[4..12].try_into().ok()?);
    Some((oid, count))
}

fn player_for_client(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Pet experience (slice 12)
// ---------------------------------------------------------------------------

/// Java `Config.ALT_PARTY_RANGE` — the pet only earns while it is near enough
/// to its owner to have plausibly helped.
const PET_EXP_RANGE: f64 = 1500.0;

/// The owner's share of a kill's exp/sp, and the pet's cut, as Java's
/// `PlayerStat.addExpAndSp` computes it.
///
/// `get_exp_type` is the **owner's** percentage (73 on most species), so the
/// pet takes the remainder. The owner's own award is then multiplied by that
/// same ratio — the pet's exp is taken *from* the owner, not minted on top, so
/// hunting with a pet genuinely costs the player exp.
///
/// Returns `(owner_ratio, pet_exp, pet_sp)`. `owner_ratio` is 1.0 with no
/// eligible pet, which leaves the owner's award untouched.
pub(crate) fn split_exp_with_pet(
    world: &World,
    owner_oid: i32,
    exp: f64,
    sp: f64,
) -> (f64, f64, f64) {
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        return (1.0, 0.0, 0.0);
    };
    // A dead pet earns nothing (Java `if (!pet.isDead())`), but note the
    // owner's ratio is still reduced — Java adjusts it outside that guard, so
    // the exp is lost rather than returned to the player. Faithful.
    if is_dead(world, pet_oid) {
        return (1.0, 0.0, 0.0);
    }
    if !within(world, owner_oid, pet_oid, PET_EXP_RANGE) {
        return (1.0, 0.0, 0.0);
    }
    let level = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .map(|p| p.level)
        .unwrap_or(1);
    let owner_taken = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .and_then(|t| t.levels.get(&level))
        .map(|l| l.owner_exp_taken)
        .unwrap_or(100);
    // "allow possible customizations that would have the pet earning more
    // than 100% of the owner's exp/sp" — but never a negative owner award.
    let ratio = (owner_taken as f64 / 100.0).min(1.0);
    (ratio, exp * (1.0 - ratio), sp * (1.0 - ratio))
}

fn within(world: &World, a: i32, b: i32, range: f64) -> bool {
    crate::geo::distance::within_3d(world, a, b, range)
}

/// Java `PetStat.addExpAndSp` — award the pet its cut and level it up.
///
/// **A starving pet earns nothing** (`isUncontrollable()` guards `addExp`),
/// which is a real link between the feeding loop and progression rather than
/// an incidental check.
pub(crate) fn add_pet_exp(world: &mut World, owner_oid: i32, exp: f64, sp: f64) {
    use crate::network::server_packets::{SmParam, sm_ids};
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        return;
    };
    if is_uncontrollable(world, pet_oid) {
        return;
    }
    let gained = exp.round() as i64;
    if gained <= 0 && sp.round() as i64 <= 0 {
        return;
    }
    let max_level = max_pet_level(world, pet_oid);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        p.exp += gained.max(0);
        p.sp += (sp.round() as i64).max(0);
    }
    notify_owner(
        world,
        owner_oid,
        sm_ids::YOUR_PET_GAINED_S1_XP,
        &[SmParam::Int(gained as i32)],
    );
    level_up_pet(world, owner_oid, pet_oid, max_level);
    send_pet_info(world, owner_oid, pet_oid, PetInfoKind::Default);
}

/// The highest level this species has a row for. Java caps at
/// `ExperienceData.getMaxPetLevel() - 1`; here the species table is the
/// authority, and it is what every per-level lookup needs anyway.
fn max_pet_level(world: &World, pet_oid: i32) -> i32 {
    npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .and_then(|t| t.levels.keys().copied().max())
        .unwrap_or(1)
}

/// Advance the pet through every level its new exp total has earned.
fn level_up_pet(world: &mut World, owner_oid: i32, pet_oid: i32, max_level: i32) {
    let Some(npc_id) = npc_template_id(world, pet_oid) else {
        return;
    };
    let mut levelled = false;
    loop {
        let Some(pet) = world
            .objects
            .get_component::<crate::model::components::PetOf>(&pet_oid)
            .copied()
        else {
            return;
        };
        if pet.level >= max_level {
            break;
        }
        let next = pet.level + 1;
        let needed = world
            .data
            .pet_data
            .get(npc_id)
            .map(|t| t.exp_for_level(next))
            .unwrap_or(i64::MAX);
        if needed <= 0 || pet.exp < needed {
            break;
        }
        // The food bar's capacity is per level, so it moves with the level.
        let new_max_fed = world
            .data
            .pet_data
            .get(npc_id)
            .map(|t| t.max_meal(next))
            .unwrap_or(pet.max_fed);
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
        {
            p.level = next;
            p.max_fed = new_max_fed;
            p.fed = p.fed.min(new_max_fed);
        }
        levelled = true;
    }
    if levelled {
        // The new level's stat row is what makes levelling mean anything.
        recalculate_pet_stats(world, pet_oid);
    }
    if levelled {
        // Java sends no system message for a pet level — just the animation.
        let pkt = crate::network::server_packets::social_action(pet_oid, SOCIAL_LEVEL_UP);
        crate::game_loop::helpers::broadcast_including_self(world, owner_oid, &pkt);
        sync_collar_enchant(world, owner_oid, pet_oid);
    }
}

/// `getControlItem().setEnchantLevel(getLevel())` — the collar's enchant level
/// *is* the pet's level, which is how the client shows "Wolf Collar +12" and
/// how a traded pet advertises what it is without being summoned.
/// Admin entry (`//summon_setlvl`) for the collar-enchant sync below.
pub(crate) fn sync_collar_enchant_for_admin(world: &mut World, owner_oid: i32, pet_oid: i32) {
    sync_collar_enchant(world, owner_oid, pet_oid);
}

fn sync_collar_enchant(world: &mut World, owner_oid: i32, pet_oid: i32) {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    if let Some(inv) = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&owner_oid)
    {
        inv.set_item_enchant(pet.collar_object_id, pet.level);
    }
}

/// `SocialAction.LEVEL_UP`.
const SOCIAL_LEVEL_UP: i32 = 15;

// ---------------------------------------------------------------------------
// Pet stats (slice 13)
// ---------------------------------------------------------------------------

/// A pet's NPC template with its **per-level pet stats substituted in**.
///
/// Java does this at the finalizer level: `MaxHpFinalizer`, `PDefenseFinalizer`,
/// `MDefenseFinalizer`, `MaxMpFinalizer` and
/// `IStatFunction.calcWeaponBaseValue` each check `isPet()` and read
/// `getPetLevelData()` **instead of** the template base, then run the *same*
/// bonus math. Substituting the bases up front reproduces that exactly while
/// reusing the whole existing NPC stat pipeline (STR/INT/CON/MEN bonuses,
/// levelMod, the m.atk 2.2072 power, passive skills, buffs).
///
/// The **level is substituted too** — a pet's `levelMod` follows its own level,
/// not the NPC template's, which is most of why a levelled pet gets stronger.
fn pet_template_at_level(
    t: &crate::data::npc_data::NpcTemplate,
    row: &crate::data::pet_data::PetLevel,
    level: i32,
) -> crate::data::npc_data::NpcTemplate {
    let mut t = t.clone();
    t.level = level;
    // A row that does not carry a given stat keeps the NPC template's value
    // rather than substituting a zero. Java reads the pet row unconditionally
    // and every shipped species populates all of these, so this never fires on
    // real data — but a single missing `org_hp` would otherwise give the pet
    // **0 max HP**, and it is not worth losing a pet to a datapack typo.
    let or_template = |v: f64, fallback: f64| if v > 0.0 { v } else { fallback };
    t.base_p_atk = or_template(row.p_atk, t.base_p_atk);
    t.base_m_atk = or_template(row.m_atk, t.base_m_atk);
    t.base_p_def = or_template(row.p_def, t.base_p_def);
    t.base_m_def = or_template(row.m_def, t.base_m_def);
    t.base_hp_max = or_template(row.max_hp, t.base_hp_max);
    t.base_mp_max = or_template(row.max_mp, t.base_mp_max);
    t
}

/// Recompute a live pet's stats for its current level, preserving the HP/MP
/// *fraction* across a max-HP change so levelling neither heals nor wounds it.
pub(crate) fn recalculate_pet_stats(world: &mut World, pet_oid: i32) {
    let Some(level) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .map(|p| p.level)
    else {
        return;
    };
    let Some(npc_id) = npc_template_id(world, pet_oid) else {
        return;
    };
    let Some(row) = world
        .data
        .pet_data
        .get(npc_id)
        .and_then(|t| t.levels.get(&level).cloned())
    else {
        return;
    };
    let Some(template) = world.data.npc_data.get(npc_id).cloned() else {
        return;
    };
    let petted = pet_template_at_level(&template, &row, level);

    let buffs = world
        .objects
        .get_component::<crate::model::components::Buffs>(&pet_oid)
        .cloned()
        .unwrap_or_default();
    let (mut combat, speeds, max_hp, max_mp) =
        // A pet is a `Summon`, not an `Attackable`, so it can never be a
        // champion — neutral mods.
        crate::model::npc_finalized_stats(
            &world.data,
            &petted,
            &buffs,
            crate::model::ChampionStatMods::default(),
        );

    // A pet's worn armour adds to its defences. Java runs pets through the same
    // finalizers as everyone else, which sum the paperdoll; the port's NPC
    // pipeline has no inventory step, so the sum is done here against the
    // **pet's own** paperdoll (`PetInventory`, held on the owner).
    //
    // Only the defensive stats are folded: the 96 pet-armour items on this dist
    // are armour, and a pet has no weapon slot to speak of.
    let owner = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)
        .map(|s| s.owner_object_id);
    if let Some(owner) = owner
        && let Some(pi) = world
            .objects
            .get_component::<crate::model::inventory::PetInventory>(&owner)
    {
        for item in pi.0.equipped_items() {
            let Some(stats) = world.data.item_data.item_stats(item.item_id) else {
                continue;
            };
            for &(stat, val) in &stats.bonuses {
                match stat {
                    crate::model::stats::Stat::PhysicalDefence => combat.p_def += val,
                    crate::model::stats::Stat::MagicalDefence => combat.m_def += val,
                    _ => {}
                }
            }
        }
    }

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid) {
        // Keep the bar where it was proportionally — Java's stat recompute does
        // not refill a pet on level-up.
        let hp_frac = if v.max_hp > 0 {
            v.cur_hp / v.max_hp as f64
        } else {
            1.0
        };
        let mp_frac = if v.max_mp > 0 {
            v.cur_mp / v.max_mp as f64
        } else {
            1.0
        };
        v.max_hp = max_hp.round() as i32;
        v.max_mp = max_mp.round() as i32;
        v.cur_hp = (v.max_hp as f64 * hp_frac).min(v.max_hp as f64);
        v.cur_mp = (v.max_mp as f64 * mp_frac).min(v.max_mp as f64);
    }
    world.objects.add_components(&pet_oid, combat);
    world.objects.add_components(&pet_oid, speeds);
}

// ---------------------------------------------------------------------------
// Pet death (slice 14)
// ---------------------------------------------------------------------------

/// Java `Pet.doDie` — the pet-specific half, called from the NPC death path
/// once a dying NPC turns out to be a pet.
///
/// Returns the owner so the caller can finish its own bookkeeping.
pub(crate) fn pet_do_die(world: &mut World, pet_oid: i32) -> Option<i32> {
    use crate::network::server_packets::sm_ids;
    let owner = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)?
        .owner_object_id;
    world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)?;

    // `if (owner != null && !owner.isInDuel() && (!isInsideZone(PVP) || isInsideZone(SIEGE)))`
    // — no exp is lost to a duel or an arena death.
    if !crate::game_loop::duel::is_in_duel(world, owner) {
        // `SinEater`'s `ON_CREATURE_DEATH` bark, before the penalty maths.
        crate::scripts::sin_eater::on_death(world, pet_oid);
        pet_death_penalty(world, pet_oid);
    }

    // `stopFeed()` — the food clock stops with the pet. The scheduled tick
    // checks `dead` and ends its own chain, so there is nothing to cancel.
    notify_owner(world, owner, sm_ids::THE_PET_HAS_BEEN_KILLED, &[]);
    // The pet's state is captured now: the corpse can decay or be resurrected,
    // but either way the exp penalty is already what should persist.
    sync_pet_row(world, owner);
    send_pet_info(world, owner, pet_oid, PetInfoKind::Default);
    Some(owner)
}

/// Java `Pet.deathPenalty`, whose own comment admits the penalty is a guess
/// ("Need Correct Penalty") — ported as written.
///
/// `percentLost = -0.07 × level + 6.5`, applied to the size of the pet's
/// *current* level band — so the loss is a share of one level's worth of exp,
/// and it shrinks as the pet levels.
fn pet_death_penalty(world: &mut World, pet_oid: i32) {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    let Some(npc_id) = npc_template_id(world, pet_oid) else {
        return;
    };
    let (this_level, next_level) = {
        let Some(t) = world.data.pet_data.get(npc_id) else {
            return;
        };
        (t.exp_for_level(pet.level), t.exp_for_level(pet.level + 1))
    };
    let band = (next_level - this_level).max(0) as f64;
    let percent_lost = (-0.07 * pet.level as f64) + 6.5;
    let lost = ((band * percent_lost) / 100.0).round() as i64;

    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        // Captured *before* the penalty — `restoreExp` gives back a share of
        // the gap between this and the post-penalty total.
        p.exp_before_death = p.exp;
        // Java's `addExp(-lostExp)` cannot take a pet below its level floor.
        p.exp = (p.exp - lost).max(this_level);
    }
}

/// Java `Pet.restoreExp(restorePercent)` — hand back a share of what the death
/// penalty took. Called from the resurrection path with the skill's power.
pub(crate) fn pet_restore_exp(world: &mut World, pet_oid: i32, restore_percent: f64) {
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };
    if pet.exp_before_death <= 0 {
        return;
    }
    let regained =
        (((pet.exp_before_death - pet.exp) as f64 * restore_percent) / 100.0).round() as i64;
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::PetOf>(&pet_oid)
    {
        p.exp += regained.max(0);
        // One resurrection consumes the record — a second revive restores
        // nothing, as in Java.
        p.exp_before_death = 0;
    }
}

/// `Summon.onDecay` → `unSummon(owner)` + `Pet.deleteMe(owner)` for a pet whose
/// corpse has decayed.
///
/// **This destroys the pet permanently.** Java's `deleteMe` is:
///
/// ```java
/// _inventory.transferItemsToOwner();
/// super.deleteMe(owner);
/// destroyControlItem(owner, false); // "this should also delete the pet from the db"
/// ```
///
/// So letting a dead pet rot costs the player the collar *and* everything the
/// pet was carrying stays only because the inventory is handed back first. The
/// corpse lasts `DefaultCorpseTime` — **7 seconds** on this dist, since no pet
/// NPC template overrides `corpseTime` and `DecayTaskManager` has no pet
/// branch. (The "24 hours" in the death message is flavour text that does not
/// match the mechanic; checked against the datapack rather than trusted.)
pub(crate) fn pet_decay(world: &mut World, pet_oid: i32) {
    let Some(owner) = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)
        .map(|s| s.owner_object_id)
    else {
        return;
    };
    let Some(pet) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .copied()
    else {
        return;
    };

    // `_inventory.transferItemsToOwner()` — the pet's bag is handed back
    // before the collar goes, so its contents are not lost with it.
    let carried: Vec<(i32, i64)> = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&owner)
        .map(|pi| pi.0.items().iter().map(|i| (i.item_id, i.count)).collect())
        .unwrap_or_default();
    if let Some(pi) = world
        .objects
        .get_component_mut::<crate::model::inventory::PetInventory>(&owner)
    {
        pi.0 = Default::default();
    }
    for (item_id, count) in carried {
        let Some(oid) = world.alloc_object_id() else {
            break;
        };
        let World { data, objects, .. } = world;
        if let Some(inv) = objects.get_component_mut::<crate::model::inventory::Inventory>(&owner) {
            inv.add_item(&data.item_data, oid, item_id, count);
        }
    }

    // `destroyControlItem` — the collar is consumed, and with it the pet's
    // identity: the saved row is keyed by that object id.
    let collar = pet.collar_object_id;
    let removed = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&owner)
        .and_then(|inv| inv.remove_by_object_id(collar, 1));
    if let Some(change) = removed
        && let Some(cid) = client_for_player(world, owner)
    {
        let packet = crate::network::enter_world::inventory_update_changes(&world.data, &[change]);
        crate::game_loop::helpers::send_inventory_update(world, cid, owner, packet);
    }
    world
        .objects
        .get_component_mut::<crate::model::components::PlayerPets>(&owner)
        .map(|p| p.0.remove(&collar));
    let _ = world.db.send(crate::db::DbCommand::DeletePetRow {
        collar_object_id: collar,
    });

    // The owner has no pet any more.
    set_summon_link(world, owner, None, None, true);
    send_pet_item_list(world, owner);
}

// ---------------------------------------------------------------------------
// Summon shots (slice 18)
// ---------------------------------------------------------------------------

/// Java `Summon.rechargeShots` — charge a summon from its **owner's** Beast
/// shots before it swings.
///
/// The owner's auto-shot list is the switch: a Beast Soulshot only fires if the
/// player toggled it on. Each charge costs `soulshot_count` from the *pet's
/// current level row*, so a high-level pet is markedly more expensive to keep
/// shotted — which is the mechanic, not an incidental detail.
///
/// Returns true when the summon ends up charged.
pub(crate) fn recharge_shots(world: &mut World, summon_oid: i32, physical: bool) -> bool {
    use crate::data::item_data::ActionType;
    use crate::model::components::ChargedShots;

    let already = world
        .objects
        .get_component::<ChargedShots>(&summon_oid)
        .is_some_and(|c| c.soulshot);
    if already || !physical {
        return already;
    }
    let Some(owner) = world
        .objects
        .get_component::<ServitorOf>(&summon_oid)
        .map(|s| s.owner_object_id)
    else {
        return false;
    };
    // How many the swing costs: from the pet's level row. A servitor has no
    // pet row, so it uses one — Java reads `getSoulShotsPerHit()`, which for a
    // plain servitor is its template's.
    let per_hit = world
        .objects
        .get_component::<crate::model::components::PetOf>(&summon_oid)
        .and_then(|p| {
            npc_template_id(world, summon_oid)
                .and_then(|id| world.data.pet_data.get(id))
                .and_then(|t| t.levels.get(&p.level))
                .map(|l| l.soulshot_count)
        })
        .unwrap_or(1)
        .max(1) as i64;

    // Java iterates the owner's auto-shot list and picks the entries whose
    // `default_action` marks them as *summon* shots.
    let shots: Vec<i32> = world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .map(|p| p.auto_shots.clone())
        .unwrap_or_default();
    for item_id in shots {
        let is_summon_soulshot = world.data.item_data.get(item_id).map(|t| t.default_action)
            == Some(ActionType::SummonSoulshot);
        if !is_summon_soulshot {
            continue;
        }
        let have = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&owner)
            .map(|inv| inv.count_of(item_id))
            .unwrap_or(0);
        if have < per_hit {
            // Java drops the toggle when the item runs out entirely.
            if have == 0
                && let Some(p) = world
                    .objects
                    .get_component_mut::<crate::model::Player>(&owner)
            {
                p.auto_shots.retain(|&id| id != item_id);
            }
            continue;
        }
        let changes = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&owner)
            .map(|inv| inv.remove_item(item_id, per_hit))
            .unwrap_or_default();
        if let Some(cid) = client_for_player(world, owner) {
            let packet =
                crate::network::enter_world::inventory_update_changes(&world.data, &changes);
            crate::game_loop::helpers::send_inventory_update(world, cid, owner, packet);
        }
        if world
            .objects
            .get_component::<ChargedShots>(&summon_oid)
            .is_none()
        {
            world
                .objects
                .add_components(&summon_oid, ChargedShots::default());
        }
        if let Some(c) = world.objects.get_component_mut::<ChargedShots>(&summon_oid) {
            c.soulshot = true;
        }
        return true;
    }
    false
}

/// Spend a summon's charged soulshot (Java `unchargeShot(SOULSHOTS)`), which
/// happens on a landed hit only — a miss keeps the charge.
pub(crate) fn uncharge_soulshot(world: &mut World, summon_oid: i32) -> bool {
    use crate::model::components::ChargedShots;
    match world.objects.get_component_mut::<ChargedShots>(&summon_oid) {
        Some(c) if c.soulshot => {
            c.soulshot = false;
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Pet equipment (slice 25)
// ---------------------------------------------------------------------------

/// Equip or unequip an item in the pet's own paperdoll.
///
/// `PetInventory` wraps the ordinary `Inventory`, which already owns a
/// paperdoll and all the slot-displacement rules — so a pet's armour reuses the
/// player's equip logic wholesale rather than growing a second copy. Java does
/// the same: `PetInventory extends Inventory`.
///
/// Toggling matches Java's `useEquippableItem`: clicking a worn item takes it
/// off.
pub(crate) fn equip_pet_item(world: &mut World, owner_oid: i32, pet_oid: i32, object_id: i32) {
    let World { data, objects, .. } = world;
    let Some(pi) = objects.get_component_mut::<crate::model::inventory::PetInventory>(&owner_oid)
    else {
        return;
    };
    let worn = pi.0.paperdoll_slot_of(object_id).is_some();
    if worn {
        pi.0.unequip_item(object_id);
    } else {
        pi.0.equip_item(&data.item_data, object_id);
    }
    // Gear changes the pet's defences, so its stats and the client's view of
    // them both have to be rebuilt.
    recalculate_pet_stats(world, pet_oid);
    send_pet_item_list(world, owner_oid);
    send_pet_info(world, owner_oid, pet_oid, PetInfoKind::Default);
    broadcast_summon_info(world, pet_oid, false);
}

// ---------------------------------------------------------------------------
// Reconnect resummon (slice 26)
// ---------------------------------------------------------------------------

/// Java `CharSummonTable.restorePet` — bring back the pet that was out when the
/// owner logged off.
///
/// `RestorePetOnReconnect` is **True** on this dist, so this is the normal
/// path, not an opt-in. The saved row's `restore` flag is what marks a pet as
/// "was out"; a pet deliberately unsummoned before logout has it cleared, and
/// stays in its collar.
///
/// Called at enter-world, after the inventory exists — the collar has to be
/// found before the pet can be rebuilt from it.
pub(crate) fn restore_pet_on_login(world: &mut World, owner_oid: i32) {
    if !world.cfg.character.restore_pet_on_reconnect {
        return;
    }
    let collar = world
        .objects
        .get_component::<crate::model::components::PlayerPets>(&owner_oid)
        .and_then(|p| p.0.values().find(|r| r.restore).map(|r| r.collar_object_id));
    let Some(collar) = collar else { return };
    // The collar must still be there: it can have been traded or destroyed
    // between sessions, and `summon_pet` re-checks anyway — but setting the
    // holder for a collar that is gone would leave it dangling.
    let have_collar = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&owner_oid)
        .is_some_and(|inv| inv.items().iter().any(|i| i.object_id == collar));
    if !have_collar {
        return;
    }
    // Reuse the normal summon path rather than a parallel one, so a restored
    // pet is identical to a freshly summoned one — same stats, same feed clock,
    // same packets. It reads its state from the saved row exactly as it does
    // after a mid-session re-summon.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner_oid)
    {
        p.pending_pet_collar = Some(collar);
    }
    summon_pet(world, owner_oid);
}

/// Capture the owner's live servitor into `PlayerSummons` (Java's
/// `character_summons` write). The pet counterpart is `sync_pet_row`; this runs
/// in the same place, before the summon leaves the world.
pub(crate) fn sync_summon_row(world: &mut World, owner_oid: i32) {
    if !world.cfg.character.restore_servitor_on_reconnect {
        return;
    }
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        // Nothing out: clear any stale row, or a servitor dismissed before
        // logout would come back anyway.
        if let Some(s) = world
            .objects
            .get_component_mut::<crate::model::components::PlayerSummons>(&owner_oid)
        {
            s.0.clear();
        }
        return;
    };
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    // A servitor summoned with no lifetime (`lifeTime <= 0` → `u64::MAX`) has
    // nothing to count down; store 0 and let the re-cast decide again.
    let remaining_secs = if link.expires_at_tick == u64::MAX {
        0
    } else {
        ((link.expires_at_tick.saturating_sub(world.tick)) / TICKS_PER_SECOND) as i32
    };
    let (cur_hp, cur_mp) = world
        .objects
        .get_component::<Vitals>(&servitor_oid)
        .map(|v| (v.cur_hp as i32, v.cur_mp as i32))
        .unwrap_or((0, 0));
    // The servitor's own buffs go with it — a Summoner's investment in
    // buffing their servitor should survive a relog, which is exactly why Java
    // keeps `character_summon_skills_save`.
    let now = world.tick;
    let buffs = world
        .objects
        .get_component::<crate::model::components::Buffs>(&servitor_oid)
        .map(|b| {
            b.0.iter()
                .filter(|buf| buf.expires_at_tick > now)
                .map(|buf| crate::db::SkillBuffRow {
                    skill_id: buf.skill_id,
                    skill_level: buf.skill_level,
                    remaining_time_secs: ((buf.expires_at_tick - now) / TICKS_PER_SECOND) as i32,
                })
                .collect()
        })
        .unwrap_or_default();
    let row = crate::db::SummonRow {
        summon_skill_id: link.reference_skill,
        cur_hp,
        cur_mp,
        remaining_secs,
        buffs,
    };
    if world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&owner_oid)
        .is_none()
    {
        world.objects.add_components(
            &owner_oid,
            crate::model::components::PlayerSummons::default(),
        );
    }
    if let Some(s) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerSummons>(&owner_oid)
    {
        s.0.clear();
        s.0.push(row);
    }
}

/// Java `CharSummonTable.restoreServitor` — bring back the servitor that was
/// out when the owner logged off.
///
/// Java restores by **re-casting the summoning skill**
/// (`skill.applyEffects(player, player)`) and then stamping the saved vitals
/// and remaining lifetime onto the result. Doing the same here means a restored
/// servitor is built by the ordinary summon path, so it can never drift from a
/// freshly summoned one — and it comes back at the player's *current* level of
/// the skill, so levelling up between sessions is not punished.
pub(crate) fn restore_servitor_on_login(world: &mut World, owner_oid: i32) {
    if !world.cfg.character.restore_servitor_on_reconnect {
        return;
    }
    let Some(row) = world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&owner_oid)
        .and_then(|s| s.0.first().cloned())
    else {
        return;
    };
    // The row is consumed either way (Java `removeServitor` before the recast):
    // a skill the player no longer knows must not be retried every login.
    if let Some(s) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerSummons>(&owner_oid)
    {
        s.0.clear();
    }
    let Some(level) = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&owner_oid)
        .and_then(|b| b.0.get(&row.summon_skill_id).copied())
    else {
        return; // unlearned across a subclass change — nothing to restore
    };
    let Some(skill) = skill_by_id(world, row.summon_skill_id, level) else {
        return;
    };
    crate::game_loop::skills::effects::apply_skill_effects(world, owner_oid, owner_oid, &skill);

    // Stamp the saved state back over the fresh summon.
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return;
    };
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&servitor_oid) {
        v.cur_hp = (row.cur_hp as f64).clamp(1.0, v.max_hp as f64);
        v.cur_mp = (row.cur_mp as f64).clamp(0.0, v.max_mp as f64);
    }
    if row.remaining_secs > 0 {
        let expires = world.tick + (row.remaining_secs as u64) * TICKS_PER_SECOND;
        if let Some(s) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
            s.expires_at_tick = expires;
        }
    }
    // Its buffs come back too, through the same path the player's own
    // persisted buffs use — relative remaining time, frozen while offline.
    if !row.buffs.is_empty() {
        crate::game_loop::skills::effects::restore_persisted_buffs(world, servitor_oid, &row.buffs);
    }
    send_pet_info(world, owner_oid, servitor_oid, PetInfoKind::Summoned);
    broadcast_summon_info(world, servitor_oid, true);
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
                if let Some(cs) =
                    client_for_player(world, owner_oid).and_then(|c| world.clients.get(&c))
                {
                    cs.send(server_packets::system_message_with(
                        sm_ids::INVALID_TARGET,
                        &[],
                    ));
                }
                return;
            }
        }
    };

    if !crate::game_loop::npc_cast::check_use_conditions_pub(world, servitor_oid, &skill) {
        return;
    }
    crate::game_loop::npc_cast::start_cast(world, servitor_oid, target_oid, &skill);
}

/// Charge a summon's Beast Spiritshot from its owner — the magic counterpart of
/// [`recharge_shots`], costing the pet level's `spiritshot_count`.
pub(crate) fn recharge_spiritshots(world: &mut World, summon_oid: i32) -> bool {
    use crate::data::item_data::ActionType;
    use crate::model::components::ChargedShots;

    if world
        .objects
        .get_component::<ChargedShots>(&summon_oid)
        .is_some_and(|c| c.spiritshot)
    {
        return true;
    }
    let Some(owner) = world
        .objects
        .get_component::<ServitorOf>(&summon_oid)
        .map(|s| s.owner_object_id)
    else {
        return false;
    };
    let per_hit = world
        .objects
        .get_component::<crate::model::components::PetOf>(&summon_oid)
        .and_then(|p| {
            npc_template_id(world, summon_oid)
                .and_then(|id| world.data.pet_data.get(id))
                .and_then(|t| t.levels.get(&p.level))
                .map(|l| l.spiritshot_count)
        })
        .unwrap_or(1)
        .max(1) as i64;

    let shots: Vec<i32> = world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .map(|p| p.auto_shots.clone())
        .unwrap_or_default();
    for item_id in shots {
        if world.data.item_data.get(item_id).map(|t| t.default_action)
            != Some(ActionType::SummonSpiritshot)
        {
            continue;
        }
        let have = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&owner)
            .map(|inv| inv.count_of(item_id))
            .unwrap_or(0);
        if have < per_hit {
            continue;
        }
        let changes = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&owner)
            .map(|inv| inv.remove_item(item_id, per_hit))
            .unwrap_or_default();
        if let Some(cid) = client_for_player(world, owner) {
            let packet =
                crate::network::enter_world::inventory_update_changes(&world.data, &changes);
            crate::game_loop::helpers::send_inventory_update(world, cid, owner, packet);
        }
        if world
            .objects
            .get_component::<ChargedShots>(&summon_oid)
            .is_none()
        {
            world
                .objects
                .add_components(&summon_oid, ChargedShots::default());
        }
        if let Some(c) = world.objects.get_component_mut::<ChargedShots>(&summon_oid) {
            c.spiritshot = true;
        }
        return true;
    }
    false
}

/// Spend a summon's charged spiritshot. Unlike the soulshot — spent by a
/// landed swing — a magic shot is consumed by the **cast**, so this is called
/// from the effect path.
pub(crate) fn uncharge_spiritshot(world: &mut World, summon_oid: i32) -> bool {
    use crate::model::components::ChargedShots;
    match world.objects.get_component_mut::<ChargedShots>(&summon_oid) {
        Some(c) if c.spiritshot => {
            c.spiritshot = false;
            true
        }
        _ => false,
    }
}
