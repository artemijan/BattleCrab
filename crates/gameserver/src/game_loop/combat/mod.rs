//! The auto-attack pipeline (G9): `AttackRequest` handling, the player
//! intent think loops (`PlayerAI.thinkAttack`/`thinkCast` +
//! `CreatureFollowTask` — chase into attack or cast range, then act), the
//! shared swing/hit mechanics (`Creature.doAutoAttack` →
//! `CreatureAttackTaskManager` → `onHitTimeNotDual` → `onHitTarget`), and the
//! combat-stance tracker (`AttackStanceTaskManager`).
//!
//! Scope (see PROGRESS G9): melee swings only — bows/crossbows, dual-weapon
//! split hits, polearm sweeps, soulshots, and shield blocks are all deferred
//! (their formula terms are identity for the actors that exist). PvP
//! auto-attack (force-attacking players) is deferred with the PvP-flag
//! system.
//!
//! This file keeps `Combatant` and the stat readers (crit rates, crit damage,
//! shield). The rest is re-exported, so callers keep saying `combat::…`:
//!
//! - `intent` — what a player is trying to do and the movement that gets them
//!   there: attack/pickup/cast/interact intents, the per-tick think functions,
//!   chasing a moving pawn, and door swings.
//! - `attack` — landing one swing: `do_auto_attack`, polearm sweep targets,
//!   and the client's attack-hit callback.
//! - `damage` — applying it: HP/MP absorb, reflect, servitor transfer, and the
//!   NPC and player receive-damage paths.

use crate::game_loop::common::maybe_distance_too_far;
use crate::model::PlayerIntent;
use crate::model::components::{
    AttackState, Casting, Collision, CombatStats, Following, Intent, MoveToPawnState, Movement,
    PlayerVitals, Position, Speeds, Vitals,
};
use crate::model::formulas;
use crate::model::movement::{self, MoveData, get_position};
use crate::model::npc::{AggroList, NpcAi, NpcIntention};
use crate::model::stats::BaseStat;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::helpers::{
    broadcast_including_self, broadcast_near_region_in, client_for_player, instance_of, ms_to_ticks,
};
use super::skills::cast::break_cast;
use crate::game_loop::helpers::stat_mul;

mod attack;
mod damage;
mod intent;

pub(crate) use attack::*;
pub(crate) use damage::*;
pub(crate) use intent::*;

/// `AttackStanceTaskManager.COMBAT_TIME` (15 s) in ticks.
pub(crate) const COMBAT_STANCE_TICKS: u64 = 150;

/// NPC object ids live above this base (`model::npc::FIRST_NPC_OBJECT_ID`);
/// everything below is a persistent id (players, items).
pub(crate) fn is_npc_oid(object_id: i32) -> bool {
    object_id >= crate::model::npc::FIRST_NPC_OBJECT_ID
}

/// `Vitals` of any combat actor (one store since the world merge).
pub(crate) fn vitals_of(world: &World, object_id: i32) -> Option<&Vitals> {
    world.objects.get_component::<Vitals>(&object_id)
}

/// Whether an attack target is dead/gone across creatures and doors: a
/// breached siege gate (0 HP) counts as dead, like a corpse, so the attack
/// loop ends on it. A vanished object (no `Vitals`, no `Door`) is also "dead".
pub(crate) fn target_is_dead(world: &World, object_id: i32) -> bool {
    if let Some(door) = world
        .objects
        .get_component::<crate::model::door::Door>(&object_id)
    {
        return door.current_hp <= 0;
    }
    vitals_of(world, object_id).is_none_or(|v| v.dead)
}

/// The combat-relevant view of a player or NPC — the stat finalizer outputs
/// both kinds of combatant feed into the shared `Formulas` ports. NPC values
/// are derived on demand from the template (same finalizer math the player's
/// `recalculate_stats` runs: base × stat bonus × level mod), since NPCs have
/// no buff state yet.
pub(crate) struct Combatant {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub collision_radius: f64,
    pub dead: bool,
    pub p_atk: f64,
    pub p_def: f64,
    pub crit_stat: f64,
    pub accuracy: i32,
    pub evasion: i32,
    pub p_atk_spd: i32,
    pub random_dmg: i32,
    pub atk_range: i32,
    /// Shield block defence (`getShldDef`, added to pDef on a normal block) —
    /// 0 when no shield is equipped in the left hand.
    pub shield_def: f64,
    /// Shield block *rate* already multiplied by this actor's CON bonus
    /// (`SHIELD_DEFENCE_RATE × CON.calcBonus`); 0 when no shield.
    pub shield_rate: f64,
    /// This actor's CON bonus (for the perfect-block roll).
    pub con_bonus: f64,
}

