//! `ai/areas/PrimevalIsle` — the island's signature behaviors that are live
//! on this dist: Ancient Eggs that raise the jungle when struck, Sprigant
//! poison traps, and the Tyrannosaurus's curiosity pause + berserk ladder.
//!
//! Deferred with the creature-see gap (TODO(G22)): Java's `onCreatureSee`
//! drives the Trex hunting herbivores (`CREW_SKILL` presentation 6172), the
//! Deino/Ornit herd flee, and the `ag_type`-gated on-sight specials — the
//! port has no creature-enters-vision hook (only the aggro scan), and
//! approximating it there would fire on a different population. The skill
//! params themselves are already parsed (`ai_skill_params`); the hook is the
//! whole remainder. Skipped as off-chronicle: the Deinonychus Mesozoic Stone
//! taming (Gracia; its tamed dinos 22742/22743 never spawn here).

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::components::{Position, Vitals};
use crate::model::npc::AggroList;

const EGG: i32 = 18344;
pub(crate) const SPRIGANT_ANESTHESIA: i32 = 18345;
pub(crate) const SPRIGANT_POISON: i32 = 18346;
const TREX: [i32; 3] = [22215, 22216, 22217];

/// Sprigant trap skills.
const ANESTHESIA: i32 = 5085;
const DEADLY_POISON: i32 = 5086;
/// Berserk (level 1 and 2 share the id — Java's state machine keys off it).
const BERSERK: i32 = 5087;
/// The Trex arsenal: ranged stun, melee stun/silence/spin.
const LONG_RANGE_STUN: i32 = 5120;
const SPECIAL_STUN: i32 = 5083;
const SPECIAL_SILENCE: i32 = 5081;
const SPECIAL_SPIN: i32 = 5082;

pub struct PrimevalIsle;

impl QuestScript for PrimevalIsle {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "PrimevalIsle"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/PrimevalIsle"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn spawn_npcs(&self) -> &[i32] {
        &[SPRIGANT_ANESTHESIA, SPRIGANT_POISON]
    }
    fn attack_npcs(&self) -> &[i32] {
        // Java also registers the ordinary dinosaurs here for their
        // parameter-driven specials — deferred (see module doc).
        &[EGG, 22215, 22216, 22217]
    }
    fn aggro_enter_npcs(&self) -> &[i32] {
        &TREX
    }
    fn spell_finished_npcs(&self) -> &[i32] {
        &TREX
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_spawn(&self, ctx: &mut QuestCtx) {
        // A Sprigant starts its 15 s trap cycle the moment it stands up.
        let npc = ctx.npc;
        ctx.world.scheduler.schedule(
            ctx.world.tick + 150,
            crate::scheduler::ScheduledTask::SprigantTrap { npc_oid: npc },
        );
    }

    /// The Tyrannosaurus notices you before it charges: a bark, a dropped
    /// aggro list, and six seconds of sizing you up (`TREX_ATTACK`).
    fn on_aggro_range_enter(&self, ctx: &mut QuestCtx) {
        if ctx.npc_script_value() != 0 {
            return;
        }
        ctx.set_npc_script_value(1);
        ctx.npc_say_text("?");
        if let Some(a) = ctx.world.objects.get_component_mut::<AggroList>(&ctx.npc) {
            a.0.clear();
        }
        let (trex, player) = (ctx.npc, ctx.player);
        ctx.world.scheduler.schedule(
            ctx.world.tick + 60,
            crate::scheduler::ScheduledTask::TrexAttack {
                trex_oid: trex,
                player_oid: player,
            },
        );
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.npc_id == EGG {
            egg_on_attack(ctx);
        } else {
            trex_on_attack(ctx);
        }
    }

    /// Java `onSpellFinished`: a finished Berserk under 60% HP locks the
    /// ladder state (script value 3) and re-fixes the most hated. (The
    /// `< 30%` else-branch is unreachable — 30 < 60 — ported as written.)
    fn on_spell_finished(&self, ctx: &mut QuestCtx, skill_id: i32) {
        if skill_id != BERSERK {
            return;
        }
        let in_combat = ctx
            .world
            .objects
            .get_component::<AggroList>(&ctx.npc)
            .is_some_and(|a| !a.0.is_empty());
        if !in_combat {
            return;
        }
        let hp = hp_percent(ctx, ctx.npc);
        if hp < 60.0 {
            ctx.set_npc_script_value(3);
            if let Some(target) = most_hated(ctx, ctx.npc) {
                let npc = ctx.npc;
                if let Some(a) = ctx.world.objects.get_component_mut::<AggroList>(&npc) {
                    a.0.entry(target).or_default().hate += 555.0;
                }
                crate::game_loop::npc_ai::seed_attack(ctx.world, npc, target);
            }
        }
    }
}

/// Striking an Ancient Egg (80%, once) wakes the jungle: every monster
/// within 500 has a coin-flip chance to turn on the striker.
fn egg_on_attack(ctx: &mut QuestCtx) {
    if ctx.npc_script_value() != 0 || ctx.roll(100) > 80 {
        return;
    }
    ctx.set_npc_script_value(1);
    let Some(origin) = ctx
        .world
        .objects
        .get_component::<Position>(&ctx.npc)
        .copied()
    else {
        return;
    };
    let egg_oid = ctx.npc;
    let mut nearby: Vec<i32> = Vec::new();
    ctx.world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Position, &Vitals)>(|(n, p, v)| {
            if n.object_id != egg_oid && !v.dead && origin.distance_2d(p) <= 500.0 {
                nearby.push(n.object_id);
            }
        });
    for mob in nearby {
        // `isSummon ? killer.getServitors()… : killer` — the newcomer goes for
        // whatever landed the kill, pet included.
        if ctx.roll(2) == 0 {
            let target = ctx.killing_playable();
            crate::game_loop::npc_ai::seed_attack(ctx.world, mob, target);
        }
    }
}

