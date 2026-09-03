//! `handlers/effecthandlers/NightStatModify.java` — the night-only stat grant
//! behind Shadow Sense (294), "increases Accuracy by 3 **at night**".
//!
//! Java's `pump` returns early during the day, so the stat is granted or not
//! depending on the clock, and a single global `OnDayNightChange` listener
//! re-pumps every bearer when it flips. This port arrives at the same
//! behaviour from the other end: `stat_modifier_effects` (which has no clock)
//! never emits the grant, and this module rewrites the *landed buff's* stored
//! modifiers whenever the answer changes. The stat hot path stays clock-free.
//!
//! Java keeps its bearers in a static `Set<Creature>` populated by
//! `onStart`/`onExit`. There is no equivalent registry here, so the sweep walks
//! the in-game players and asks each buff list — the same
//! scan-instead-of-subscribe trade the trigger effects make, and for the same
//! reason: at a handful of players per flip it is not worth an index.

use crate::game_loop::helpers::send_sm_to_player;
use crate::model::skill::effects::SkillEffect;
use crate::network::server_packets::{SmParam, sm_ids};
use crate::world::World;

/// `CommonSkill.SHADOW_SENSE` — the one skill whose bearers get the message.
const SHADOW_SENSE: i32 = 294;

/// Re-apply every night-gated stat grant for `object_id` against the current
/// clock, rebuilding the modifier maps if anything moved.
///
/// Returns `true` when a grant was added or removed, which is what decides
/// whether the caller needs to broadcast the new stats.
pub(crate) fn refresh_one(world: &mut World, object_id: i32, night: bool) -> bool {
    // What the buffs *should* contribute right now, keyed by skill id.
    let wanted: Vec<(i32, Vec<crate::model::skill::effects::StatModifierEffect>)> = {
        let Some(buffs) = world
            .objects
            .get_component::<crate::model::components::Buffs>(&object_id)
        else {
            return false;
        };
        buffs
            .0
            .iter()
            .filter_map(|b| {
                let skill = world.data.skill_data.get(b.skill_id, b.skill_level)?;
                let grants: Vec<_> = skill
                    .effects
                    .iter()
                    .filter_map(|e| match e {
                        SkillEffect::NightStatModify { stat, amount, mode } if night => {
                            Some(crate::model::skill::effects::StatModifierEffect {
                                stat: *stat,
                                mode: *mode,
                                amount: *amount,
                                ..Default::default()
                            })
                        }
                        // During the day the grant is simply absent, which is
                        // Java's early `return` from `pump`.
                        SkillEffect::NightStatModify { .. } => None,
                        _ => None,
                    })
                    .collect();
                // Only buffs that *carry* the effect are rewritten; everything
                // else keeps whatever it merged when it landed.
                skill
                    .effects
                    .iter()
                    .any(|e| matches!(e, SkillEffect::NightStatModify { .. }))
                    .then_some((b.skill_id, grants))
            })
            .collect()
    };
    if wanted.is_empty() {
        return false;
    }

    let mut changed = false;
    if let Some(buffs) = world
        .objects
        .get_component_mut::<crate::model::components::Buffs>(&object_id)
    {
        for (skill_id, grants) in wanted {
            for b in buffs.0.iter_mut().filter(|b| b.skill_id == skill_id) {
                if b.effects.len() != grants.len() {
                    b.effects = grants.clone();
                    changed = true;
                }
            }
        }
    }
    if !changed {
        return false;
    }

    // Same rebuild-from-survivors the buff add/remove paths use: the maps
    // cannot be patched in place without drifting under rounding.
    if let Some((player, base, mut mods, inventory, buffs, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &crate::model::Player,
            &crate::model::components::BaseStats,
            &mut crate::model::components::StatModifiers,
            &crate::model::inventory::Inventory,
            &crate::model::components::Buffs,
            &mut crate::model::components::Speeds,
            &mut crate::model::components::CombatStats,
        )>(&object_id)
    {
        mods.add.clear();
        mods.mul.clear();
        mods.by_move_type.clear();
        mods.by_position.clear();
        for b in &buffs.0 {
            for effect in &b.effects {
                crate::model::stat_finalize::apply_modifier(&mut mods, effect);
            }
        }
        player.recalculate_stats(
            &world.data,
            base,
            &mods,
            inventory,
            &mut speeds,
            &mut combat,
        );
    }
    true
}

/// `onDayNightChange` — re-pump every bearer, then message the ones who
/// actually **know** Shadow Sense.
///
/// That second clause is Java's, and it is a real quirk rather than an
/// optimisation: a character carrying the effect from some other source gets
/// the stat and no message at all.
pub(crate) fn on_day_night_change(world: &mut World, night: bool) {
    let players: Vec<i32> = world.in_game_player_oids().collect();
    let sm = if night {
        sm_ids::IT_IS_NOW_MIDNIGHT_AND_THE_EFFECT_OF_S1_CAN_BE_FELT
    } else {
        sm_ids::IT_IS_DAWN_AND_THE_EFFECT_OF_S1_WILL_NOW_DISAPPEAR
    };
    for oid in players {
        if !refresh_one(world, oid, night) {
            continue;
        }
        let knows = world
            .objects
            .get_component::<crate::model::components::SkillBook>(&oid)
            .is_some_and(|b| b.0.contains_key(&SHADOW_SENSE));
        if !knows {
            continue;
        }
        send_sm_to_player(
            world,
            oid,
            sm,
            &[SmParam::SkillName {
                id: SHADOW_SENSE,
                level: 1,
            }],
        );
    }
}
