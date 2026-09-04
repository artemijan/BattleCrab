//! What a creature knows and what is currently on it: the skill book, its
//! enchant/clan/option overlays, hennas, reuses and active buffs.

use bevy_ecs::component::Component;
use std::collections::HashMap;

/// Known skills (skill_id → level), loaded from `character_skills` (or the
/// class's autoGet initial set at creation). Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct SkillBook(pub HashMap<i32, i32>);

/// The enchant sub-level per skill id (0/absent = unenchanted) — Java keeps
/// this on the `Skill` instance itself (`getSubLevel()`); the port's book is
/// (id → level), so the routes live in a parallel map. Persisted in the same
/// `character_skills` rows (`skill_sub_level`), banked per class index on a
/// subclass switch like the book. PLAN_G19_SKILL_ENCHANT.md.
#[derive(Component, Debug, Clone, Default)]
pub struct SkillEnchants(pub HashMap<i32, i32>);

/// Clan skills currently granted to this member (skill_id → level), Java's
/// `Player.addSkill(clanSkill, false)` set. **Transient** — re-derived from the
/// clan on every login (see `game_loop::clans::apply_clan_skills`) and never
/// written to `character_skills` (Java passes `store=false`). Kept separate from
/// [`SkillBook`] both to preserve that no-persist contract and so leaving/
/// dispersing the clan strips exactly these. Folded into the `SkillList` packet
/// alongside the skill book. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct ClanSkills(pub HashMap<i32, i32>);

/// Active skills granted by the **augment options** on currently-equipped
/// items (skill_id → level), Java's `Options.apply` → `addSkill(skill, false)`
/// set.
///
/// **Transient**, for the same reason [`ClanSkills`] is: Java's `store = false`
/// never reaches `character_skills`, and this port persists the whole
/// [`SkillBook`] — so an option skill kept there would survive the item being
/// unequipped and re-arm on every login with nothing equipped to explain it.
/// Re-derived from the equipped items on each equip/unequip. Folded into the
/// `SkillList` packet and into the cast path's known-skill lookup, which is what
/// makes an augment active actually castable. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct OptionSkills(pub HashMap<i32, i32>);

/// Augment **activation** skills — Java `Creature._triggerSkills`, a map keyed
/// by the triggered skill's id (so re-adding the same skill replaces rather
/// than stacks, and one option's removal takes exactly one entry).
///
/// Populated by `Options.apply`/`remove` from `<attack_skill>`,
/// `<magic_skill>` and `<critical_skill>`; read on every landed auto-attack and
/// every finished cast. Transient like [`OptionSkills`]. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct OptionTriggers(pub HashMap<i32, crate::data::option_data::OptionTrigger>);

/// The player's three worn henna dyes (Java `Player._henna[3]`), by slot →
/// dye id. Loaded from `character_hennas`, persisted in the store transaction.
/// The dyes' base-stat bonuses are folded into [`super::stats::BaseStats`] (recomputed on
/// add/remove); this component holds only the slot assignments. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct HennaSlots(pub [Option<i32>; 3]);

impl HennaSlots {
    /// Number of filled slots (Java `3 - getHennaEmptySlots()` counts these).
    pub fn worn(&self) -> usize {
        self.0.iter().filter(|s| s.is_some()).count()
    }

    /// The worn dye ids in slot order.
    pub fn dye_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.0.iter().filter_map(|s| *s)
    }
}

/// Live cooldowns (Java `_reuseTimeStampsSkills` + `_disabledSkills`,
/// unified), keyed by `Skill::reuse_key()`. Checked lazily — no expiry tasks.
/// Persisted across relog via `character_skills_save` (Java `storeEffect`/
/// `restoreEffects`, reuse half; gated by `StoreSkillCooltime`) — see
/// `db::SkillReuseRow` + `PlayerData::restore_reuses`. Buff restore (the
/// `restore_type = 0` half) is still deferred.
#[derive(Component, Debug, Clone, Default)]
pub struct Reuses(pub HashMap<i32, crate::model::SkillReuse>);

/// Active buffs/debuffs (Java `EffectList`). Expiry is driven by the
/// `Scheduler` (`ScheduledTask::BuffExpire`), not by anything here.
#[derive(Component, Debug, Clone, Default)]
pub struct Buffs(pub Vec<crate::model::skill::active_buff::ActiveBuff>);
