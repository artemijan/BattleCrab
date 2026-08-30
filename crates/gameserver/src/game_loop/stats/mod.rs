//! Derived numbers — every module that recomputes what a creature *is* after
//! its equipment, buffs, augments or surroundings change.
//!
//! [`context`] resolves the component borrow the recalculation needs; the rest
//! are the individual contributors: armor-conditioned passives, augment option
//! bonuses, the night-only grant, mesmerizing-debuff resistance, carried weight
//! and the HP/MP/CP regeneration tick.

pub(in crate::game_loop) mod basic_property;
pub(crate) mod context;
pub(crate) mod night_stats;
pub(in crate::game_loop) mod options;
pub(in crate::game_loop) mod passive_skills;
pub(crate) mod regen;
pub(crate) mod weight;
