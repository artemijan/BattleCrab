//! The Beast Farm tamed beast (Java `model/actor/instance/TamedBeast`) —
//! the reward at the top of the feeding chain: a beast that follows its
//! tamer and stays only as long as the spice keeps coming.
//!
//! Java's clock: spawn with 20 minutes; every 60 s the check subtracts a
//! minute and, if the owner carries the right spice, auto-feeds one (which
//! routes through the feed path and gives 20 s back — net -40 s/min); with
//! no spice the beast leaves at once unless it is inside its first five
//! minutes. A 5 s `CheckOwnerBuffs` beat keeps the tamer buffed from the
//! beast's `<skillList>` (see [`handle_buff_check`]).

use crate::game_loop::death::despawn_npc_by_oid;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::skill_by_id;
use crate::model::components::{Position, TamedBeastOf, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `MAX_DURATION` (20 min) in ticks.
const MAX_DURATION_TICKS: i32 = 12_000;
/// `DURATION_CHECK_INTERVAL` (60 s).
const DURATION_CHECK_TICKS: u64 = 600;
/// `DURATION_INCREASE_INTERVAL` (20 s) — gained per feeding.
const FEED_GAIN_TICKS: i32 = 200;
/// The no-food grace: a beast younger than 5 minutes survives an empty
/// pouch (`MAX_DURATION - 300000`).
const NO_FOOD_GRACE_TICKS: i32 = 3_000;
/// The follow beat and its trigger distance (Java `startFollow(owner, 100)`).
const FOLLOW_TICKS: u64 = 10;
const FOLLOW_DISTANCE: f64 = 100.0;
/// `BUFF_INTERVAL` (5 s) and `MAX_DISTANCE_FROM_OWNER` — the owner-buff beat.
const BUFF_CHECK_TICKS: u64 = 50;
const MAX_DISTANCE_FROM_OWNER: f64 = 2000.0;

/// Spice skill → spice item (2188→6643 golden, 2189→6644 crystal).
pub(crate) fn spice_item_for_skill(skill_id: i32) -> Option<i32> {
    match skill_id {
        2188 => Some(6643),
        2189 => Some(6644),
        _ => None,
    }
}

/// Spawn a freshly tamed beast for `owner` and arm its clocks. Any other
/// beast the player has trained first is dismissed (Java clears
/// `getTrainedBeasts` before the new one spawns).
pub(crate) fn spawn_tamed_beast(
    world: &mut World,
    npc_id: i32,
    owner: i32,
    food_skill: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Option<i32> {
    for old in beasts_of(world, owner) {
        despawn_npc_by_oid(world, old);
    }
    let oid = crate::game_loop::npc::spawn_npc_at(world, npc_id, x, y, z, 0)?;
    world.objects.add_components(
        &oid,
        TamedBeastOf {
            owner,
            food_skill,
            remaining_ticks: MAX_DURATION_TICKS,
        },
    );
    world.scheduler.schedule(
        world.tick + DURATION_CHECK_TICKS,
        ScheduledTask::TamedBeastDuration { beast_oid: oid },
    );
    world.scheduler.schedule(
        world.tick + FOLLOW_TICKS,
        ScheduledTask::TamedBeastFollow { beast_oid: oid },
    );
    world.scheduler.schedule(
        world.tick + BUFF_CHECK_TICKS,
        ScheduledTask::TamedBeastBuffCheck { beast_oid: oid },
    );
    Some(oid)
}

/// All living tamed beasts trained by `owner`.
pub(crate) fn beasts_of(world: &mut World, owner: i32) -> Vec<i32> {
    let mut out = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &TamedBeastOf)>(|(n, t)| {
            if t.owner == owner {
                out.push(n.object_id);
            }
        });
    out
}

/// `onReceiveFood`: +20 s, capped at 20 minutes.
pub(crate) fn on_receive_food(world: &mut World, beast_oid: i32) {
    if let Some(t) = world.objects.get_component_mut::<TamedBeastOf>(&beast_oid) {
        t.remaining_ticks = (t.remaining_ticks + FEED_GAIN_TICKS).min(MAX_DURATION_TICKS);
    }
}

/// The 60 s `CheckDuration` beat.
pub(crate) fn handle_duration(world: &mut World, beast_oid: i32) {
    let Some(t) = world.objects.get_component_mut::<TamedBeastOf>(&beast_oid) else {
        return;
    };
    t.remaining_ticks -= DURATION_CHECK_TICKS as i32;
    let (owner, food_skill, remaining) = (t.owner, t.food_skill, t.remaining_ticks);

    let spice = spice_item_for_skill(food_skill);
    let has_spice = spice.is_some_and(|item| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&owner)
            .is_some_and(|inv| inv.count_of(item) >= 1)
    });
    if has_spice {
        // Java triggers a cast of the feed skill (whose item consume takes
        // the spice) at the beast; the port consumes directly and feeds.
        if let (Some(item), Some(client_id)) = (
            spice,
            crate::game_loop::helpers::client_for_player(world, owner),
        ) {
            crate::game_loop::quests::take_items(world, client_id, owner, item, 1);
        }
        on_receive_food(world, beast_oid);
    } else if remaining < MAX_DURATION_TICKS - NO_FOOD_GRACE_TICKS {
        // Out of spice past the newcomer grace: leaves at once.
        despawn_npc_by_oid(world, beast_oid);
        return;
    }
    if world
        .objects
        .get_component::<TamedBeastOf>(&beast_oid)
        .is_some_and(|t| t.remaining_ticks <= 0)
    {
        despawn_npc_by_oid(world, beast_oid);
        return;
    }
    world.scheduler.schedule(
        world.tick + DURATION_CHECK_TICKS,
        ScheduledTask::TamedBeastDuration { beast_oid },
    );
}

