//! Cubics (G29) — port of `model/cubic/Cubic` + `effecthandlers/SummonCubic`.
//!
//! A cubic is a satellite that periodically casts a skill for its owner. Unlike
//! a servitor it is **not a world object**: it has no template, no position and
//! no AI, and lives entirely on the player as a [`Cubics`] component. Other
//! players see it only as an id in the owner's `CharInfo`.
//!
//! 12 of the 28 `SummonCubic` skills are learnable on this dist — which is what
//! put cubics ahead of agathions (166 skills, none of them learnable).

use crate::data::cubic_data::{CubicSkill, CubicTargetType, CubicTemplate};
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::target;
use crate::game_loop::time::TICKS_PER_SECOND;
use crate::geo::distance::within_3d;
use crate::model::components::{Position, Vitals};
use crate::world::World;

/// Java `Stat.MAX_CUBIC`'s default. **Nothing in this datapack sets
/// `cubicCount`** — Cubic Mastery does not exist on Interlude Classic — so the
/// allowance is always 1 and a second cubic always displaces the first.
/// Not a deferral — there is no work pending behind this. Read the stat if a
/// `cubicCount` carrier is ever added to the datapack.
const MAX_CUBIC: usize = 1;

/// One live cubic on a player.
#[derive(Debug, Clone, Copy)]
pub struct ActiveCubic {
    pub id: i32,
    pub level: i32,
    /// Written at summon time; no reader until the cubic display packet
    /// (`ExUserInfoCubic`-shaped) stops hard-coding its count.
    #[allow(dead_code)]
    pub slot: i32,
    /// The cubic's own object id.
    ///
    /// Java's `Cubic extends Creature`, and **the cubic — not its owner — is
    /// the caster**: `skill.activateSkill(this, target)`, with the cubic's
    /// `getBasePAtk()`/`getBaseMAtk()` both returning the template's
    /// `power / 10`. Passing the owner instead scaled cubic damage off the
    /// *player's* m.atk, which for a levelled mage is many times the intended
    /// value (Storm Cubic level 1 is `power=282` → m.atk **28.2**).
    ///
    /// So the cubic gets a real entity carrying `CombatStats` and `Vitals`,
    /// but **no `Npc`, `Player`, `RegionCell` or `Movement`** — every store
    /// sweep in the server is anchored on one of those, so it stays invisible
    /// to visibility, targeting, movement and AI while still being a valid
    /// caster for the damage formulas.
    pub caster_oid: i32,
    /// Absolute tick the cubic expires (`duration` seconds after summon).
    pub expires_at_tick: u64,
    /// Actions left before it goes away (`maxCount`); `i32::MAX` when the
    /// template says unlimited.
    pub remaining_count: i32,
}

/// Java `Player.getCubics()` — keyed by cubic id so re-summoning the same cubic
/// replaces rather than stacks.
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct Cubics(pub Vec<ActiveCubic>);

impl Cubics {
    pub fn ids(&self) -> Vec<i32> {
        self.0.iter().map(|c| c.id).collect()
    }
}

