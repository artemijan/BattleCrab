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

mod ai;
mod death;
pub(crate) mod evolve;
mod exp;
mod feeding;
mod lifetime;
pub(crate) mod pet;
mod restore;
mod shots;
mod stats;
pub(crate) mod tamed_beast;

use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::restore_hp_mp;
use crate::game_loop::helpers::send_to_player;
use crate::game_loop::time::TICKS_PER_SECOND;
use crate::model::components::{Collision, CombatStats, Position, ServitorOf, Speeds, Vitals};
use crate::network::server_packets;
use crate::world::World;
pub(crate) use ai::{
    handle_pet_action, handle_servitor_action, servitor_attack, servitor_follow_tick,
};
#[cfg(test)]
pub(crate) use ai::{servitor_stop, servitor_toggle_follow, use_servitor_skill};
use commons::network::PacketWriter;
pub(crate) use death::{pet_decay, pet_do_die, pet_restore_exp};
pub(crate) use exp::{add_pet_exp, split_exp_with_pet, sync_collar_enchant};
#[cfg(test)]
pub(crate) use feeding::apply_feed;
use feeding::npc_template_id;
pub(crate) use feeding::{
    handle_feed_tick, handle_get_item_from_pet, handle_give_item_to_pet, handle_pet_use_item,
    is_hungry, is_uncontrollable, send_pet_item_list, start_feed,
};
use lifetime::notify_owner;
pub(crate) use lifetime::{broadcast_summon_info, handle_life_tick, on_owner_leave_world};
pub(crate) use pet::{
    active_pet_collar, equip_pet_item, handle_request_pet_get_item, pet_of, pet_pickup_think,
    summon_pet, sync_pet_row,
};
pub(crate) use restore::{restore_pet_on_login, restore_servitor_on_login, sync_summon_row};
pub(crate) use shots::{
    recharge_shots, recharge_spiritshots, uncharge_soulshot, uncharge_spiritshot,
};
pub(crate) use stats::recalculate_pet_stats;

/// Java's `Servitor.run()` period — a fixed `usedtime = 5000` ms.
const LIFE_TICK_SECS: u64 = 5;

/// Java's default `consumeItemInterval`:
///
/// ```java
/// final int consumeItemInterval = (_consumeItemInterval > 0 ? _consumeItemInterval
///     : (template.getRace() != Race.SIEGE_WEAPON ? 240 : 60)) * 1000;
/// ```
///
/// **240 s for an ordinary servitor, 60 s for a siege weapon.** The split is
/// not decoration: Summon Siege Golem (13) is learnable and costs 40 C-grade
/// gemstones a go, so running it on the ordinary interval quarters the price of
/// the most expensive summon in the game.
///
/// No skill on this dist declares a `consumeItemInterval` of its own, so the
/// default arm is the whole of it.
const CONSUME_INTERVAL_SECS: u64 = 240;
const SIEGE_WEAPON_CONSUME_INTERVAL_SECS: u64 = 60;

/// The consume period for one servitor — [`CONSUME_INTERVAL_SECS`] unless the
/// template's `<race>` is `SIEGE_WEAPON`.
fn consume_interval_secs(world: &World, npc_id: i32) -> u64 {
    let siege_weapon = world
        .data
        .npc_data
        .get(npc_id)
        .and_then(|t| t.race)
        .is_some_and(|r| r == crate::enums::Race::SiegeWeapon as i32);
    if siege_weapon {
        SIEGE_WEAPON_CONSUME_INTERVAL_SECS
    } else {
        CONSUME_INTERVAL_SECS
    }
}

/// Java's leash: further than this from its owner and the servitor is forced
/// back into follow, regardless of what it was doing.
const LEASH_DISTANCE: f64 = 2000.0;

/// The Sin Eater's display id — the one species Java summons at its *owner's*
/// level rather than its template level (`Pet`'s three-arg constructor).
const SIN_EATER_DISPLAY_ID: i32 = 12564;

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

    let pos = maybe_position(world, owner_oid)?;
    let servitor_oid =
        crate::game_loop::npc::spawn_npc_at(world, npc_id, pos.x, pos.y, pos.z, pos.heading)?;

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
            defending: false,
            consume_item_id,
            consume_item_count,
            next_consume_tick: if consume_item_id > 0 {
                world.tick + consume_interval_secs(world, npc_id) * TICKS_PER_SECOND
            } else {
                u64::MAX
            },
        },
    );
    // `summon.setCurrentHp(getMaxHp()); setCurrentMp(getMaxMp())`.
    restore_hp_mp(world, servitor_oid);
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
    crate::game_loop::npc::despawn_npc(world, servitor_oid, region);
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
    let Some(pkt) = build_pet_info(world, owner_oid, servitor_oid, kind) else {
        return;
    };
    send_to_player(world, owner_oid, pkt);
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
