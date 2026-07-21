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
//! slices (see `docs/PLAN_G29_SERVITOR_SUMMON.md`).

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

use super::helpers::client_for_player;

/// Java `Player.getServitors()` — this port scans rather than caching a second
/// index, because a player has at most one servitor on this dist.
pub(crate) fn servitor_of(world: &mut World, owner_oid: i32) -> Option<i32> {
    let mut found = None;
    // `for_each_mut` is the store's only sweep; the query borrows shared.
    world.objects.for_each_mut::<(&crate::model::npc::Npc, &ServitorOf)>(|(npc, s)| {
        if s.owner_object_id == owner_oid {
            found = Some(npc.object_id);
        }
    });
    found
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
    world.objects.get_component::<crate::model::Player>(&owner_oid)?;
    unsummon_servitor(world, owner_oid);

    let pos = world.objects.get_component::<Position>(&owner_oid).copied()?;
    let servitor_oid = crate::model::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, pos.heading)?;

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
    let servitor_oid = servitor_of(world, owner_oid)?;
    let region = world.objects.get_component::<crate::model::components::RegionCell>(&servitor_oid)?.0;
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
    let Some(client_id) = client_for_player(world, owner_oid) else { return };
    let Some(cs) = world.clients.get(&client_id) else { return };
    let Some(pkt) = build_pet_info(world, owner_oid, servitor_oid, kind) else { return };
    cs.send(pkt);
}

fn build_pet_info(world: &World, owner_oid: i32, servitor_oid: i32, kind: PetInfoKind) -> Option<Vec<u8>> {
    let npc = world.objects.get_component::<crate::model::npc::Npc>(&servitor_oid)?;
    let template = npc.template(world)?;
    let pos = world.objects.get_component::<Position>(&servitor_oid)?;
    let vitals = world.objects.get_component::<Vitals>(&servitor_oid)?;
    let cs = world.objects.get_component::<CombatStats>(&servitor_oid)?;
    let speeds = world.objects.get_component::<Speeds>(&servitor_oid)?;
    let collision = world.objects.get_component::<Collision>(&servitor_oid)?;
    let servitor = world.objects.get_component::<ServitorOf>(&servitor_oid)?;
    let owner_name = world.objects.get_component::<crate::model::Player>(&owner_oid).map(|p| p.name.clone())?;

    // Java divides the wire speeds by the move multiplier (the client
    // multiplies them back) — the same treatment `UserInfo`/`CharInfo` already
    // get on this port.
    let mult = speeds.move_multiplier;
    let run = (speeds.run_spd / mult).round() as i16;
    let walk = (speeds.walk_spd / mult).round() as i16;

    // `getLifeTimeRemaining()` / `getLifeTime()` ride in the fed/max-fed pair
    // for a servitor — this is what draws the summon's remaining-time bar.
    let (cur_fed, max_fed) = if servitor.life_time_secs > 0 {
        let remaining = servitor.expires_at_tick.saturating_sub(world.tick) / TICKS_PER_SECOND;
        (remaining as i32, servitor.life_time_secs)
    } else {
        (0, 0)
    };

    let mut w = PacketWriter::new();
    w.write_u8(server_packets::opcodes::PET_INFO);
    w.write_u8(2); // `getSummonType()` — 2 = servitor (1 is a pet)
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
    w.write_string(if template.server_side_name { &template.name } else { "" });
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
    let Some(link) = world.objects.get_component::<ServitorOf>(&servitor_oid).copied() else { return };
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
        world.objects.get_component::<Position>(&link.owner_object_id).copied(),
        world.objects.get_component::<Position>(&servitor_oid).copied(),
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
    let Some(servitor_oid) = servitor_of(world, owner_oid) else { return false };
    let (Some(owner), Some(target)) = (
        world.objects.get_component::<Position>(&owner_oid).copied(),
        world.objects.get_component::<Position>(&target_oid).copied(),
    ) else {
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
    if let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&servitor_oid) {
        aggro.0.entry(target_oid).or_default().hate = max_hate + 1.0;
    }
    if let Some(ai) = world.objects.get_component_mut::<crate::model::npc::NpcAi>(&servitor_oid) {
        ai.intention = crate::model::npc::NpcIntention::Attack;
        ai.attack_timeout_tick = world.tick + crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
    }
    true
}