/// `effecthandlers/SummonCubic.instant`.
///
/// Java refuses when the owner is dead, mounted or observing. Re-summoning a
/// cubic the player already has **refuses outright if the existing one is a
/// higher level** ("What do we do in such case?" in the Java, which returns);
/// otherwise it replaces it.
pub(crate) fn summon_cubic(world: &mut World, owner_oid: i32, cubic_id: i32, cubic_level: i32) {
    if cubic_id < 0 {
        return;
    }
    if world
        .objects
        .get_component::<crate::model::Player>(&owner_oid)
        .is_none()
    {
        return;
    }
    if is_dead(world, owner_oid) {
        return;
    }
    let Some(template) = world.data.cubic_data.get(cubic_id, cubic_level).cloned() else {
        return;
    };

    let existing = world
        .objects
        .get_component::<Cubics>(&owner_oid)
        .and_then(|c| c.0.iter().find(|c| c.id == cubic_id).copied());
    if let Some(old) = existing {
        // A higher-level cubic is not downgraded by re-casting a weaker one.
        if old.level > cubic_level {
            return;
        }
        remove_cubic(world, owner_oid, cubic_id);
    } else {
        // At the cap, Java drops a *random* existing cubic. With MAX_CUBIC == 1
        // on this dist that is always "the only one", but keep the shape.
        let count = world
            .objects
            .get_component::<Cubics>(&owner_oid)
            .map(|c| c.0.len())
            .unwrap_or(0);
        if count >= MAX_CUBIC {
            let victim = world
                .objects
                .get_component::<Cubics>(&owner_oid)
                .and_then(|c| c.0.first().map(|c| c.id));
            if let Some(v) = victim {
                remove_cubic(world, owner_oid, v);
            }
        }
    }

    let Some(caster_oid) = spawn_cubic_caster(world, owner_oid, &template) else {
        return;
    };
    let active = ActiveCubic {
        id: cubic_id,
        level: cubic_level,
        slot: template.slot,
        caster_oid,
        expires_at_tick: world.tick + (template.duration.max(0) as u64) * TICKS_PER_SECOND,
        remaining_count: if template.max_count > 0 {
            template.max_count
        } else {
            i32::MAX
        },
    };
    if world.objects.get_component::<Cubics>(&owner_oid).is_none() {
        world.objects.add_components(&owner_oid, Cubics::default());
    }
    if let Some(c) = world.objects.get_component_mut::<Cubics>(&owner_oid) {
        c.0.push(active);
    }

    // Java's `activate()` schedules at a *fixed rate starting immediately*
    // (`scheduleAtFixedRate(..., 0, delay)`), so the cubic acts on the same
    // tick it is summoned rather than after one delay.
    schedule_action(world, owner_oid, cubic_id, 0);
    broadcast_cubic_change(world, owner_oid);
}

fn schedule_action(world: &mut World, owner_oid: i32, cubic_id: i32, delay_secs: u64) {
    world.scheduler.schedule(
        world.tick + delay_secs * TICKS_PER_SECOND,
        crate::scheduler::ScheduledTask::CubicAction {
            owner_oid,
            cubic_id,
        },
    );
}

/// The cubic's stats-only caster entity.
///
/// `power / 10` is Java's, in `CubicTemplate`'s constructor — the XML value is
/// ten times the base attack, and dropping the divide would overstate every
/// cubic hit tenfold.
fn spawn_cubic_caster(world: &mut World, owner_oid: i32, template: &CubicTemplate) -> Option<i32> {
    let oid = world.alloc_object_id()?;
    let atk = template.power / 10.0;
    let mut stats = crate::model::components::CombatStats::default();
    stats.p_atk = atk;
    stats.m_atk = atk;
    // `spawn` first — `add_components` silently no-ops on an id the store has
    // never seen, which is how the first draft of this ended up with a caster
    // that had no stats at all.
    world.objects.spawn(oid, stats);
    // The level link — see `CubicOf`. Without it every cast is resisted.
    world.objects.add_components(
        &oid,
        crate::model::components::CubicOf {
            owner_object_id: owner_oid,
        },
    );
    // A cubic is never attacked, but the damage path reads the caster's
    // vitals; give it something alive rather than a zeroed corpse.
    world.objects.add_components(&oid, Vitals::hp_full(1, 1));
    // Position is read for range/aggro bookkeeping; the cubic floats at its
    // owner, and follows them because it is re-read at cast time.
    if let Some(pos) = maybe_position(world, owner_oid) {
        world.objects.add_components(&oid, pos);
    }
    Some(oid)
}

/// Java `Cubic.deactivate()` — drop it and tell everyone.
pub(crate) fn remove_cubic(world: &mut World, owner_oid: i32, cubic_id: i32) {
    let caster = world
        .objects
        .get_component::<Cubics>(&owner_oid)
        .and_then(|c| c.0.iter().find(|c| c.id == cubic_id).map(|c| c.caster_oid));
    if let Some(c) = world.objects.get_component_mut::<Cubics>(&owner_oid) {
        c.0.retain(|c| c.id != cubic_id);
    }
    // The caster entity dies with the cubic, or it leaks one entity per summon.
    if let Some(caster) = caster {
        world.objects.despawn(&caster);
    }
    broadcast_cubic_change(world, owner_oid);
}

/// Cubics do not survive their owner leaving the world (Java drops them in
/// `deleteMe`); nothing persists them.
pub(crate) fn on_owner_leave_world(world: &mut World, owner_oid: i32) {
    if let Some(c) = world.objects.get_component_mut::<Cubics>(&owner_oid) {
        c.0.clear();
    }
}

