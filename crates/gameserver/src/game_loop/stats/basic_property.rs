//! Mesmerizing-debuff resistance — Java `BasicPropertyResist` +
//! `Formulas.getAbnormalResist`, G34 S2 (PLAN_G34_SKILL_PARITY.md).
//!
//! # The mechanic
//!
//! Chain-stunning something gets harder. Every debuff that lands and declares a
//! non-`NONE` `<basicProperty>` bumps a per-target counter for that property;
//! the counter multiplies the *next* debuff's landing chance by
//! **1.0 / 0.6 / 0.3 / 0** at level 0 / 1 / 2 / 3+, and decays 15 s after the
//! last one landed. Level 3 is a hard immunity, not a small penalty: the
//! multiplier is applied **after** Java's min/max clamp, so it can drive a rate
//! the clamp had floored at 10 all the way to 0.
//!
//! # Why it was missing, and why the comment that said so was wrong
//!
//! `Formulas.calcEffectSuccess` reads **two different things** off the same
//! `basicProperty` on adjacent lines:
//!
//! ```java
//! final double targetBasicProperty = getAbnormalResist(skill.getBasicProperty(), target);
//! final double baseMod = (((magicLevel - target.getLevel() + 3) * lvlBonusRate) + activateRate + 30.0) - targetBasicProperty;
//! …
//! final double basicPropertyResist = getBasicPropertyResistBonus(skill.getBasicProperty(), target);
//! final double finalRate = traitMod > 0 ? constrain(rate, min, max) * basicPropertyResist : 0;
//! ```
//!
//! `getAbnormalResist` is a **stat** lookup (`ABNORMAL_RESIST_PHYSICAL` /
//! `_MAGICAL`), granted by the `PhysicalAbnormalResist`/`MagicalAbnormalResist`
//! effects — no *learnable* source on this dist, so it is 0 for anything a
//! player can build toward. `getBasicPropertyResistBonus` is the accrual chain,
//! which **nothing grants**: it is earned by *being debuffed*, by everyone.
//!
//! `formulas.rs` used to carry a comment writing **both** off on the strength
//! of the first one — "`BasicPropertyResist` is granted by no skill on this
//! dist, so it can never leave its identity". That is the
//! [[l2r-deviation-comments-self-justify]] shape: a deviation note resting on a
//! half-checked premise, believed for months because it was written down.
//!
//! # Who has it here
//!
//! `Creature.hasBasicPropertyResist()` returns `true` unconditionally;
//! `Player` overrides it to `isInCategory(CategoryType.SIXTH_CLASS_GROUP)`,
//! which on this datapack lists only awakened classes (148+) that Interlude
//! does not have. So on this dist the chain is **live for every NPC, monster,
//! pet and servitor, and off for every player** — retail PvE stun-lock
//! resistance, with PvP chain-CC unaffected. Getting that backwards silently
//! rewrites PvP, so it is asserted in `basic_property_tests`.

use crate::game_loop::helpers::stat_add;
use crate::model::components::stats::BasicPropertyResists;
use crate::model::skill::BasicProperty;
use crate::world::World;

/// Java `BasicPropertyResist.RESIST_DURATION` — 15 s from the last landed
/// debuff, in 100 ms game ticks.
pub(crate) const RESIST_DURATION_TICKS: u64 = 150;

/// Java `Creature.hasBasicPropertyResist()`.
///
/// `Player` overrides it to `isInCategory(SIXTH_CLASS_GROUP)`; that category
/// holds only awakened (148+) classes, so it is always false for a player on
/// this dist. Every other creature inherits `Creature`'s unconditional `true`.
pub(crate) fn has_resist(world: &World, object_id: i32) -> bool {
    match world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    {
        Some(player) => world
            .data
            .categories
            .contains("SIXTH_CLASS_GROUP", player.class_id),
        // NPCs, monsters, pets and servitors: `Creature`'s default.
        None => true,
    }
}

/// Java `Formulas.getBasicPropertyResistBonus` — the multiplier the *next*
/// debuff of this property is scaled by. `1.0` when the property is `NONE`,
/// when the target can't accrue at all, or when the chain has decayed.
pub(crate) fn resist_bonus(world: &World, object_id: i32, property: BasicProperty) -> f64 {
    if property == BasicProperty::None || !has_resist(world, object_id) {
        return 1.0;
    }
    match resist_level(world, object_id, property) {
        0 => 1.0,
        1 => 0.6,
        2 => 0.3,
        // Java's `default:` — 3 and above are a flat immunity.
        _ => 0.0,
    }
}

/// Java `BasicPropertyResist.getResistLevel()` — the stored level, or 0 once
/// the 15 s window has passed. **Expiry is checked on read, never swept**, like
/// Java: a stale entry simply reads as 0 and is reset by the next accrual.
pub(crate) fn resist_level(world: &World, object_id: i32, property: BasicProperty) -> i32 {
    world
        .objects
        .get_component::<BasicPropertyResists>(&object_id)
        .map(|r| r.get(property))
        .filter(|(_, end_tick)| world.tick <= *end_tick)
        .map_or(0, |(level, _)| level)
}

/// Java `BasicPropertyResist.increaseResistLevel()`, called from
/// `Skill.applyEffects` **only when the debuff actually landed** (inside the
/// `addContinuousEffects` branch), so a resisted stun builds no resistance —
/// spamming a debuff that keeps failing never locks you out of it.
///
/// An expired chain restarts at 1 rather than continuing, which is why the
/// window is "15 s since the last landed debuff" and not a fixed budget.
pub(crate) fn increase_resist_level(world: &mut World, object_id: i32, property: BasicProperty) {
    if property == BasicProperty::None || !has_resist(world, object_id) {
        return;
    }
    let expired = resist_level(world, object_id, property) == 0;
    let end_tick = world.tick + RESIST_DURATION_TICKS;
    let mut resists = world
        .objects
        .get_component::<BasicPropertyResists>(&object_id)
        .copied()
        .unwrap_or_default();
    let level = if expired {
        1
    } else {
        resists.get(property).0 + 1
    };
    resists.set(property, level, end_tick);
    world.objects.add_components(&object_id, resists);
}

/// Java `Formulas.getAbnormalResist` — the **stat** half. Subtracted *inside*
/// `baseMod`, so unlike [`resist_bonus`] it is still subject to the 10–90
/// clamp: a huge value floors the rate at 10, it cannot reach 0.
///
/// `ABNORMAL_RESIST_PHYSICAL`/`_MAGICAL` are granted by the
/// `PhysicalAbnormalResist`/`MagicalAbnormalResist` effects, both plain
/// `AbstractStatAddEffect`s and both registered in `EFFECT_REGISTRY` as part of
/// this slice. Neither has a *learnable* source (3 items each), so the term is
/// 0 for anything a player can build toward — but it is wired end to end
/// rather than assumed away, which is exactly the assumption that hid the
/// accrual chain above.
pub(crate) fn abnormal_resist(world: &World, object_id: i32, property: BasicProperty) -> f64 {
    let stat = match property {
        BasicProperty::None => return 0.0,
        BasicProperty::Physical => crate::model::stats::Stat::AbnormalResistPhysical,
        BasicProperty::Magic => crate::model::stats::Stat::AbnormalResistMagical,
    };
    stat_add(world, object_id, stat)
}
