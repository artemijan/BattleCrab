//! NPC walking routes — port of `WalkingManager` + `WalkInfo.calculateNextNode`.
//!
//! 13 routes on this dist drive 13 town NPCs: Giran's porters and scribes on
//! their circuits, the running boy, and Gordon on a 67-node patrol. They spawn
//! from `TownNpcWalkers.xml` and, until now, stood still.
//!
//! **Shape difference from Java.** Java hangs a `ScheduledFuture` off each
//! arrival and keeps a per-NPC `WalkInfo` in a manager map. The port keeps the
//! state on the NPC as a component and drives it from a one-second sweep, which
//! is the same cadence the rest of the NPC AI already runs at. The state
//! machine is the two phases Java's arrive-task implies:
//!
//! - **Travelling** — a `Movement` is in flight. When it disappears the NPC has
//!   arrived, so bank the node's `delay` and switch to waiting.
//! - **Waiting** — once the delay elapses, advance to the next node and walk.

use crate::data::route_data::RepeatStyle;
use crate::model::components::{Movement, Vitals};
use crate::world::World;
use commons::util::rnd;

/// The sweep runs once a second, like the AI think.
pub(crate) const WALKER_PERIOD: u64 = 10;

/// Where an NPC is along its route (Java `WalkInfo`).
#[derive(Debug, Clone, Copy, bevy_ecs::component::Component)]
pub struct WalkState {
    pub route_idx: usize,
    /// Index of the node the NPC is at (waiting) or heading to (travelling).
    pub node: usize,
    /// `WalkInfo._forward` — only meaningful for `GoBack` routes.
    pub forward: bool,
    /// True while a `Movement` should be in flight.
    pub travelling: bool,
    /// Absolute tick the node's `delay` expires at.
    pub resume_at: u64,
}

/// `WalkingManager.onSpawn`: attach a route if this NPC id has one. Called
/// from the spawn path, next to the minion hook.
pub(crate) fn on_npc_spawn(world: &mut World, npc_oid: i32, npc_id: i32) {
    let Some((route_idx, route)) = world.data.routes.route_for_npc(npc_id) else {
        return;
    };
    if route.nodes.is_empty() {
        return;
    }
    world.objects.add_components(
        &npc_oid,
        WalkState {
            route_idx,
            node: 0,
            forward: true,
            travelling: false,
            resume_at: 0,
        },
    );
}

/// One sweep over every NPC on a route.
pub(crate) fn walker_tick(world: &mut World) {
    let mut walkers: Vec<(i32, WalkState)> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &WalkState)>(|(npc, w)| {
            walkers.push((npc.object_id, *w));
        });

    for (oid, mut state) in walkers {
        // `WalkingManager.onDeath` cancels the route permanently.
        if world
            .objects
            .get_component::<Vitals>(&oid)
            .is_none_or(|v| v.dead)
        {
            world.objects.remove_component::<WalkState>(&oid);
            continue;
        }
        // Still walking this leg.
        if world.objects.has_component::<Movement>(&oid) {
            continue;
        }

        // Borrowed per use rather than cloned — Gordon's 67-node route was
        // being copied wholesale every second, mid-delay ticks included.
        if world.data.routes.get(state.route_idx).is_none() {
            world.objects.remove_component::<WalkState>(&oid);
            continue;
        }

        if state.travelling {
            // The `Movement` vanished, so the NPC reached `node`: bank its
            // delay, then wait (Java schedules `ArrivedTask` with the node's
            // `delay` and blocks the walk check meanwhile).
            let delay_ticks = world
                .data
                .routes
                .get(state.route_idx)
                .and_then(|r| r.nodes.get(state.node))
                .map_or(0, |n| n.delay.max(0) as u64 * 10);
            state.travelling = false;
            state.resume_at = world.tick + delay_ticks;
            world.objects.add_components(&oid, state);
            continue;
        }

        if world.tick < state.resume_at {
            continue;
        }

        // Delay served — pick the next node.
        let next = world
            .data
            .routes
            .get(state.route_idx)
            .and_then(|route| advance(route, state.node, &mut state.forward));
        let Some(next) = next else {
            // A non-repeating route ends at its last node.
            world.objects.remove_component::<WalkState>(&oid);
            continue;
        };
        state.node = next;
        state.travelling = true;
        world.objects.add_components(&oid, state);

        let dest = world
            .data
            .routes
            .get(state.route_idx)
            .and_then(|r| r.nodes.get(next))
            .map(|n| (n.x, n.y, n.z));
        if let Some((x, y, z)) = dest {
            super::npc_ai::move_npc_to(world, oid, x, y, z);
        }
    }
}

/// `WalkInfo.calculateNextNode`. Returns the next node index, or `None` when a
/// non-repeating route has run out.
///
/// The `GoBack` arithmetic is Java's and looks odd on purpose: on overrunning
/// the last node it steps back **two** (`_currentNode -= 2`), because the
/// index was already incremented past the end — landing on the second-to-last
/// node, which is the first step of the return leg.
fn advance(
    route: &crate::data::route_data::WalkRoute,
    current: usize,
    forward: &mut bool,
) -> Option<usize> {
    let count = route.nodes.len();
    if count == 0 {
        return None;
    }

    if route.repeat_style == RepeatStyle::Random {
        if count == 1 {
            return Some(0);
        }
        let mut next = current;
        while next == current {
            next = rnd::get(count as i32) as usize;
        }
        return Some(next);
    }

    let mut node = current as i64 + if *forward { 1 } else { -1 };

    if node == count as i64 {
        // Ran past the last node.
        if !route.repeat {
            return None;
        }
        match route.repeat_style {
            RepeatStyle::GoBack => {
                *forward = false;
                node -= 2;
            }
            // `conveyor` teleports rather than walks back; without NPC
            // teleport plumbing it behaves as `cycle` here.
            // `TeleportFirst` would teleport rather than walk back to node 0.
            // Not a deferral: no route in `dist/game/data` declares that repeat
            // style, so the branch has no way to be reached.
            RepeatStyle::GoFirst | RepeatStyle::TeleportFirst => node = 0,
            _ => return None,
        }
    } else if node < 0 {
        // Retraced past the first node — turn around again.
        node = 1;
        *forward = true;
    }

    let node = node.clamp(0, count as i64 - 1) as usize;
    Some(node)
}