/// `Cubic.readyToUseSkill` — one action attempt, then reschedule.
pub(crate) fn handle_cubic_action(world: &mut World, owner_oid: i32, cubic_id: i32) {
    let Some(active) = world
        .objects
        .get_component::<Cubics>(&owner_oid)
        .and_then(|c| c.0.iter().find(|c| c.id == cubic_id).copied())
    else {
        return; // deactivated — the chain ends.
    };
    // Owner gone or dead → stop, the same contract the servitor ticks use.
    if is_dead(world, owner_oid) {
        remove_cubic(world, owner_oid, cubic_id);
        return;
    }
    if world.tick >= active.expires_at_tick {
        remove_cubic(world, owner_oid, cubic_id);
        return;
    }
    let Some(template) = world.data.cubic_data.get(cubic_id, active.level).cloned() else {
        return;
    };

    if try_action(world, owner_oid, active.caster_oid, &template) {
        // `maxCount` counts *actions*, not attempts — a cubic that fails its
        // success roll has not spent one of its charges.
        let mut exhausted = false;
        if let Some(c) = world.objects.get_component_mut::<Cubics>(&owner_oid)
            && let Some(a) = c.0.iter_mut().find(|c| c.id == cubic_id)
            && a.remaining_count != i32::MAX
        {
            a.remaining_count -= 1;
            exhausted = a.remaining_count <= 0;
        }
        if exhausted {
            remove_cubic(world, owner_oid, cubic_id);
            return;
        }
    }

    schedule_action(world, owner_oid, cubic_id, template.delay.max(1) as u64);
}

/// Returns true when a skill actually fired.
fn try_action(
    world: &mut World,
    owner_oid: i32,
    caster_oid: i32,
    template: &CubicTemplate,
) -> bool {
    // `<hp type="GREATER" percent="33"/>` gates the *owner*: a badly wounded
    // player's attack cubic stops firing.
    if let Some(cond) = template.hp_condition {
        let pct = hp_percent(world, owner_oid);
        let ok = if cond.greater {
            pct > cond.percent as f64
        } else {
            pct < cond.percent as f64
        };
        if !ok {
            return false;
        }
    }

    let Some(skill) = choose_skill(world, template) else {
        return false;
    };

    let target = match template.target_type {
        CubicTargetType::Master => Some(owner_oid),
        CubicTargetType::Heal => heal_target(world, owner_oid, template),
        // `BY_SKILL` defers to the nested skill's own target type; the only
        // kinds this dist nests are TARGET, HEAL and MASTER.
        CubicTargetType::BySkill => match skill.target_type.unwrap_or(CubicTargetType::Target) {
            CubicTargetType::Master => Some(owner_oid),
            CubicTargetType::Heal => heal_target(world, owner_oid, template),
            _ => live_target(world, owner_oid),
        },
        CubicTargetType::Target => live_target(world, owner_oid),
    };
    let Some(target) = target else { return false };

    if !in_range(world, owner_oid, target, template) {
        return false;
    }
    if let Some((min, max)) = template.health_percent {
        let pct = hp_percent(world, target);
        if pct < min as f64 || pct > max as f64 {
            return false;
        }
    }
    // `Rnd.get(100) < successRate` — rolled after the skill is chosen, so it
    // gates the cast, not the choice.
    if world.roll(100) >= skill.success_rate {
        return false;
    }

    cast(world, owner_oid, caster_oid, target, &skill);
    true
}

/// `Cubic.chooseSkill` — cumulative `triggerRate` weights over a single
/// `Rnd.nextDouble() * 100`, so the weights are shares of 100 rather than
/// independent rolls.
fn choose_skill(world: &mut World, template: &CubicTemplate) -> Option<CubicSkill> {
    let roll = world.roll(100);
    let mut cumulative = 0;
    for s in &template.skills {
        cumulative += s.trigger_rate;
        if cumulative > roll {
            return Some(s.clone());
        }
    }
    None
}

