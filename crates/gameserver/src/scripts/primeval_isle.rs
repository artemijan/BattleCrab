//! `ai/areas/PrimevalIsle` — the island's signature behaviors: Ancient Eggs
//! that raise the jungle when struck, Sprigant poison traps, the
//! Tyrannosaurus's curiosity pause + berserk ladder, and the sight-driven AI
//! over the creature-see hook (`addCreatureSeeId` → the 1 s sweep): the
//! Deino/Ornit herd flee, the `ag_type`-gated on-sight specials, and the
//! Trex hunting herbivores (`CREW_SKILL` presentation 6172). The ordinary
//! dinosaurs also run Java's parameter-driven `onAttack` block (the
//! `SKILL_MULTIPLER` HP bands, the one-shot self range buff, and the
//! most-hated specials). Skipped as off-chronicle: the Deinonychus Mesozoic
//! Stone taming reward (Gracia-era item 14828).

use crate::game_loop::helpers::is_dead;
use crate::game_loop::npc::ai;
use crate::game_loop::npc::npc_id_of;
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::game_loop::space::position::maybe_position;
use crate::model::components::{Position, Vitals};
use crate::model::npc::AggroList;

const EGG: i32 = 18344;
pub(crate) const SPRIGANT_ANESTHESIA: i32 = 18345;
pub(crate) const SPRIGANT_POISON: i32 = 18346;
const TREX: [i32; 3] = [22215, 22216, 22217];
const ORNIT: i32 = 22742;
const DEINO: i32 = 22743;
/// Java `MONSTERS` — the ordinary dinosaurs (creature-see + parameter AI).
const MONSTERS: [i32; 17] = [
    22196, 22198, 22200, 22202, 22203, 22205, 22208, 22210, 22211, 22213, 22223, 22224, 22225,
    22226, 22227, ORNIT, DEINO,
];
/// Java `VEGETABLE` — the herbivores a Tyrannosaurus hunts on sight.
const VEGETABLE: [i32; 8] = [22200, 22201, 22202, 22203, 22204, 22205, 22224, 22225];
/// The attack listeners: the egg, the three Trex, and the ordinary dinosaurs.
const ATTACK_NPCS: [i32; 21] = [
    EGG, 22215, 22216, 22217, 22196, 22198, 22200, 22202, 22203, 22205, 22208, 22210, 22211, 22213,
    22223, 22224, 22225, 22226, 22227, ORNIT, DEINO,
];
/// Creature-see listeners: Java registers TREX + MONSTERS.
const CREATURE_SEE_NPCS: [i32; 20] = [
    22215, 22216, 22217, 22196, 22198, 22200, 22202, 22203, 22205, 22208, 22210, 22211, 22213,
    22223, 22224, 22225, 22226, 22227, ORNIT, DEINO,
];

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
    fn attack_npcs(&self) -> &[i32] {
        &ATTACK_NPCS
    }
    fn spawn_npcs(&self) -> &[i32] {
        &[SPRIGANT_ANESTHESIA, SPRIGANT_POISON]
    }
    fn aggro_enter_npcs(&self) -> &[i32] {
        &TREX
    }
    fn spell_finished_npcs(&self) -> &[i32] {
        &TREX
    }
    fn creature_see_npcs(&self) -> &[i32] {
        &CREATURE_SEE_NPCS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.npc_id == EGG {
            egg_on_attack(ctx);
        } else if TREX.contains(&ctx.npc_id) {
            trex_on_attack(ctx);
        } else {
            monster_on_attack(ctx);
        }
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
                ai::seed_attack(ctx.world, npc, target);
            }
        }
    }

    /// Java `onCreatureSee`: an ordinary dinosaur noticing a *player* either
    /// flees with the herd (Deino 30 %, Ornit once) or opens with an
    /// `ag_type`-gated special; a Tyrannosaurus noticing a *herbivore* hunts
    /// it with the Presentation - Tyranno crew skill.
    fn on_creature_see(&self, ctx: &mut QuestCtx, creature: i32) {
        if MONSTERS.contains(&ctx.npc_id) {
            monster_on_creature_see(ctx, creature);
        } else {
            trex_on_creature_see(ctx, creature);
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
            ai::seed_attack(ctx.world, mob, target);
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
            ctx.npc_cast(npc, npc, BERSERK, 1);
        } else if sv == 1 {
            ctx.npc_cast(npc, npc, BERSERK, 2);
        }
    } else if hp <= 60.0 && sv == 3 {
        ctx.npc_cast(npc, npc, BERSERK, 1);
    }

    let far = {
        let a = maybe_position(ctx.world, npc);
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
            ctx.npc_cast(npc, target, LONG_RANGE_STUN, 1);
        }
        return;
    }
    let Some(mh) = most_hated(ctx, npc) else {
        return;
    };
    if ctx.roll(100) <= 10 * sv {
        ctx.npc_cast(npc, mh, LONG_RANGE_STUN, 1);
    }
    if ctx.roll(100) <= 5 * sv {
        ctx.npc_cast(npc, mh, SPECIAL_STUN, 4);
    }
    if ctx.roll(100) <= 3 * sv {
        ctx.npc_cast(npc, mh, SPECIAL_SILENCE, 4);
    }
    if ctx.roll(100) <= 5 * sv {
        ctx.npc_cast(npc, mh, SPECIAL_SPIN, 4);
    }
}

