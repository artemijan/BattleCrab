//! `EffectPoint` totems (G19, PLAN_G19_SYMBOLS.md) — the seals the `SummonNpc`
//! effect drops at a ground point (Symbol of Noise, Day of Doom,
//! Anti-summoning Field). Java splits this between
//! `effecthandlers/SummonNpc.java` (the spawn) and
//! `model/actor/instance/EffectPoint.java` (the fixed-rate `union_skill` cast
//! task + the despawn schedule); both halves live here.

use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::skill_by_id;
use crate::model::components::{SummonerRef, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::helpers::ms_to_ticks;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::npc::cast;

/// The `SummonNpc` effect's `EffectPoint` branch: spawn the totem at the
/// point, link it to its owner, title it with the owner's name, and arm the
/// cast + despawn schedules from the template's `<parameters>`.
///
/// Java also `setInvul(true)`; the port's totems are unattackable already
/// (`EffectPoint` is not a monster type), so no invul flag is modeled.
pub(crate) fn spawn_effect_point(
    world: &mut World,
    owner_oid: i32,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    effect_despawn_ms: i32,
) {
    let Some(npc_oid) = crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, 0) else {
        return;
    };
    world
        .objects
        .add_components(&npc_oid, SummonerRef(owner_oid));
    // `effectPoint.setTitle(player.getName())` — the seal shows whose it is,
    // which is the only way a bystander can tell one totem from another.
    let owner_name = world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)
        .map(|p| p.name.clone());
    if let Some(name) = owner_name
        && let Some(npc) = world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&npc_oid)
    {
        npc.title_override = Some(name);
    }

    let Some(template) = world.data.npc_data.get(npc_id) else {
        return;
    };
    // `EffectPoint`'s ctor: first fire after `cast_time` (default **0.1 s**),
    // then every `skill_delay` (default 2 s) — armed only when the template
    // declares a `union_skill`.
    if template.ai_skill_params.contains_key("union_skill") {
        let cast_time_ms = (template.ai_param_f64("cast_time", 0.1) * 1000.0) as i32;
        world.scheduler.schedule(
            world.tick + ms_to_ticks(cast_time_ms),
            ScheduledTask::EffectPointCast { npc_oid },
        );
    }
    // `SummonNpc.instant`: the template's `despawn_time` wins; the effect's
    // `despawnDelay` is the fallback. (All three symbol totems declare 15 s.)
    // A totem with neither lives until something else removes it, like Java.
    let template_ms = (template.ai_param_f64("despawn_time", 0.0) * 1000.0) as i32;
    let despawn_ms = if template_ms > 0 {
        template_ms
    } else {
        effect_despawn_ms
    };
    if despawn_ms > 0 {
        world.scheduler.schedule(
            world.tick + ms_to_ticks(despawn_ms),
            ScheduledTask::EffectPointDespawn { npc_oid },
        );
    }
}

/// `SummonNpc.instant`'s **default** branch — the plain world spawn. The
/// reachable carriers on this dist are all item-cast: the Holiday Trees
/// 5560/5561 (skills 2137/2138 → Folk 13006/13007) and the Squash/Watermelon
/// seeds (skills 2003/2004/2508/2509/9029-9032 → the squash Monsters). Java:
/// `new Spawn(template)` at the point with the summoner's heading,
/// `stopRespawn()`, `doSpawn()`, `player.addSummonedNpc(npc)`, name *and*
/// title set to the template's name, then `scheduleDespawn(despawnDelay)`.
/// (None of the carriers set `randomOffset`/`singleInstance`/`isSummonSpawn`,
/// so those Java params have nothing to model here.)
pub(crate) fn spawn_plain_summon(
    world: &mut World,
    owner_oid: i32,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    despawn_ms: i32,
) {
    let heading = world
        .objects
        .get_component::<crate::model::components::Position>(&owner_oid)
        .map_or(0, |p| p.heading);
    let Some(npc_oid) = crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, heading) else {
        return;
    };
    world
        .objects
        .add_components(&npc_oid, SummonerRef(owner_oid));
    // `player.addSummonedNpc(npc)` — the same registry the quest engine's
    // NPC-side summons use; a corpse still counts until decay, as in Java.
    {
        use crate::model::components::SummonedNpcs;
        match world.objects.get_component_mut::<SummonedNpcs>(&owner_oid) {
            Some(list) => list.0.push(npc_oid),
            None => world
                .objects
                .add_components(&owner_oid, SummonedNpcs(vec![npc_oid])),
        }
    }
    // `npc.setTitle(npcTemplate.getName())` — a plain summon wears its own
    // name as its title (the name itself already defaults to the template's).
    let template_name = world.data.npc_data.get(npc_id).map(|t| t.name.clone());
    if let Some(name) = template_name
        && let Some(npc) = world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&npc_oid)
    {
        npc.title_override = Some(name);
    }
    if despawn_ms > 0 {
        world.scheduler.schedule(
            world.tick + ms_to_ticks(despawn_ms),
            ScheduledTask::EffectPointDespawn { npc_oid },
        );
    }
}

/// One `EffectPoint` skill-task fire: `doCast(union_skill)` at itself (the
/// auras are `SELF` + `POINT_BLANK`), then re-arm at `skill_delay` — Java's
/// `scheduleAtFixedRate`, self-cancelling once the totem is dead or gone.
pub(crate) fn handle_effect_point_cast(world: &mut World, npc_oid: i32) {
    // `if (isDead() || !isSpawned()) { cancel; }`
    let alive = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_some_and(|v| !v.dead);
    if !alive {
        return;
    }
    let Some((skill_id, skill_level, delay_ms)) = npc_template(world, npc_oid).and_then(|t| {
        t.ai_skill_params.get("union_skill").map(|&(id, lvl)| {
            (
                id,
                lvl,
                (t.ai_param_f64("skill_delay", 2.0) * 1000.0) as i32,
            )
        })
    }) else {
        return;
    };
    if let Some(skill) = skill_by_id(world, skill_id, skill_level) {
        cast::start_cast(world, npc_oid, npc_oid, &skill);
    }
    world.scheduler.schedule(
        world.tick + ms_to_ticks(delay_ms.max(100)),
        ScheduledTask::EffectPointCast { npc_oid },
    );
}

/// `Npc.scheduleDespawn` firing: remove the totem from the world. The cast
/// task dies with it (its next fire finds no living NPC).
pub(crate) fn handle_effect_point_despawn(world: &mut World, npc_oid: i32) {
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    super::death::despawn_npc(world, npc_oid, region);
}