/// Stand-in collision radius for a siege door's extent (the gate carries no
/// `Collision` component). Added to the attacker's reach so a swing/chase
/// lands at roughly the gate face rather than its polygon centre.
pub(crate) const DOOR_COLLISION_RADIUS: f64 = 80.0;

pub(crate) fn combatant(world: &World, object_id: i32) -> Option<Combatant> {
    // A siege door is a valid attack *target* but carries no
    // Vitals/Collision/CombatStats — synthesize a stationary combatant from
    // its Position + template pDef so the shared chase/reach geometry
    // (`distance_2d`/`maybe_move_to_pawn`/`pawn_destination`) works uniformly.
    // `dead` = breached (0 HP); the combat-stat fields are unused for a door
    // target (`do_door_swing` reads the template directly).
    if let Some(door) = world
        .objects
        .get_component::<crate::model::door::Door>(&object_id)
    {
        let pos = world.objects.get_component::<Position>(&object_id)?;
        let p_def = world
            .data
            .door_data
            .get(door.door_id)
            .map(|t| t.p_def as f64)
            .unwrap_or(0.0);
        return Some(Combatant {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            heading: pos.heading,
            collision_radius: DOOR_COLLISION_RADIUS,
            dead: door.current_hp <= 0,
            p_atk: 0.0,
            p_def,
            crit_stat: 0.0,
            accuracy: 0,
            evasion: 0,
            p_atk_spd: 0,
            random_dmg: 0,
            atk_range: 0,
            shield_def: 0.0,
            shield_rate: 0.0,
            con_bonus: 1.0,
        });
    }
    // One component-shaped path for both kinds — NPC stats are memoized
    // into `CombatStats` at spawn (`npc::npc_combat_stats`), so the old
    // per-call template derivation is gone.
    let pos = world.objects.get_component::<Position>(&object_id)?;
    let collision = world.objects.get_component::<Collision>(&object_id)?;
    let vitals = world.objects.get_component::<Vitals>(&object_id)?;
    let cs = world.objects.get_component::<CombatStats>(&object_id)?;
    let (shield_def, shield_rate, con_bonus) = shield_stats(world, object_id);
    Some(Combatant {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading: pos.heading,
        collision_radius: collision.radius,
        dead: vitals.dead,
        p_atk: cs.p_atk,
        p_def: cs.p_def,
        crit_stat: cs.crit_hit,
        accuracy: cs.accuracy,
        // `EvasionRateFinalizer` ends in `Stat.defaultValue`, whose move-type
        // term is read against the creature's *live* move type — so it belongs
        // on this per-attack snapshot rather than the cached `CombatStats`
        // (Acrobatic Move 225 grants evasion only while running, and a cached
        // value would need invalidating on every start and stop of movement).
        evasion: cs.evasion + move_type_evasion_bonus(world, object_id),
        p_atk_spd: cs.p_atk_spd,
        random_dmg: cs.random_dmg,
        atk_range: cs.atk_range,
        shield_def,
        shield_rate,
        con_bonus,
    })
}

/// The defender's `DEFENCE_CRITICAL_RATE` multiplier and `_ADD` term, both at
/// Java's identity defaults when nothing grants them.
pub(crate) fn defence_crit_rate(world: &World, target_oid: i32) -> (f64, f64) {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;
    let Some(m) = world.objects.get_component::<StatModifiers>(&target_oid) else {
        return (1.0, 0.0);
    };
    (
        m.mul
            .get(&Stat::DefenceCriticalRate)
            .copied()
            .unwrap_or(1.0),
        m.add
            .get(&Stat::DefenceCriticalRateAdd)
            .copied()
            .unwrap_or(0.0),
    )
}

/// `Formulas.calcCritDamage` / `calcCritDamageAdd`, **autoattack branch**
/// (`skill == null`) — the crit-damage stats for one attacker/target pair at a
/// given attack position:
///
/// ```java
/// criticalDamage = getValue(CRITICAL_DAMAGE, 1) * getPositionTypeValue(CRITICAL_DAMAGE, position);
/// defenceCriticalDamage = target.getValue(DEFENCE_CRITICAL_DAMAGE, 1);
/// return 2 * criticalDamage * defenceCriticalDamage * balanceMod;   // balanceMod 1
/// ```
///
/// This is where Death Whisper 1242, Focus Attack 317, Vicious Stance 312,
/// Frenzy 176, Dance of Fire 274 and the rest of the 18 learnable
/// `CriticalDamage` skills finally land — every one was inert before, pumping
/// a stat with no reader anywhere. The position term is
/// `getPositionTypeValue(Stat.CRITICAL_RATE, position)` — Focus Chance 356's
/// per-position crit-*rate* multiplier. Identity 1.0 for anyone without it,
/// which is what `calcCriticalPositionBonus` hard-coded before G34 S4.
pub(crate) fn crit_rate_position_mul(
    world: &World,
    object_id: i32,
    position: movement::Position,
) -> f64 {
    world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&object_id)
        .and_then(|m| {
            m.by_position
                .get(&(crate::model::stats::Stat::CriticalRate, position))
                .copied()
        })
        .unwrap_or(1.0)
}

