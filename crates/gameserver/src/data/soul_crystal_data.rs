//! The Interlude Soul Crystal tables (`LevelUpCrystalData.xml`), read by quest
//! 350 (Enhance Your Weapon). They answer two questions:
//!   * given a Soul Crystal item, what level is it and what does it level *into*?
//!   * given a monster, which crystal levels can it raise, how, and at what odds?
//!
//! TODO(salvation): Salvation ships no `LevelUpCrystalData.xml` and no Soul
//! Crystal leveling — Q350 is gone, and its crystals are the ensoul system
//! (`EnsoulData`, `stats/ensoul/*.xml`). The loader is removed, so the tables
//! are always empty and Q350 registers no monsters; the type stays only until
//! Q350 and its tests are deleted with the other Interlude-only content (S9).

use std::collections::HashMap;

/// How a monster distributes a crystal level-up across a party (Java
/// `AbsorbCrystalType`). Solo play collapses every variant to "the killer".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AbsorbType {
    /// Only whoever struck the killing blow.
    #[default]
    LastHit,
    /// Every party member holding a crystal.
    FullParty,
    /// One randomly-chosen party member (retail's naive pick — may miss).
    PartyOneRandom,
    /// A cascading 33%-per-member roll over the party.
    PartyRandom,
}

/// One monster's leveling rule for a given crystal level.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct LevelingInfo {
    pub absorb_type: AbsorbType,
    /// Whether the Soul Crystal skill (2096) must have been cast on the mob
    /// (below half HP) before the kill.
    pub skill_needed: bool,
    /// Percent chance (`Rnd.get(100) <= chance`) that the crystal levels.
    pub chance: i32,
}

/// A Soul Crystal item: its stage and the item it becomes when it levels.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SoulCrystal {
    pub level: i32,
    pub leveled_item_id: i32,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SoulCrystalData {
    /// item id → the crystal's stage + next-stage item id.
    crystals: HashMap<i32, SoulCrystal>,
    /// npc id → (crystal level → how that level absorbs from this mob).
    npc_info: HashMap<i32, HashMap<i32, LevelingInfo>>,
}

impl SoulCrystalData {
    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub fn insert_crystal_for_test(&mut self, item_id: i32, level: i32, leveled_item_id: i32) {
        self.crystals.insert(
            item_id,
            SoulCrystal {
                level,
                leveled_item_id,
            },
        );
    }

    #[doc(hidden)]
    pub fn insert_npc_level_for_test(&mut self, npc_id: i32, level: i32, info: LevelingInfo) {
        self.npc_info.entry(npc_id).or_default().insert(level, info);
    }

    /// The crystal record for an item id, if it is a Soul Crystal.
    pub fn crystal(&self, item_id: i32) -> Option<&SoulCrystal> {
        self.crystals.get(&item_id)
    }

    /// The per-level leveling map for a monster, if it can raise crystals.
    pub fn npc_levels(&self, npc_id: i32) -> Option<&HashMap<i32, LevelingInfo>> {
        self.npc_info.get(&npc_id)
    }

    /// The leveling rule for a specific crystal level at this monster.
    pub fn leveling_info(&self, npc_id: i32, crystal_level: i32) -> Option<&LevelingInfo> {
        self.npc_info.get(&npc_id)?.get(&crystal_level)
    }

    /// The union of every monster registered for crystal leveling — the quest's
    /// `addKillId` / `addSkillSeeId` set.
    pub fn leveling_npc_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.npc_info.keys().copied()
    }
}
