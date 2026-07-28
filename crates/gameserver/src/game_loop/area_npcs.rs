//! Global, player-less lifecycles from the `ai/areas` scripts — beats that
//! cannot ride the quest-timer machinery because that is anchored to a
//! player (`QuestTimerSeqs`), while these run from boot with nobody online.
//!
//! First resident: **Toma** (`ai/areas/DwarvenVillage/Toma`). Java spawns him
//! at one of three haunts and relocates him every 30 minutes
//! (`RESPAWN_TOMA`); his chat window is `scripts::toma`.

use crate::scheduler::ScheduledTask;
use crate::world::World;

pub(crate) const TOMA: i32 = 30556;

/// Java's `LOCATIONS` — the three spots Toma wanders between.
const TOMA_LOCS: [(i32, i32, i32, i32); 3] = [
    (151680, -174891, -1782, 0),
    (154153, -220105, -3402, 0),
    (178834, -184336, -355, 41400),
];

/// `TELEPORT_DELAY` (30 min) in ticks.
const TOMA_RELOCATE_TICKS: u64 = 30 * 60 * 10;

/// Boot: Toma is not in the spawn data — the script owns him entirely (Java
/// ctor fires `RESPAWN_TOMA` immediately, then every 30 minutes).
pub(crate) fn spawn_at_boot(world: &mut World) {
    relocate_toma(world);
}

/// The `RESPAWN_TOMA` beat: despawn the old Toma, spawn him at a random
/// haunt, re-arm.
pub(crate) fn relocate_toma(world: &mut World) {
    let old = find_toma(world);
    if let Some(oid) = old {
        let region = world
            .objects
            .get_component::<crate::model::components::RegionCell>(&oid)
            .map(|r| r.0);
        if let Some(region) = region {
            crate::game_loop::death::despawn_npc(world, oid, region);
        }
    }
    let (x, y, z, heading) = TOMA_LOCS[world.roll(3) as usize];
    crate::model::npc::spawn_npc_at(world, TOMA, x, y, z, heading);
    world.scheduler.schedule(
        world.tick + TOMA_RELOCATE_TICKS,
        ScheduledTask::TomaRelocate,
    );
}

pub(crate) fn find_toma(world: &mut World) -> Option<i32> {
    let mut found = None;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &crate::model::components::Position)>(
            |(n, _)| {
                if n.npc_id == TOMA {
                    found = Some(n.object_id);
                }
            },
        );
    found
}
