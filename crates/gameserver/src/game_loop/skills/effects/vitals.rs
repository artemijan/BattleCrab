//! HP/MP/CP restore effects and the party HP rebalance — the vitals half of
//! the instant-effect match.

use super::broadcast_vitals;
use super::caster_display_name;
use super::max_recoverable;
use super::player_or_npc_level;
use crate::game_loop::helpers;
use crate::game_loop::space::position::maybe_position;
use crate::model::components::stats::StatModifiers;
use crate::model::components::stats::Vitals;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::world::World;

/// `effected.getStat().getValue(Stat.MANA_CHARGE, amount)` — the recipient's
/// recharge bonus. Java's two-arg `getValue` is `mul * baseValue + add`, so
/// Higher Mana Gain 285 (`mode=DIFF`, +22..81 by level) is a flat addition.
///
/// The order was already right here; routing it through
/// [`crate::model::stat_finalize::finalize`] (which *is* `Stat.defaultValue`) rather than
/// respelling it also picks up the `//setparam` fixed-value short-circuit that
/// `getValue` checks first and this function did not.
pub(crate) fn mana_charge_of(world: &World, target_oid: i32, amount: f64) -> f64 {
    use crate::model::stats::Stat;
    let Some(mods) = world.objects.get_component::<StatModifiers>(&target_oid) else {
        return amount;
    };
    crate::model::stat_finalize::finalize(mods, Stat::ManaCharge, amount)
}

/// `ManaHealByLevel`'s recharge penalty: a target more than 5 levels above the
/// skill's `magicLevel` gets progressively less, and 15+ levels above gets
/// **nothing at all**.
///
/// Java writes it as an `if/else if` ladder from `levelDiff == 6` (×0.9) down
/// to `== 14` (×0.1) with `>= 15` → 0; that is exactly `1 - (diff - 5)/10`
/// over the ladder's range, so it collapses to arithmetic here rather than
/// nine branches. A gap of 5 or less is unpenalised.
pub(crate) fn recharge_level_penalty(target_level: i32, skill_magic_level: i32) -> f64 {
    let diff = target_level - skill_magic_level;
    if diff <= 5 {
        return 1.0;
    }
    if diff >= 15 {
        return 0.0;
    }
    1.0 - ((diff - 5) as f64 / 10.0)
}

/// The tail every MP-restore handler shares: the dead / `isMpBlocked` gate, the
/// overheal clamp, the write, `broadcastStatusUpdate`, and the self-vs-other
/// system message.
///
/// Java clamps against `getMaxRecoverableMp()` (`MAX_RECOVERABLE_MP` over
/// `maxMp`). Two skills declare `LimitMp` — Seal of Limit (1509) and Mass
/// Restriction (11603) — but **neither is reachable**: 1509 appears on no
/// skill tree, NPC or item, and 11603 is post-Interlude. So the stat is
/// identity and the ceiling is plain `maxMp` here.
pub(crate) fn restore_mp(world: &mut World, caster_oid: i32, target_oid: i32, amount: f64) {
    use server_packets::{SmParam, sm_ids};
    // `effected.isDead() || effected.isDoor() || effected.isMpBlocked()`.
    if helpers::is_dead(world, target_oid) {
        return;
    }
    if crate::game_loop::abnormal::is_mp_blocked(world, target_oid) {
        return;
    }
    // "Prevents overheal and negative amount".
    let restored = {
        let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
            return;
        };
        let headroom = (v.max_mp as f64 - v.cur_mp).max(0.0);
        let restored = amount.min(headroom).max(0.0);
        if restored != 0.0 {
            v.cur_mp += restored;
        }
        restored
    };
    if restored != 0.0 {
        broadcast_vitals(world, target_oid);
    }
    // Java sends the message even when the amount rounded to nothing.
    if let Some(cid) = helpers::client_for_player(world, target_oid) {
        let pkt = if caster_oid != target_oid {
            server_packets::system_message_with(
                sm_ids::S2_MP_HAS_BEEN_RESTORED_BY_C1,
                &[
                    SmParam::Text(caster_display_name(world, caster_oid)),
                    SmParam::Int(restored as i32),
                ],
            )
        } else {
            server_packets::system_message_with(
                sm_ids::S1_MP_HAS_BEEN_RESTORED,
                &[SmParam::Int(restored as i32)],
            )
        };
        helpers::send_to_client(world, cid, pkt);
    }
}

