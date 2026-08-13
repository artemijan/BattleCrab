//! Hate-list management: stop, calm-down, the periodic hate check and
//! `onEvtForgetObject`.

use super::*;

/// Stop a mob dead (remove its move, broadcast `StopMove`) — the NPC half of
/// `AbstractAI.clientStopMoving(null)`.
pub(crate) fn stop_npc(world: &mut World, npc_oid: i32) {
    if !world.objects.has_component::<Movement>(&npc_oid) {
        return;
    }
    world.objects.remove_component::<Movement>(&npc_oid);
    if let (Some(pos), Some(region)) = (
        maybe_position(world, npc_oid),
        region_cell_of(world, npc_oid),
    ) {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading),
        );
    }
}

/// `AttackableAI.setGlobalAggro(-25)`: the longer calm window Java drops a mob
/// into when it stops hating *everyone* — roughly 25 think seconds during which
/// [`think_active`] neither seeds hate from its aggro scan nor acts on hate it
/// is handed. Distinct from the −10 a fresh [`NpcAi`] carries out of spawn.
const CALM_GLOBAL_AGGRO: i32 = -25;

/// The tail Java runs when a mob stops hating everyone — `setGlobalAggro(-25)`
/// + `clearAggroList()` + `setWalking()` + `setIntention(ACTIVE)`. It appears
/// in `Attackable.setTarget(null)` (`Attackable.java` 1861-1881), which is what
/// [`on_forget_object`] ports, and in the three `Attackable.reduceHate`
/// branches (873-919) — which nothing on this chronicle reaches, so that caller
/// is left unwired on purpose: `AddHate` double-negates its way into *raising*
/// hate (see the note in `skills::effects`) and `TransferHate` (skill 489,
/// Shift Target) is off-chronicle here.
///
/// Without this a mob whose last hated player vanished re-seeds hate from the
/// very next scan tick and re-aggros instantly; Java stands it down for ~25 s.
pub(crate) fn go_calm(world: &mut World, npc_oid: i32) {
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.global_aggro = CALM_GLOBAL_AGGRO;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.clear();
    }
    // `setWalking()` + `setIntention(AI_INTENTION_ACTIVE)`.
    set_active(world, npc_oid);
}

/// `AggroInfo.checkHate`, run across the aggro list before every most-hated
/// pick (Java runs it per-entry inside `Attackable.getMostHated`): hate
/// silently zeroes for an attacker who is dead, despawned, or no longer
/// inside the NPC's 3×3 surrounding regions. The entry survives — only its
/// weight drops — and this is what actually makes a mob forget a target that
/// left the neighbourhood; without it a hated player stays "most hated"
/// forever and the mob chases across the world.
pub(super) fn check_hate(world: &mut World, npc_oid: i32) {
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    let Some(aggro) = world.objects.get_component::<AggroList>(&npc_oid) else {
        return;
    };
    let hated: Vec<i32> = aggro
        .0
        .iter()
        .filter(|(_, info)| info.hate > 0.0)
        .map(|(&id, _)| id)
        .collect();
    let mut expired: Vec<i32> = Vec::new();
    for id in hated {
        let alive_nearby = world
            .objects
            .get_component::<Vitals>(&id)
            .is_some_and(|v| !v.dead)
            && world
                .objects
                .get_component::<RegionCell>(&id)
                .is_some_and(|r| regions_adjacent(region, r.0));
        if !alive_nearby {
            expired.push(id);
        }
    }
    if expired.is_empty() {
        return;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        for id in expired {
            if let Some(info) = aggro.0.get_mut(&id) {
                info.hate = 0.0;
            }
        }
    }
}

/// Java `CreatureAI.onEvtForgetObject` narrowed to an `Attackable` whose
/// *current target* is the departing object, i.e. `Attackable.setTarget(null)`
/// (`Attackable.java` 1861-1881): drop the target's aggro entry outright, and
/// if that emptied the list send the mob into the −25 calm window.
///
/// Fired from the visibility layer — the object leaving the NPC's 3×3 block or
/// the world — because that is where Java raises it (`World.switchRegion` /
/// `removeVisibleObject`), and doing it there rather than lazily at think time
/// matters twice over: it is an *edge*, so an object that was never nearby (a
/// script seeding a grudge across the map) can't trigger it; and it still fires
/// when the departure leaves the mob's region with no players in it, which
/// stops the AI thinking at all.
///
/// NPCs here hold no `TargetRef` — the aggro list *is* the target, and Java
/// only ever assigns one in `thinkAttack` from `getMostHated` — so the
/// most-hated stands in for `getTarget()`.
pub(crate) fn on_forget_object(world: &mut World, npc_oid: i32, object_id: i32) {
    let Some(aggro) = world.objects.get_component::<AggroList>(&npc_oid) else {
        return;
    };
    if aggro.most_hated() != Some(object_id) {
        return;
    }
    let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) else {
        return;
    };
    // `if (target != null) _aggroList.remove(target);`
    aggro.0.remove(&object_id);
    // `if (_aggroList.isEmpty())` — literally empty, as in Java: a zeroed entry
    // left behind for some other attacker keeps the mob out of the calm window.
    if aggro.0.is_empty() {
        go_calm(world, npc_oid);
    }
}