/// Java `onCreatureSee`, the MONSTERS × player arm: Deino (30 %) and Ornit
/// (first sighting) spook and lead the herd away — aggro dropped, running
/// 3000 units directly away from the player; everyone else with `ag_type` 1
/// opens with a parameter special. `SKILL_MULTIPLER` reads the value the last
/// `onAttack` set — 0 before first blood, so an unbloodied special only fires
/// on a rolled 0 (Java's `getRandom(100) <= prob * 0`), ported as-is.
fn monster_on_creature_see(ctx: &mut QuestCtx, creature: i32) {
    if ctx.player == 0 {
        return; // only players spook the herd (Java `creature.isPlayer()`)
    }
    let npc = ctx.npc;
    if (ctx.npc_id == DEINO && ctx.roll(100) < 30)
        || (ctx.npc_id == ORNIT && ctx.npc_script_value() == 0)
    {
        if let Some(a) = ctx.world.objects.get_component_mut::<AggroList>(&npc) {
            a.0.clear();
        }
        ctx.set_npc_script_value(1);
        set_running(ctx, npc);
        // `calculateHeadingFrom(creature, npc)` — the direction from the
        // player *to* the dino, extended 3000 units: straight away.
        let (from, at) = (
            maybe_position(ctx.world, creature),
            maybe_position(ctx.world, npc),
        );
        let (Some(from), Some(at)) = (from, at) else {
            return;
        };
        let (dx, dy) = (f64::from(at.x - from.x), f64::from(at.y - from.y));
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (nx, ny) = (
            at.x + (dx / len * 3000.0) as i32,
            at.y + (dy / len * 3000.0) as i32,
        );
        ai::move_npc_to(ctx.world, npc, nx, ny, at.z);
        return;
    }
    let Some(tpl) = ctx.world.data.npc_data.get(ctx.npc_id) else {
        return;
    };
    let ag_type = tpl
        .ai_params
        .get("ag_type")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    if ag_type != 1 {
        return;
    }
    let prob1 = ai_int(ctx, "ProbPhysicalSpecial1");
    let prob2 = ai_int(ctx, "ProbPhysicalSpecial2");
    let s1 = tpl.ai_skill_params.get("PhysicalSpecial1").copied();
    let s2 = tpl.ai_skill_params.get("PhysicalSpecial2").copied();
    let mult = npc_var(ctx, "SKILL_MULTIPLER");
    if ctx.roll(100) <= prob1 * mult {
        if let Some((id, lvl)) = s1 {
            ctx.npc_cast(npc, creature, id, lvl);
        }
    } else if ctx.roll(100) <= prob2 * mult
        && let Some((id, lvl)) = s2
    {
        ctx.npc_cast(npc, creature, id, lvl);
    }
}

/// Java `onCreatureSee`, the else arm: a Tyrannosaurus sighting a herbivore
/// hunts it — the Presentation - Tyranno crew skill, then a running charge.
fn trex_on_creature_see(ctx: &mut QuestCtx, creature: i32) {
    const CREW_SKILL: i32 = 6172;
    let prey_id = npc_id_of(ctx.world, creature);
    if !prey_id.is_some_and(|id| VEGETABLE.contains(&id)) {
        return;
    }
    let npc = ctx.npc;
    ctx.npc_cast(npc, creature, CREW_SKILL, 1);
    set_running(ctx, npc);
    ai::seed_attack(ctx.world, npc, creature);
}