/// `RebalanceHP.instant` — Balance Life (1043).
///
/// Two passes over the same set: sum `maxHp` and `curHp` across every living
/// party member in `affect_range` (plus their pet and servitors), then set each
/// of them to `maxHp * (sumCur / sumMax)`. Java bails outright when the caster
/// is not a player, and does nothing at all when there is no party — an
/// unpartied cast is wasted, which is *not* the "party of one" reading every
/// other party-scoped effect uses.
///
/// The heal direction matters: only a member whose HP goes **up** is clamped by
/// `MAX_RECOVERABLE_HP` (and a member already above that ceiling keeps what
/// they have rather than being pulled down to it). A member who loses HP is
/// written unconditionally — the ceiling guards heals, not the redistribution.
pub(crate) fn rebalance_party_hp(world: &mut World, caster_oid: i32, skill: &Skill) {
    if !world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    let Some(members) = crate::game_loop::party::party_members(world, caster_oid) else {
        // No party: Java's `if (party != null)` guard skips the whole effect.
        return;
    };
    let Some(origin) = maybe_position(world, caster_oid) else {
        return;
    };
    let range = skill.affect_range;
    let in_range = |world: &World, oid: i32| -> bool {
        // `Util.checkIfInRange(range, effector, target, true)` — 3D, and a
        // range of 0 means "no distance filter" the same way the affect
        // helpers read it.
        if range <= 0 {
            return true;
        }
        crate::geo::distance::within_3d_xyz(world, oid, origin.x, origin.y, origin.z, range as f64)
    };

    // Every creature the effect touches: each member, then their pet and
    // servitor. Java walks all three lists twice; collecting once keeps the
    // two passes over exactly the same set.
    let mut touched: Vec<i32> = Vec::new();
    for member in &members {
        for oid in std::iter::once(*member)
            .chain(crate::game_loop::servitor::pet_of(world, *member))
            .chain(crate::game_loop::servitor::servitor_of(world, *member))
        {
            let alive = world
                .objects
                .get_component::<Vitals>(&oid)
                .is_some_and(|v| !v.dead);
            if alive && in_range(world, oid) {
                touched.push(oid);
            }
        }
    }

    let (mut full_hp, mut current_hp) = (0.0f64, 0.0f64);
    for &oid in &touched {
        if let Some(v) = world.objects.get_component::<Vitals>(&oid) {
            full_hp += v.max_hp as f64;
            current_hp += v.cur_hp;
        }
    }
    if full_hp <= 0.0 {
        return;
    }
    let percent = current_hp / full_hp;

    for &oid in &touched {
        let Some(v) = world.objects.get_component::<Vitals>(&oid).copied() else {
            continue;
        };
        let mut new_hp = v.max_hp as f64 * percent;
        if new_hp > v.cur_hp {
            let ceiling = max_recoverable(
                world,
                oid,
                crate::model::stats::Stat::MaxRecoverableHp,
                v.max_hp as f64,
            );
            if v.cur_hp > ceiling {
                new_hp = v.cur_hp;
            } else if new_hp > ceiling {
                new_hp = ceiling;
            }
        }
        if let Some(vit) = world.objects.get_component_mut::<Vitals>(&oid) {
            vit.cur_hp = new_hp.clamp(0.0, vit.max_hp as f64);
        }
        broadcast_vitals(world, oid);
    }
}

/// Which of the MP-restore family computed the amount — four Java handlers,
/// four amount formulas, one shared apply path (`restore_mp`).
pub(crate) enum ManaHealKind {
    Flat,
    ByLevel,
    Percent,
}

/// The `ManaHeal`/`ManaHealByLevel`/`ManaHealPercent` amount formulas.
pub(crate) fn mana_heal(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    power: f64,
    kind: ManaHealKind,
) {
    let max_mp = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.max_mp as f64)
        .unwrap_or(0.0);
    let amount = match kind {
        // `ManaHealPercent`: a straight share of the pool. Java special-cases
        // `power == 100` to the full pool, which is the same number the
        // multiply gives — kept as one branch.
        ManaHealKind::Percent => (max_mp * power) / 100.0,
        // `ManaHeal`: flat power, then the recipient's `MANA_CHARGE`. Java
        // skips that for a *static* skill; no skill in this family is static,
        // so it always applies.
        ManaHealKind::Flat => mana_charge_of(world, target_oid, power),
        // `ManaHealByLevel`: `MANA_CHARGE` first, *then* the level-gap
        // penalty.
        ManaHealKind::ByLevel => {
            let charged = mana_charge_of(world, target_oid, power);
            charged
                * recharge_level_penalty(player_or_npc_level(world, target_oid), skill.magic_level)
        }
    };
    restore_mp(world, caster_oid, target_oid, amount);
}