/// The 1 s follow beat: trot after the tamer; if they are gone (logged out
/// or dead-and-gone), the beast leaves.
pub(crate) fn handle_follow(world: &mut World, beast_oid: i32) {
    let Some(owner) = world
        .objects
        .get_component::<TamedBeastOf>(&beast_oid)
        .map(|t| t.owner)
    else {
        return;
    };
    let owner_pos = world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .and_then(|_| maybe_position(world, owner));
    let Some(owner_pos) = owner_pos else {
        // Owner no longer in the world.
        despawn_npc_by_oid(world, beast_oid);
        return;
    };
    let near = world
        .objects
        .get_component::<Position>(&beast_oid)
        .is_some_and(|p| p.distance_2d(&owner_pos) <= FOLLOW_DISTANCE);
    let alive = world
        .objects
        .get_component::<Vitals>(&beast_oid)
        .is_some_and(|v| !v.dead);
    if alive && !near {
        crate::game_loop::ai::move_npc_to(world, beast_oid, owner_pos.x, owner_pos.y, owner_pos.z);
    }
    world.scheduler.schedule(
        world.tick + FOLLOW_TICKS,
        ScheduledTask::TamedBeastFollow { beast_oid },
    );
}

/// The 5 s `CheckOwnerBuffs` beat: gather the continuous non-debuff skills
/// from the beast's `<skillList>`; when the tamer carries fewer than two
/// thirds of them, cast one picked at random (Java rolls the index before
/// counting, so the pick is independent of what's missing). Skipped while
/// the owner is dead or out of `MAX_DISTANCE_FROM_OWNER`, or while the
/// beast is mid-cast; the task dies with the beast.
pub(crate) fn handle_buff_check(world: &mut World, beast_oid: i32) {
    let Some(owner) = world
        .objects
        .get_component::<TamedBeastOf>(&beast_oid)
        .map(|t| t.owner)
    else {
        return; // despawned — the fixed-rate task ends here
    };
    // `if ((owner == null) || !owner.isOnline()) deleteMe()`.
    if world
        .objects
        .get_component::<crate::model::Player>(&owner)
        .is_none()
    {
        despawn_npc_by_oid(world, beast_oid);
        return;
    }
    world.scheduler.schedule(
        world.tick + BUFF_CHECK_TICKS,
        ScheduledTask::TamedBeastBuffCheck { beast_oid },
    );
    // Too far: Java startFollows and returns — the follow beat here already
    // chases, so just skip the buffing.
    let too_far = match (
        world.objects.get_component::<Position>(&beast_oid),
        world.objects.get_component::<Position>(&owner),
    ) {
        (Some(b), Some(o)) => b.distance_2d(o) > MAX_DISTANCE_FROM_OWNER,
        _ => true,
    };
    let owner_dead = is_dead(world, owner);
    let beast_casting = world
        .objects
        .has_component::<crate::model::components::Casting>(&beast_oid);
    if too_far || owner_dead || beast_casting {
        return;
    }

    // The template's buffs: `<skillList>` entries that parse to a continuous
    // non-debuff skill.
    let buffs: Vec<(i32, i32)> = npc_template(world, beast_oid)
        .map(|t| {
            t.skill_list
                .iter()
                .copied()
                .filter(|&(id, lvl)| {
                    world
                        .data
                        .skill_data
                        .get(id, lvl)
                        .is_some_and(|s| s.is_continuous && !s.is_debuff)
                })
                .collect()
        })
        .unwrap_or_default();
    if buffs.is_empty() {
        return;
    }
    let pick = buffs[world.roll(buffs.len() as i32) as usize];
    let on_owner = world
        .objects
        .get_component::<crate::model::components::Buffs>(&owner)
        .map_or(0, |b| {
            buffs
                .iter()
                .filter(|(id, _)| b.0.iter().any(|a| a.skill_id == *id))
                .count()
        });
    // `if (((numBuffs * 2) / 3) > totalBuffsOnOwner) sitCastAndFollow(...)`.
    if (buffs.len() * 2) / 3 > on_owner
        && let Some(skill) = skill_by_id(world, pick.0, pick.1)
    {
        crate::game_loop::npc::cast::start_cast(world, beast_oid, owner, &skill);
    }
}

/// The mad-cow reversion: 10 s after a mad cow emerges it becomes the plain
/// top-stage animal — still furious at the feeder (Java's
/// `"polymorph Mad Cow"` timer in `FeedableBeasts`).
pub(crate) fn handle_mad_cow_polymorph(world: &mut World, cow_oid: i32, feeder_oid: i32) {
    let Some(npc_id) = npc_id_of(world, cow_oid) else {
        return;
    };
    let Some(next_id) = crate::scripts::feedable_beasts::mad_cow_reverts_to(npc_id) else {
        return;
    };
    let Some(pos) = maybe_position(world, cow_oid) else {
        return;
    };
    despawn_npc_by_oid(world, cow_oid);
    if let Some(next) = crate::game_loop::npc::spawn_npc_at(world, next_id, pos.x, pos.y, pos.z, 0)
    {
        crate::game_loop::death::introduce_npc(world, next);
        if let Some(n) = world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&next)
        {
            n.vars.insert("feeder".into(), feeder_oid);
        }
        crate::game_loop::ai::seed_attack(world, next, feeder_oid);
    }
}