/// `CriticalDamagePosition` (Focus Death 355, Focus Power 357), read *only*
/// here, matching Java.
pub(crate) fn crit_damage_auto(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    position: movement::Position,
) -> formulas::CritDamage {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;
    let attacker = world.objects.get_component::<StatModifiers>(&attacker_oid);
    let target = world.objects.get_component::<StatModifiers>(&target_oid);
    // `getValue(stat, 1)` / `getValue(stat, 0)`: the mul map defaults to 1.0
    // and the add map to 0.0, so an actor with no `StatModifiers` at all (most
    // NPCs) yields Java's stat-free `2.0` / `0.0` — what the whole port
    // hard-coded before this slice.
    let mul_of =
        |m: Option<&StatModifiers>, s: Stat| m.and_then(|m| m.mul.get(&s).copied()).unwrap_or(1.0);
    let add_of =
        |m: Option<&StatModifiers>, s: Stat| m.and_then(|m| m.add.get(&s).copied()).unwrap_or(0.0);
    let position_mul = attacker
        .map(|m| m.position_value(Stat::CriticalDamage, position))
        .unwrap_or(1.0);
    formulas::CritDamage {
        mul: 2.0
            * mul_of(attacker, Stat::CriticalDamage)
            * position_mul
            * mul_of(target, Stat::DefenceCriticalDamage),
        add: add_of(attacker, Stat::CriticalDamageAdd)
            + add_of(target, Stat::DefenceCriticalDamageAdd),
    }
}

/// `calcCritDamage`'s **skill** branches, which take neither the position term
/// nor any additive one (`PhysicalAttack` and `calcMagicDam` apply only
/// `critMod`).
///
/// The physical half reads `PHYSICAL_SKILL_CRITICAL_DAMAGE`, which **no
/// learnable skill on this dist grants** (40 non-learnable ones do), so it
/// stays the stat-free 2.0 — the established `BLOW_RATE_DEFENCE`/`MP_BLOCK`
/// precedent of not inventing plumbing for a stat nothing reachable sets.
/// The magic half is real: Prophecy of Wind 1357 and Victories of Pa'agrio
/// 1414 grant `MAGIC_CRITICAL_DAMAGE`.
pub(crate) fn crit_damage_skill(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    magic: bool,
) -> f64 {
    use crate::model::stats::Stat;
    let mul_of = |oid: i32, s: Stat| stat_mul(world, oid, s);
    // `Formulas.calcCritDamage`: with a skill involved the *skill* crit stats
    // are read, not `CRITICAL_DAMAGE` — the magic pair for a magic skill, the
    // physical-skill pair otherwise. The physical branch used to be a flat
    // 2.0, i.e. both its stats pinned at identity (G34 S4).
    // `balanceMod` stays 1: its `Config.PV*_*_CRITICAL_DAMAGE_MULTIPLIERS`
    // tables are per-class and default to 1f, and this dist sets none of them.
    let (attack_stat, defence_stat) = if magic {
        (Stat::MagicCriticalDamage, Stat::DefenceMagicCriticalDamage)
    } else {
        (
            Stat::PhysicalSkillCriticalDamage,
            Stat::DefencePhysicalSkillCriticalDamage,
        )
    };
    2.0 * mul_of(attacker_oid, attack_stat) * mul_of(target_oid, defence_stat)
}

/// The `StatByMoveType` contribution to evasion for whoever is being snapshot
/// — Acrobatic Move 225's `+4..6 EVASION_RATE` while `RUNNING`, the only
/// non-regen use of the effect among learnable skills. Truncated to an `i32`
/// like every other evasion term on this port; zero for anyone without the
/// passive or standing still.
fn move_type_evasion_bonus(world: &World, object_id: i32) -> i32 {
    let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&object_id)
    else {
        return 0;
    };
    let move_type = crate::game_loop::regen::move_type_of(world, object_id);
    mods.move_type_value(crate::model::stats::Stat::EvasionRate, move_type) as i32
}