/// Java's `Mp` handler: `amount`, flat or as a share of max MP.
pub(crate) fn mp_restore(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    amount: f64,
    percent: bool,
) {
    let max_mp = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.max_mp as f64)
        .unwrap_or(0.0);
    let amount = if percent {
        (max_mp * amount) / 100.0
    } else {
        amount
    };
    restore_mp(world, caster_oid, target_oid, amount);
}

/// `HpByLevel.instant` — heals the **effector**. Life Scavenge (46) and
/// Corpse Life Drain (1151) drain a corpse to top the *caster* up, so the
/// target is only the corpse being consumed.
pub(crate) fn hp_by_level(world: &mut World, caster_oid: i32, power: f64) {
    use server_packets::{SmParam, sm_ids};
    let Some(v) = world.objects.get_component::<Vitals>(&caster_oid).copied() else {
        return;
    };
    // Java clamps to `getMaxHp()` here, **not** to `getMaxRecoverableHp()` —
    // the one heal in this family that ignores the recoverable cap. Ported as
    // written.
    let restored = ((v.cur_hp + power).min(v.max_hp as f64) - v.cur_hp).trunc();
    if restored <= 0.0 {
        return;
    }
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&caster_oid) {
        v.cur_hp += restored;
    }
    crate::game_loop::stats::passive_skills::refresh_on_hp_change(world, caster_oid);
    helpers::send_sm_to_player(
        world,
        caster_oid,
        sm_ids::S1_HP_HAS_BEEN_RESTORED,
        &[SmParam::Int(restored as i32)],
    );
    broadcast_vitals(world, caster_oid);
}

/// `Cp.instant` — an immediate CP change:
///
/// ```java
/// if (effected.isDead() || effected.isDoor() || effected.isHpBlocked()) return;
/// …
/// case DIFF: amount = Math.min(basicAmount, Math.max(0, effected.getMaxRecoverableCp() - effected.getCurrentCp())); break;
/// case PER:  amount = Math.min((effected.getMaxCp() * basicAmount) / 100, Math.max(0, effected.getMaxRecoverableCp() - effected.getCurrentCp())); break;
/// ```
///
/// Two details this shares with its neighbour [`super::instant::cp_heal_percent`],
/// and used not to. The headroom is **`getMaxRecoverableCp()`**, not `getMaxCp()`
/// — `LimitCp`'s learnable carriers, Noblesse Harmony (1326) and Noblesse
/// Symphony (1327), cap it at 60 %, so under either aura a CP potion has to stop
/// where the aura says. And the three-way bail is Java's, `isHpBlocked` included
/// (not a typo on Java's part: the *CP* effect reads the *HP* block).
///
/// Note the `PER` arm reads plain `getMaxCp()` for the *size* of the gain and
/// the recoverable ceiling only for the *headroom* — the two are deliberately
/// different reads, so a capped target still computes its percentage off the
/// full pool.
pub(crate) fn cp(world: &mut World, target_oid: i32, amount: f64, percent: bool) {
    if crate::game_loop::helpers::is_dead(world, target_oid)
        || world
            .objects
            .has_component::<crate::model::door::Door>(&target_oid)
        || crate::game_loop::abnormal::is_hp_blocked(world, target_oid)
    {
        return;
    }
    let Some(pv) = world
        .objects
        .get_component::<crate::model::components::stats::PlayerVitals>(&target_oid)
        .copied()
    else {
        return; // NPCs have no CP pool
    };
    let basic = if percent {
        pv.max_cp as f64 * amount / 100.0
    } else {
        amount
    };
    let ceiling = super::max_recoverable(
        world,
        target_oid,
        crate::model::stats::Stat::MaxRecoverableCp,
        pv.max_cp as f64,
    );
    let headroom = (ceiling - pv.cur_cp).max(0.0);
    let delta = if basic >= 0.0 {
        basic.min(headroom)
    } else {
        basic
    };
    if delta != 0.0 {
        if let Some(v) = world
            .objects
            .get_component_mut::<crate::model::components::stats::PlayerVitals>(&target_oid)
        {
            v.cur_cp = (v.cur_cp + delta).clamp(0.0, v.max_cp as f64);
        }
        broadcast_vitals(world, target_oid);
    }
}