/// The Trex combat ladder — Berserk by HP band and the stun/silence/spin
/// specials whose odds scale with the ladder state (script value).
fn trex_on_attack(ctx: &mut QuestCtx) {
    let npc = ctx.npc;
    let sv = ctx.npc_script_value();
    let hp = hp_percent(ctx, npc);
    if hp <= 30.0 {
        if sv == 3 {
            cast(ctx, npc, npc, BERSERK, 1);
        } else if sv == 1 {
            cast(ctx, npc, npc, BERSERK, 2);
        }
    } else if hp <= 60.0 && sv == 3 {
        cast(ctx, npc, npc, BERSERK, 1);
    }

    let far = {
        let a = ctx.world.objects.get_component::<Position>(&npc).copied();
        let b = ctx
            .world
            .objects
            .get_component::<Position>(&ctx.player)
            .copied();
        match (a, b) {
            (Some(a), Some(b)) => a.distance_2d(&b) > 100.0,
            _ => return,
        }
    };
    if far {
        if ctx.roll(100) <= 10 * sv {
            let target = ctx.player;
            cast(ctx, npc, target, LONG_RANGE_STUN, 1);
        }
        return;
    }
    let Some(mh) = most_hated(ctx, npc) else {
        return;
    };
    if ctx.roll(100) <= 10 * sv {
        cast(ctx, npc, mh, LONG_RANGE_STUN, 1);
    }
    if ctx.roll(100) <= 5 * sv {
        cast(ctx, npc, mh, SPECIAL_STUN, 4);
    }
    if ctx.roll(100) <= 3 * sv {
        cast(ctx, npc, mh, SPECIAL_SILENCE, 4);
    }
    if ctx.roll(100) <= 5 * sv {
        cast(ctx, npc, mh, SPECIAL_SPIN, 4);
    }
}

fn cast(ctx: &mut QuestCtx, npc: i32, target: i32, skill_id: i32, level: i32) {
    if let Some(skill) = ctx.world.data.skill_data.get(skill_id, level).cloned()
        && crate::game_loop::npc_cast::check_use_conditions_pub(ctx.world, npc, &skill)
    {
        crate::game_loop::npc_cast::start_cast(ctx.world, npc, target, &skill);
    }
}

fn hp_percent(ctx: &QuestCtx, oid: i32) -> f64 {
    ctx.world
        .objects
        .get_component::<Vitals>(&oid)
        .map(|v| v.cur_hp / v.max_hp as f64 * 100.0)
        .unwrap_or(100.0)
}

fn most_hated(ctx: &QuestCtx, oid: i32) -> Option<i32> {
    ctx.world
        .objects
        .get_component::<AggroList>(&oid)
        .and_then(|a| {
            a.0.iter()
                .max_by(|x, y| x.1.hate.total_cmp(&y.1.hate))
                .map(|(k, _)| *k)
        })
}

/// The Sprigant trap beat: cast the trap AoE and re-arm, forever (they are
/// stationary plants; the beat dies with the NPC).
pub(crate) fn handle_sprigant_trap(world: &mut crate::world::World, npc_oid: i32) {
    let Some(npc_id) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map(|n| n.npc_id)
    else {
        return;
    };
    let dead = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_none_or(|v| v.dead);
    if dead {
        return;
    }
    let skill_id = if npc_id == SPRIGANT_ANESTHESIA {
        ANESTHESIA
    } else {
        DEADLY_POISON
    };
    if let Some(skill) = world.data.skill_data.get(skill_id, 1).cloned()
        && crate::game_loop::npc_cast::check_use_conditions_pub(world, npc_oid, &skill)
    {
        crate::game_loop::npc_cast::start_cast(world, npc_oid, npc_oid, &skill);
    }
    world.scheduler.schedule(
        world.tick + 150,
        crate::scheduler::ScheduledTask::SprigantTrap { npc_oid },
    );
}

/// `TREX_ATTACK` — six seconds after noticing you: still within 800? The
/// sizing-up is over.
pub(crate) fn handle_trex_attack(world: &mut crate::world::World, trex_oid: i32, player_oid: i32) {
    if let Some(n) = world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&trex_oid)
    {
        n.script_value = 0;
    }
    let close = {
        let a = world.objects.get_component::<Position>(&trex_oid).copied();
        let b = world
            .objects
            .get_component::<Position>(&player_oid)
            .copied();
        match (a, b) {
            (Some(a), Some(b)) => a.distance_2d(&b) <= 800.0,
            _ => false,
        }
    };
    if !close {
        return;
    }
    if let Some(skill) = world.data.skill_data.get(LONG_RANGE_STUN, 1).cloned()
        && crate::game_loop::npc_cast::check_use_conditions_pub(world, trex_oid, &skill)
    {
        crate::game_loop::npc_cast::start_cast(world, trex_oid, player_oid, &skill);
    }
    crate::game_loop::npc_ai::seed_attack(world, trex_oid, player_oid);
}