/// A creature's shield block stats: `(shieldDef, shieldRate×CON, conBonus)`.
/// Only players carry an inventory/shield here; NPCs return no shield with a
/// neutral CON bonus.
pub(crate) fn shield_stats(world: &World, object_id: i32) -> (f64, f64, f64) {
    use crate::model::components::{BaseStats, StatModifiers};
    use crate::model::inventory::{Inventory, PaperdollSlot};
    use crate::model::stats::Stat;
    let Some(base) = world.objects.get_component::<BaseStats>(&object_id) else {
        return (0.0, 0.0, 1.0);
    };
    let con_bonus = world.data.stat_bonus.bonus(BaseStat::Con, base.con);
    let shield = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| {
            inv.paperdoll_item(PaperdollSlot::LHand)
                .map(|it| it.item_id)
        })
        .and_then(|id| world.data.item_data.item_stats(id));
    // Java `Formulas.calcShldUse` bails on `!(secondaryWeaponItem instanceof
    // Armor)` *before* ever reading `Stat.SHIELD_DEFENCE`/`_RATE` — so a buff
    // like Residence Shield Defense (+225 DIFF) contributes nothing without an
    // actual shield equipped, matching the early return here.
    let Some(shield) = shield else {
        return (0.0, 0.0, con_bonus);
    };
    let (def, rate) = (
        shield.shield_def.unwrap_or(0) as f64,
        shield.shield_rate.unwrap_or(0) as f64,
    );
    // `ShieldDefenceFinalizer`/`ShieldDefenceRateFinalizer`: `Stat.defaultValue`
    // (`base * mul + add`) over `calcWeaponPlusBaseValue` — the shield's own
    // sDef/rShld *is* that base value (no other item contributes to either
    // stat), so folding the buff mods here reproduces `getShldDef()`/
    // `getValue(SHIELD_DEFENCE_RATE)` exactly. The CON multiply on the rate
    // happens after, in `calcShldUse` itself — not baked into the stat.
    let (def, rate) = match world.objects.get_component::<StatModifiers>(&object_id) {
        Some(mods) => (
            crate::model::finalize(mods, Stat::ShieldDefence, def),
            crate::model::finalize(mods, Stat::ShieldDefenceRate, rate),
        ),
        None => (def, rate),
    };
    (def, rate * con_bonus, con_bonus)
}

/// 2D center-to-center distance between two combat actors.
pub(crate) fn distance_2d(a: &Combatant, b: &Combatant) -> f64 {
    (((b.x - a.x) as f64).powi(2) + ((b.y - a.y) as f64).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Combat stance (`AttackStanceTaskManager`)
// ---------------------------------------------------------------------------

/// Put a player into (or refresh) combat stance — `addAttackStanceTask`.
/// Broadcasts `AutoAttackStart` only on the not-in-stance → in-stance edge.
pub(crate) fn refresh_attack_stance(world: &mut World, player_object_id: i32) {
    let now = world.tick;
    let Some(st) = world
        .objects
        .get_component_mut::<AttackState>(&player_object_id)
    else {
        return;
    };
    let was_in_stance = st.stance_until_tick > now;
    st.stance_until_tick = now + COMBAT_STANCE_TICKS;
    if !was_in_stance {
        broadcast_including_self(
            world,
            player_object_id,
            &server_packets::auto_attack_start(player_object_id),
        );
    }
}

/// The 1 s stance sweep: players whose 15 s ran out leave combat stance
/// (`AutoAttackStop` broadcast).
pub(crate) fn stance_tick(world: &mut World) {
    let now = world.tick;
    let mut expired: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &AttackState)>(|(p, st)| {
            if st.stance_until_tick != 0 && st.stance_until_tick <= now {
                expired.push(p.object_id);
            }
        });
    for object_id in expired {
        if let Some(st) = world.objects.get_component_mut::<AttackState>(&object_id) {
            st.stance_until_tick = 0;
        }
        broadcast_including_self(
            world,
            object_id,
            &server_packets::auto_attack_stop(object_id),
        );
    }
}

/// Whether the actor's right hand holds a two-handed weapon
/// (`SLOT_LR_HAND`) — the flag `Formulas.calculateTimeToHit` needs, since a
/// two-hander lands its blow at a different point in the swing.
///
/// `false` for an empty hand and for anything with no inventory at all, both
/// of which swing barehanded.
pub(crate) fn wields_two_handed(world: &World, attacker_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&attacker_oid)
        .is_some_and(|inv| {
            let rhand = inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand);
            rhand != 0
                && world
                    .data
                    .item_data
                    .get(rhand)
                    .is_some_and(|t| t.body_part == crate::data::item_data::SLOT_LR_HAND)
        })
}

/// Port of `AttackStanceTaskManager.hasAttackStanceTask` — the actor is in
/// combat stance (sword drawn), i.e. within 15 s of its last swing/hit. This is
/// the state `Player.canLogout` uses to refuse a restart/logout while fighting.
pub(crate) fn has_attack_stance(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.stance_until_tick > world.tick)
}

// ---------------------------------------------------------------------------
// AttackRequest + player attack think
// ---------------------------------------------------------------------------
