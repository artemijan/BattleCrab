//! `CreatureFollowTaskManager`'s normal follow for an NPC — what a script's
//! `npc.getAI().startFollow(player)` starts. Every second the NPC walks back
//! to within a fresh `Rnd.get(50, 100)` of its target, and gives up (IDLE)
//! once the target is gone or more than 3000 away.
//!
//! `AbstractAI.setIntention` stops the follow for any intention other than
//! FOLLOW or ATTACK; the NPC intention setters call [`stop_follow`].

use super::movement::chase;
use crate::model::components::space::{NpcFollow, Position};
use crate::model::npc::NpcAi;
use crate::scheduler::ScheduledTask;
use crate::world::World;
use commons::util::rnd;

/// `CreatureFollowNormalTask`'s period.
const FOLLOW_PERIOD_TICKS: u64 = 10;

/// `follow`'s "if the target is too far (maybe also teleported)" bound.
const FOLLOW_ABANDON_DISTANCE: f64 = 3000.0;

/// `AbstractAI.startFollow(target)`: replace any follow, set the AI target,
/// and run the first pass at once (`addNormalFollow` calls `follow` itself).
pub(crate) fn start_follow(world: &mut World, npc_oid: i32, target_oid: i32) {
    stop_follow(world, npc_oid);
    let seq = world.next_request_seq();
    world.objects.add_components(
        &npc_oid,
        NpcFollow {
            target_object_id: target_oid,
            seq,
        },
    );
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.target = target_oid;
    }
    follow(world, npc_oid);
    world.scheduler.schedule(
        world.tick + FOLLOW_PERIOD_TICKS,
        ScheduledTask::NpcFollow { npc_oid, seq },
    );
}

/// `AbstractAI.stopFollow()`.
pub(crate) fn stop_follow(world: &mut World, npc_oid: i32) {
    world.objects.remove_component::<NpcFollow>(&npc_oid);
}

/// `ScheduledTask::NpcFollow`: one pass, then the next one a second later.
pub(crate) fn handle_npc_follow(world: &mut World, npc_oid: i32, seq: u64) {
    let live = world
        .objects
        .get_component::<NpcFollow>(&npc_oid)
        .is_some_and(|f| f.seq == seq);
    if !live {
        return;
    }
    follow(world, npc_oid);
    if world
        .objects
        .get_component::<NpcFollow>(&npc_oid)
        .is_some_and(|f| f.seq == seq)
    {
        world.scheduler.schedule(
            world.tick + FOLLOW_PERIOD_TICKS,
            ScheduledTask::NpcFollow { npc_oid, seq },
        );
    }
}

/// `CreatureFollowTaskManager.follow(creature, -1)`. The port has no IDLE
/// intention; ACTIVE is the nearest, and Java's IDLE is left by the next
/// attack the same way.
fn follow(world: &mut World, npc_oid: i32) {
    let Some(target_oid) = world
        .objects
        .get_component::<NpcFollow>(&npc_oid)
        .map(|f| f.target_object_id)
    else {
        return;
    };
    let (Some(me), Some(target)) = (
        world.objects.get_component::<Position>(&npc_oid).copied(),
        world
            .objects
            .get_component::<Position>(&target_oid)
            .copied(),
    ) else {
        give_up(world, npc_oid);
        return;
    };
    let range = rnd::get_range(50, 100);
    let (dx, dy, dz) = (
        f64::from(target.x - me.x),
        f64::from(target.y - me.y),
        f64::from(target.z - me.z),
    );
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    if distance <= f64::from(range) {
        return;
    }
    if distance > FOLLOW_ABANDON_DISTANCE {
        give_up(world, npc_oid);
        return;
    }
    chase(world, npc_oid, target_oid, f64::from(range));
}

/// `ai.setIntention(IDLE)`, which also stops the follow.
fn give_up(world: &mut World, npc_oid: i32) {
    stop_follow(world, npc_oid);
    super::set_active_intention(world, npc_oid);
}