/// The owner's target, but only while it is still alive.
///
/// Deliberately **not** [`target::current`], which answers the
/// raw selection: a cubic that fires at a corpse wastes its cast and its reuse.
/// The name says `live_` so the next reader does not "collapse" it into the
/// plain resolver the way the identical-looking copies elsewhere were.
fn live_target(world: &World, owner_oid: i32) -> Option<i32> {
    let target = target::current(world, owner_oid)?;
    // A dead target is not worth a cast.
    if is_dead(world, target) {
        return None;
    }
    Some(target)
}

/// `Cubic.actionHeal` — the most wounded of the owner and their party, skipping
/// the dead ("Life Cubic should not try to heal dead targets").
fn heal_target(world: &mut World, owner_oid: i32, _template: &CubicTemplate) -> Option<i32> {
    let mut candidates = vec![owner_oid];
    if let Some(crate::model::components::PartyRef(pid)) = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&owner_oid)
        .copied()
        && let Some(party) = world.parties.get(&pid)
    {
        candidates.extend(party.members.iter().copied().filter(|m| *m != owner_oid));
    }
    candidates
        .into_iter()
        .filter(|oid| {
            world
                .objects
                .get_component::<Vitals>(oid)
                .is_some_and(|v| !v.dead)
        })
        .filter(|oid| in_heal_range(world, owner_oid, *oid))
        .min_by(|a, b| hp_percent(world, *a).total_cmp(&hp_percent(world, *b)))
}

/// `0.0` when the answer is unavailable — the opposite default to
/// `npc_cast::hp_percent`, and deliberately so: this one picks the *most hurt*
/// ally to heal, and an unreadable candidate must not win that comparison by
/// looking healthy.
fn hp_percent(world: &World, oid: i32) -> f64 {
    crate::game_loop::helpers::hp_fraction(world, oid).map_or(0.0, |f| f * 100.0)
}

fn in_range(world: &World, owner_oid: i32, target: i32, template: &CubicTemplate) -> bool {
    let Some(range) = template.range else {
        return true;
    };
    within_3d(world, owner_oid, target, range as f64)
}

/// Java heals within `Config.ALT_PARTY_RANGE`.
const PARTY_RANGE: f64 = 1500.0;

fn in_heal_range(world: &World, owner_oid: i32, target: i32) -> bool {
    if owner_oid == target {
        return true;
    }
    within_3d(world, owner_oid, target, PARTY_RANGE)
}

/// `Cubic.activateCubicSkill` — the cast animation is broadcast **from the
/// owner**, since the cubic has no object id of its own, and the effects are
/// applied with the owner as caster.
fn cast(world: &mut World, owner_oid: i32, caster_oid: i32, target: i32, cubic_skill: &CubicSkill) {
    let Some(skill) = skill_by_id(world, cubic_skill.skill_id, cubic_skill.skill_level) else {
        return;
    };
    let target_pos = maybe_position(world, target);
    if let (Some(caster), Some(pos), Some(tp)) = (
        world
            .objects
            .get_component::<crate::model::Player>(&owner_oid),
        maybe_position(world, owner_oid),
        target_pos,
    ) {
        let pkt = crate::network::server_packets::magic_skill_use(
            caster,
            &pos,
            (target, tp.x, tp.y, tp.z),
            skill.id,
            skill.level,
            skill.hit_time,
            skill.reuse_delay_group,
            skill.reuse_delay,
        );
        crate::game_loop::helpers::broadcast_including_self(world, owner_oid, &pkt);
    }
    // The cubic floats with its owner — keep its position current so range and
    // aggro bookkeeping resolve from where the owner actually is.
    if let Some(pos) = maybe_position(world, owner_oid)
        && let Some(p) = world.objects.get_component_mut::<Position>(&caster_oid)
    {
        *p = pos;
    }
    // `skill.activateSkill(this, target)` — **the cubic** is the caster, so the
    // damage scales off the template's power, not the owner's m.atk.
    crate::game_loop::skills::effects::apply_skill_effects(world, caster_oid, target, &skill);
}

fn broadcast_cubic_change(world: &mut World, owner_oid: i32) {
    // Other players learn a cubic's existence from the owner's `CharInfo`,
    // which carries the id list — this chronicle has no incremental cubic
    // packet, so the whole record is re-sent. `UserInfo` carries no cubic
    // field, so the owner's own client learns of it from the cast animation.
    crate::game_loop::visibility::refresh_char_info(world, owner_oid);
}
