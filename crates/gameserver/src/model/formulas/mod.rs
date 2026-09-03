//! Port of `model/stats/Formulas.java`, scoped to what the single-target cast
//! pipeline needs: magic damage, magic crit, cast timing, heal, and the
//! cast-break-on-hit roll. Every function documents the Java method it ports
//! and which terms are dropped. The dropped terms are identity values for an
//! unarmed, shotless player: `SHOTS_BONUS`/spiritshots (1.0/absent),
//! `SKILL_POWER_ADD` (0), `RANDOM_DAMAGE` (weapon-supplied, unarmed = 0 →
//! randomMod 1.0), pvp/pve config multipliers (1.0 by default),
//! `MAGICAL_SKILL_POWER` (1.0).
//!
//! The **attribute** mod is real since the G19 attributes slice
//! ([`land_rate::calc_attribute_bonus`]) and the **trait** mods since the G20 trait slice
//! (`skills::effects::skill_trait_mod`) — callers multiply both in at Java's
//! spots rather than this module folding them, which is why the signatures
//! stop short of them.

pub mod heal;
pub mod land_rate;
pub mod magic;
pub mod physical;
pub mod progression;
pub mod timing;