/// Java `onAttack`'s ordinary-dinosaur block: set `SKILL_MULTIPLER` from the
/// HP band, fire the one-shot self range buff at 30 % (clearing aggro and
/// re-fixing the most hated), and — only on the hit that popped the buff —
/// roll both parameter specials at that target.
fn monster_on_attack(ctx: &mut QuestCtx) {
    let npc = ctx.npc;
    let hp = hp_percent(ctx, npc);
    set_npc_var(ctx, "SKILL_MULTIPLER", if hp <= 50.0 { 2 } else { 1 });

    let mut target = None;
    if hp <= 30.0 && npc_var(ctx, "SELFBUFF_USED") == 0 {
        target = most_hated(ctx, npc);
        if let Some(a) = ctx.world.objects.get_component_mut::<AggroList>(&npc) {
            a.0.clear();
        }
        let buff = ctx
            .world
            .data
            .npc_data
            .get(ctx.npc_id)
            .and_then(|t| t.ai_skill_params.get("SelfRangeBuff1").copied());
        if let Some((id, lvl)) = buff {
            set_npc_var(ctx, "SELFBUFF_USED", 1);
            ctx.npc_cast(npc, npc, id, lvl);
            set_running(ctx, npc);
            if let Some(t) = target {
                ai::seed_attack(ctx.world, npc, t);
            }
        }
    }
    // Java: `if (target != null)` — the specials ride the self-buff hit only.
    let Some(t) = target else {
        return;
    };
    let (prob1, prob2) = (
        ai_int(ctx, "ProbPhysicalSpecial1"),
        ai_int(ctx, "ProbPhysicalSpecial2"),
    );
    let tpl_skills = ctx.world.data.npc_data.get(ctx.npc_id).map(|tpl| {
        (
            tpl.ai_skill_params.get("PhysicalSpecial1").copied(),
            tpl.ai_skill_params.get("PhysicalSpecial2").copied(),
        )
    });
    let Some((s1, s2)) = tpl_skills else { return };
    let mult = npc_var(ctx, "SKILL_MULTIPLER");
    if ctx.roll(100) <= prob1 * mult
        && let Some((id, lvl)) = s1
    {
        ctx.npc_cast(npc, t, id, lvl);
    }
    if ctx.roll(100) <= prob2 * mult
        && let Some((id, lvl)) = s2
    {
        ctx.npc_cast(npc, t, id, lvl);
    }
}

/// Java `npc.setRunning()` — the run flag lives on the `Speeds` component.
fn set_running(ctx: &mut QuestCtx, npc: i32) {
    if let Some(sp) = ctx
        .world
        .objects
        .get_component_mut::<crate::model::components::Speeds>(&npc)
    {
        sp.running = true;
    }
}

/// An integer `<param>` off the template (0 when absent, like Java's
/// `getParameters().getInt(name, 0)`).
fn ai_int(ctx: &QuestCtx, name: &str) -> i32 {
    ctx.world
        .data
        .npc_data
        .get(ctx.npc_id)
        .and_then(|t| t.ai_params.get(name))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}

/// Java `npc.getVariables().getInt(name)` — 0 when unset.
fn npc_var(ctx: &QuestCtx, name: &str) -> i32 {
    ctx.world
        .objects
        .get_component::<crate::model::npc::Npc>(&ctx.npc)
        .and_then(|n| n.vars.get(name).copied())
        .unwrap_or(0)
}

fn set_npc_var(ctx: &mut QuestCtx, name: &str, value: i32) {
    if let Some(n) = ctx
        .world
        .objects
        .get_component_mut::<crate::model::npc::Npc>(&ctx.npc)
    {
        n.vars.insert(name.to_string(), value);
    }
}

/// `100.0` when the answer is unavailable, as in `npc_cast`.
///
/// This copy had no zero-maximum guard and divided anyway, yielding a `NaN`.
/// All three callers below compare with `<` or `<=`, and `NaN` fails both, so
/// the effect was already "treat as full health" — the same answer this now
/// gives on purpose rather than by accident.
fn hp_percent(ctx: &QuestCtx, oid: i32) -> f64 {
    crate::game_loop::helpers::hp_fraction(ctx.world, oid).map_or(100.0, |f| f * 100.0)
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
    let Some(npc_id) = npc_id_of(world, npc_oid) else {
        return;
    };
    let dead = is_dead(world, npc_oid);
    if dead {
        return;
    }
    let skill_id = if npc_id == SPRIGANT_ANESTHESIA {
        ANESTHESIA
    } else {
        DEADLY_POISON
    };
    crate::game_loop::npc::cast::cast_skill(world, npc_oid, npc_oid, skill_id, 1);
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
        let a = maybe_position(world, trex_oid);
        let b = maybe_position(world, player_oid);
        match (a, b) {
            (Some(a), Some(b)) => a.distance_2d(&b) <= 800.0,
            _ => false,
        }
    };
    if !close {
        return;
    }
    crate::game_loop::npc::cast::cast_skill(world, trex_oid, player_oid, LONG_RANGE_STUN, 1);
    ai::seed_attack(world, trex_oid, player_oid);
}