/// `ServitorStop` (action 23) — `cancelAction()`: drop the target, stop moving,
/// and go back to trailing the owner.
pub(crate) fn servitor_stop(world: &mut World, owner_oid: i32) -> bool {
    let Some(servitor_oid) = servitor_of(world, owner_oid) else { return false };
    if let Some(aggro) = world.objects.get_component_mut::<crate::model::npc::AggroList>(&servitor_oid) {
        aggro.0.clear();
    }
    world.objects.remove_component::<crate::model::components::Movement>(&servitor_oid);
    if let Some(ai) = world.objects.get_component_mut::<crate::model::npc::NpcAi>(&servitor_oid) {
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
    let l = world.objects.get_component_mut::<ServitorOf>(&servitor_oid)?;
    l.following = !l.following;
    let now = l.following;
    if !now {
        // Holding ground: stop where you are.
        world.objects.remove_component::<crate::model::components::Movement>(&servitor_oid);
    }
    Some(now)
}

/// Java action ids for the servitor commands (`dist/game/data/ActionData.xml`).
pub mod action {
    /// `ServitorHold` — follow me / hold your ground.
    pub const SERVITOR_HOLD: i32 = 21;
    /// `ServitorAttack` — attack my target.
    pub const SERVITOR_ATTACK: i32 = 22;
    /// `ServitorStop` — cancel what you are doing.
    pub const SERVITOR_STOP: i32 = 23;
}

/// `RequestActionUse` — the servitor commands only. Other action ids (sit,
/// socials, the per-summon skill buttons) are not handled here yet.
pub(crate) fn handle_request_action_use(world: &mut World, client_id: u32, body: &[u8]) {
    use crate::network::server_packets::sm_ids;
    let Some(pkt) = crate::network::client_packets::RequestActionUse::read(body) else { return };
    if !matches!(pkt.action_id, action::SERVITOR_HOLD | action::SERVITOR_ATTACK | action::SERVITOR_STOP) {
        return;
    }
    let Some(owner_oid) = (match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }) else {
        return;
    };
    // Java's shared guard: dead or control-blocked players issue no actions.
    if world.objects.get_component::<Vitals>(&owner_oid).is_none_or(|v| v.dead)
        || crate::game_loop::abnormal::is_control_blocked(world, owner_oid)
    {
        return;
    }
    // Every handler opens with the same "do you even have one" check.
    if servitor_of(world, owner_oid).is_none() {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(sm_ids::YOU_DO_NOT_HAVE_A_SERVITOR, &[]));
        }
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
    let Some(link) = world.objects.get_component::<ServitorOf>(&servitor_oid).copied() else { return };
    let Some(region) = world.objects.get_component::<RegionCell>(&servitor_oid).map(|r| r.0) else { return };
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&servitor_oid) else { return };
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
    let pkt =
        server_packets::summon_info(servitor_oid, t, pos, vitals, speeds, combat, &owner_name, 0, summoned);
    for cs in world.clients.values() {
        let crate::session::ClientSession::InGame(session) = cs else { continue };
        let viewer = session.player_object_id();
        if viewer == link.owner_object_id {
            continue; // the owner has the PetInfo view
        }
        let Some(vr) = world.objects.get_component::<RegionCell>(&viewer) else { continue };
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
    use crate::network::server_packets::{sm_ids, SmParam};
    let Some(link) = world.objects.get_component::<ServitorOf>(&servitor_oid).copied() else { return };
    // Dead or already gone → the chain ends (Java cancels the task).
    if world.objects.get_component::<Vitals>(&servitor_oid).is_none_or(|v| v.dead) {
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
                if let Some(cs) = world.clients.get(&cid) {
                    cs.send(crate::network::enter_world::inventory_update_changes(&world.data, &changes));
                }
            }
            notify_owner(world, owner, sm_ids::A_SUMMONED_MONSTER_USES_S1, &[SmParam::ItemName(link.consume_item_id)]);
            if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
                l.next_consume_tick = world.tick + CONSUME_INTERVAL_SECS * TICKS_PER_SECOND;
            }
        } else {
            notify_owner(world, owner, sm_ids::NOT_ENOUGH_ITEMS_TO_MAINTAIN_SERVITOR, &[]);
            unsummon_servitor(world, owner);
            return;
        }
    }

    // 3. The remaining-time bar.
    if link.life_time_secs > 0 {
        let remaining = (link.expires_at_tick.saturating_sub(world.tick) / TICKS_PER_SECOND) as i32;
        if let Some(cid) = client_for_player(world, owner) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(server_packets::set_summon_remain_time(link.life_time_secs, remaining));
            }
        }
    }

    // 4. The leash — "using same task to check if owner is in visible range".
    // A servitor left too far behind is dragged back into follow whatever it
    // was doing, so an ordered attack can't strand it across the map.
    if let (Some(me), Some(o)) = (
        world.objects.get_component::<Position>(&servitor_oid).copied(),
        world.objects.get_component::<Position>(&owner).copied(),
    ) {
        let dx = (me.x - o.x) as f64;
        let dy = (me.y - o.y) as f64;
        let dz = (me.z - o.z) as f64;
        if (dx * dx + dy * dy + dz * dz).sqrt() > LEASH_DISTANCE {
            if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
                l.following = true;
            }
            if let Some(ai) = world.objects.get_component_mut::<crate::model::npc::NpcAi>(&servitor_oid) {
                ai.intention = crate::model::npc::NpcIntention::Active;
            }
        }
    }

    world.scheduler.schedule(
        world.tick + LIFE_TICK_SECS * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::ServitorLifeTick { servitor_oid },
    );
}

fn notify_owner(world: &World, owner_oid: i32, sm: i16, params: &[crate::network::server_packets::SmParam]) {
    let Some(cid) = client_for_player(world, owner_oid) else { return };
    let Some(cs) = world.clients.get(&cid) else { return };
    cs.send(server_packets::system_message_with(sm, params));
}

/// The owner left the world (logout/disconnect) — their servitor goes with
/// them. Java stores it in `CharSummonTable` for `RestoreServitorOnReconnect`;
/// persistence is a later slice, so this just removes it.
pub(crate) fn on_owner_leave_world(world: &mut World, owner_oid: i32) {
    unsummon_servitor(world, owner_oid);
}
