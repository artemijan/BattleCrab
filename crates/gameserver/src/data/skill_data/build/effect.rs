//! Turning one parsed `<effect>` into the `SkillEffect`s it produces.
//!
//! The name match is split across the family modules below, each returning
//! `None` for a name it does not own, so an effect falls through to the
//! `EFFECT_REGISTRY` stat lookup and then to the gap record — the same order
//! the single match had.

use std::cell::RefCell;

use super::super::{EFFECT_REGISTRY, LeveledValues, ParsedEffect, SkillGaps, value_at};
use crate::model::skill;
use crate::model::stats::{Stat, StatModifierType};

/// Everything an effect arm reads: the effect's own `<params>` plus the two
/// values derived from them before the match (`modifier_mode`, `hp_percent`)
/// and the skill-level `values` the `Restoration` arm needs.
pub(super) struct Cx<'a> {
    pub xml_name: &'a String,
    pub params: &'a LeveledValues,
    pub mode: &'a String,
    pub groups: &'a Vec<skill::effects::RestorationGroup>,
    pub armor_condition: &'a u8,
    pub weapon_condition: &'a u32,
    pub values: &'a LeveledValues,
    pub level: i32,
    pub modifier_mode: StatModifierType,
    pub hp_percent: i32,
}

impl Cx<'_> {
    pub(super) fn param(&self, key: &str) -> Option<f64> {
        value_at(self.params, key, self.level).and_then(|v| v.parse().ok())
    }

    pub(super) fn stat_mod(&self, stat: Stat, amount: f64) -> skill::effects::SkillEffect {
        skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
            stat,
            mode: self.modifier_mode,
            amount,
            armor_condition: *self.armor_condition,
            weapon_condition: *self.weapon_condition,
            qualifier: None,
            two_handed: false,
            hp_percent: self.hp_percent,
        })
    }
}

/// One `<effect>` → the effects it contributes at `level`.
pub(super) fn of(
    e: &ParsedEffect,
    values: &LeveledValues,
    level: i32,
    id: i32,
    gaps: &RefCell<SkillGaps>,
) -> Vec<skill::effects::SkillEffect> {
    let (xml_name, params, mode, groups, armor_condition, weapon_condition) = (
        &e.name,
        &e.params,
        &e.mode,
        &e.groups,
        &e.armor_condition,
        &e.weapon_condition,
    );
    let modifier_mode = if mode == "PER" {
        StatModifierType::Per
    } else {
        StatModifierType::Diff
    };
    // `AbstractConditionalHpEffect`'s `<hpPercent>`: the four handlers that
    // extend it (`PAtk`, `PhysicalDefence`, `PhysicalEvasion`, `CriticalRate`)
    // are otherwise ordinary stat effects, and every one of them reaches this
    // function through `EFFECT_REGISTRY` — so reading the parameter here covers
    // the family without a per-name arm. Absent → 0 → unconditional, which is
    // Java's `_hpPercent <= 0` case.
    let cx = Cx {
        xml_name,
        params,
        mode,
        groups,
        armor_condition,
        weapon_condition,
        values,
        level,
        modifier_mode,
        hp_percent: 0,
    };
    let hp_percent = cx.param("hpPercent").unwrap_or(0.0) as i32;
    let cx = Cx { hp_percent, ..cx };
    let param = |key: &str| cx.param(key);
    let stat_mod = |stat: Stat, amount: f64| cx.stat_mod(stat, amount);

    if let Some(v) = super::aggro::build(&cx) {
        return v;
    }
    if let Some(v) = super::control::build(&cx) {
        return v;
    }
    if let Some(v) = super::damage::build(&cx) {
        return v;
    }
    if let Some(v) = super::dispel::build(&cx) {
        return v;
    }
    if let Some(v) = super::gathering::build(&cx) {
        return v;
    }
    if let Some(v) = super::stats::build(&cx) {
        return v;
    }
    if let Some(v) = super::summoning::build(&cx) {
        return v;
    }
    if let Some(v) = super::support::build(&cx) {
        return v;
    }
    if let Some(v) = super::ticks::build(&cx) {
        return v;
    }
    if let Some(v) = super::traits::build(&cx) {
        return v;
    }
    if let Some(v) = super::triggers::build(&cx) {
        return v;
    }
    if let Some(v) = super::utility::build(&cx) {
        return v;
    }

    match EFFECT_REGISTRY
        .iter()
        .find(|(n, _)| n == xml_name)
        .map(|(_, s)| *s)
    {
        Some(stat) => param("amount")
            .map(|amount| stat_mod(stat, amount))
            .into_iter()
            .collect(),
        // Nothing recognised this name: the effect is
        // dropped and, if it was the skill's only one, so
        // is the whole buff. Recorded, not silent.
        None => {
            SkillGaps::record(&mut gaps.borrow_mut().effects, xml_name, id);
            Vec::new()
        }
    }
}
