//! The category/slot dispel family, extracted from the `apply_skill_effects`
//! match. The fixed-list variants (`DispelBySlot`/`DispelBySlotProbability`)
//! live in `skills::instant` with the rest of the delegated one-liners.

use super::buffs_snapshot;
use super::handle_buff_expire;
use crate::game_loop::helpers::is_dead;
use crate::model::components::StatModifiers;
use crate::model::skill::BuffSlot;
use crate::model::skill::effects::DispelSlot;
use crate::model::skill::Skill;
use crate::world::World;
/// `DispelBySlotMyself.instant` — same shape as `DispelBySlot` with two
/// differences that both matter: the list carries **no levels** (every level
/// of a listed abnormal goes), and an **`irreplacableBuff` is spared**, which
/// is what stops Flames of Invincibility from stripping the clan/transform
/// buffs that `isStayAfterDeath()` also protects.
pub(crate) fn dispel_by_slot_myself(world: &mut World, target_oid: i32, dispel: &[String]) {
    let candidates: Vec<(i32, i32)> =
        buffs_snapshot(world, target_oid, |b| Some((b.skill_id, b.skill_level)));
    let to_dispel: Vec<i32> = candidates
        .into_iter()
        .filter(|&(sid, slvl)| {
            world.data.skill_data.get(sid, slvl).is_some_and(|bs| {
                // `!info.getSkill().isIrreplacableBuff()` — the port folds
                // that tag into `stay_after_death` (G34 S3), which is the
                // same predicate Java's getter uses.
                !bs.stay_after_death && dispel.contains(&bs.abnormal_type)
            })
        })
        .map(|(sid, _)| sid)
        .collect();
    for skill_id in to_dispel {
        handle_buff_expire(world, target_oid, skill_id);
    }
}

/// `DispelAll.instant` — `effected.stopAllEffects()`, i.e.
/// `stopEffects(b -> !b.getSkill().isIrreplacableBuff())`: no abnormal-type
/// list and no level ranking, just "everything that is not irreplacable".
/// Skill 4177 Cancellation, the raid-boss sweep.
///
/// The predicate is `stay_after_death` for the same reason
/// `dispel_by_slot_myself` uses it: this port folds `<irreplacableBuff>` into
/// that field (G34 S3) and cannot tell the three source tags apart. It spares
/// marginally more than Java here — a skill with only `<stayAfterDeath>`
/// survives a cancel it would lose upstream — which is the conservative
/// direction for a buff-stripping effect, and consistent with the fn above.
pub(crate) fn dispel_all(world: &mut World, target_oid: i32) {
    let candidates: Vec<(i32, i32, bool)> = buffs_snapshot(world, target_oid, |b| {
        Some((b.skill_id, b.skill_level, b.passive))
    });
    let to_dispel: Vec<i32> = candidates
        .into_iter()
        // Passives never sit in Java's `_actives` list at all.
        .filter(|&(_, _, passive)| !passive)
        .filter(|&(sid, slvl, _)| {
            world
                .data
                .skill_data
                .get(sid, slvl)
                .is_some_and(|bs| !bs.stay_after_death)
        })
        .map(|(sid, _, _)| sid)
        .collect();
    for skill_id in to_dispel {
        handle_buff_expire(world, target_oid, skill_id);
    }
}

/// `DispelByCategory.instant` — the "Cancel" family (Cancellation, Cleanse,
/// Purification Field, Touch of Death): unlike the fixed-list dispels this
/// steals *whatever* is up. `BUFF` walks dances then buffs in reverse cast
/// order (Java's `getDances()`/`getBuffs()` reversed); `DEBUFF` walks
/// debuffs. Both stop once `max` buffs are collected. `ALL` is dead in Java
/// too (no shipped skill uses it) and is a no-op here.
pub(crate) fn dispel_by_category(
    world: &mut World,
    target_oid: i32,
    skill: &Skill,
    slot: &DispelSlot,
    rate: i32,
    max: i32,
) {
    if is_dead(world, target_oid) {
        return;
    }
    let mut candidates: Vec<(i32, i32, BuffSlot)> = buffs_snapshot(world, target_oid, |b| {
        Some((b.skill_id, b.skill_level, b.slot))
    });
    // Reverse cast order — Java's `getDances()`/`getBuffs()` reversed.
    candidates.reverse();
    let mut to_dispel: Vec<i32> = Vec::new();
    match slot {
        DispelSlot::Buff => {
            // `Formulas.calcCancelSuccess`'s only consumer of
            // `Stat.RESIST_DISPEL_BUFF` — pumped by `ResistDispelByCategory`
            // since an earlier slice but unread until now.
            let resist = world
                .objects
                .get_component::<StatModifiers>(&target_oid)
                .map(|m| {
                    crate::model::finalize(m, crate::model::stats::Stat::ResistDispelBuff, 1.0)
                })
                .unwrap_or(1.0);
            for want in [BuffSlot::Dance, BuffSlot::Buff] {
                for &(sid, slvl, _) in candidates.iter().filter(|&&(_, _, s)| s == want) {
                    if to_dispel.len() >= max as usize {
                        break;
                    }
                    let Some(bs) = world.data.skill_data.get(sid, slvl) else {
                        continue;
                    };
                    // `canBeStolen()`: passive/toggle/debuff are already
                    // excluded by the `Dance`/`Buff` slot filter above.
                    // `isIrreplacableBuff()`/hero/GM/static-skill exclusions
                    // aren't modeled.
                    if !bs.can_be_dispelled {
                        continue;
                    }
                    let hit = rate >= 100 || {
                        let chance = rate as f64
                            + ((skill.magic_level - bs.magic_level) as f64 * 2.0)
                            + ((bs.abnormal_time / 120) as f64 * resist);
                        world.roll(100) < (chance as i32).clamp(25, 75)
                    };
                    if hit {
                        to_dispel.push(sid);
                    }
                }
            }
        }
        DispelSlot::Debuff => {
            for &(sid, slvl, _) in &candidates {
                if to_dispel.len() >= max as usize {
                    break;
                }
                let Some(bs) = world.data.skill_data.get(sid, slvl) else {
                    continue;
                };
                if !bs.is_debuff || !bs.can_be_dispelled {
                    continue;
                }
                if world.roll(100) <= rate {
                    to_dispel.push(sid);
                }
            }
        }
        DispelSlot::All => {}
    }
    for skill_id in to_dispel {
        handle_buff_expire(world, target_oid, skill_id);
    }
}
