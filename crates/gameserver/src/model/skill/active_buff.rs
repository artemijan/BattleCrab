//! Java `BuffInfo` — one landed buff/debuff on a creature.

use super::BuffSlot;
use super::effects::StatModifierEffect;

/// A landed buff/debuff on a `Player` (Java `BuffInfo`, trimmed to what G6
/// needs: which stats it's modifying and when it wears off — the "when" is
/// tracked by the `Scheduler`, not stored here).
#[derive(Debug, Clone)]
pub struct ActiveBuff {
    pub skill_id: i32,
    pub skill_level: i32,
    pub abnormal_type_client_id: i32,
    /// Java `Skill.getAbnormalType()` ("NONE" when unset) — the stacking key:
    /// effects of the same abnormal type don't stack (`EffectList.addActive`).
    pub abnormal_type: String,
    /// Java `Skill.getAbnormalLevel()` — decides which of two same-type buffs
    /// wins (the higher level overrides; a lower one is refused).
    pub abnormal_level: i32,
    /// Which slot pool this occupies for the count caps (`MaxBuffAmount` /
    /// `MaxDanceAmount`); debuffs/toggles/passives are `Uncapped`.
    pub slot: BuffSlot,
    /// Absolute tick the buff expires at (for `AbnormalStatusUpdate`'s
    /// remaining-time field).
    pub expires_at_tick: u64,
    /// Java `BuffInfo.isDisplayedForEffected()`:
    /// `!isSelfContinuous() || (effected == effector) || !hasEffects(SELF)`.
    ///
    /// An `A3` skill that also carries `<selfEffects>` hides its row from
    /// anyone who is not the caster — Blinding Blow 321, Vengeance 368, Evade
    /// Shot 369, Critical Blow 409, Aura Flare 1231 and Hurricane Shackle 1996
    /// on this dist. The victim feels the debuff but is never shown an icon
    /// for it. Stamped at creation because the effector is not stored.
    pub displayed: bool,
    /// True for entries that stand in for a passive skill's stat pump (the
    /// grade-penalty skills 6209/6213) rather than a timed buff. They drive
    /// stats through the same modifier maps but are hidden from
    /// `AbnormalStatusUpdate` (Java passive skills never show an abnormal icon)
    /// and never get a `BuffExpire` schedule.
    pub passive: bool,
    /// OR of the [`super::effect_flag`] bits this buff's skill contributes (0 for the
    /// overwhelming majority). Stamped at creation so the creature's live mask
    /// is a fold over its buff list — see [`super::effect_flag`].
    pub effect_flags: u32,
    /// Client ids of the visual effects this buff shows while up. Stamped at
    /// creation and folded over the buff list when a packet needs the creature's
    /// current look — same pattern as `effect_flags`.
    pub abnormal_visuals: Vec<i16>,
    /// Abnormal types this buff *blocks* from landing while it is up
    /// (`BlockAbnormalSlot`). Empty for almost every buff; stamped at creation
    /// and folded on read, the same way `effect_flags` is.
    pub blocked_abnormals: Vec<String>,
    pub effects: Vec<StatModifierEffect>,
}

impl ActiveBuff {
    /// A synthetic **passive stat pump**: displayed, but with no abnormal state
    /// of its own, `Uncapped`, and never scheduled to expire.
    ///
    /// The shape the grade-penalty, weight-penalty, clan-skill and
    /// passive-skill folds all want — they are stat contributions wearing a
    /// buff's clothes so that `remove_buff` can take them off again, not
    /// abnormals the client should stack or display an icon for.
    ///
    /// Augment options build a *similar* buff by hand in `game_loop::stats::options`
    /// with `expires_at_tick: 0` and an empty abnormal type; that difference is
    /// untested, so it is deliberately not folded in here.
    pub fn passive_pump(skill_id: i32, skill_level: i32, effects: Vec<StatModifierEffect>) -> Self {
        Self {
            displayed: true,
            skill_id,
            skill_level,
            abnormal_type_client_id: -1,
            abnormal_type: "NONE".to_string(),
            abnormal_level: 0,
            slot: BuffSlot::Uncapped,
            expires_at_tick: u64::MAX,
            passive: true,
            effect_flags: 0,
            blocked_abnormals: Vec::new(),
            abnormal_visuals: Vec::new(),
            effects,
        }
    }
}
