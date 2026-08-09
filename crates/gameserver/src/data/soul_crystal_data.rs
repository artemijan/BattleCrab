//! Port of `data/LevelUpCrystalData.xml`, loaded by quest 350 (Enhance Your
//! Weapon — the Soul Crystal system). It answers two questions:
//!   * given a Soul Crystal item, what level is it and what does it level *into*?
//!   * given a monster, which crystal levels can it raise, how, and at what odds?
//!
//! The crystal chain runs Red/Green/Blue stages 0→18; Interlude only reaches
//! ~13, so the higher rungs are simply unreachable data, not a problem.

use std::collections::HashMap;

use crate::data::xml::attr_str;
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::{info, warn};

const CRYSTAL_FILE: &str = "data/LevelUpCrystalData.xml";

/// How a monster distributes a crystal level-up across a party (Java
/// `AbsorbCrystalType`). Solo play collapses every variant to "the killer".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

impl AbsorbType {
    fn parse(s: &str) -> Self {
        match s {
            "FULL_PARTY" => Self::FullParty,
            "PARTY_ONE_RANDOM" => Self::PartyOneRandom,
            "PARTY_RANDOM" => Self::PartyRandom,
            _ => Self::LastHit,
        }
    }
}

/// One monster's leveling rule for a given crystal level.
#[derive(Debug, Clone, Copy)]
pub struct LevelingInfo {
    pub absorb_type: AbsorbType,
    /// Whether the Soul Crystal skill (2096) must have been cast on the mob
    /// (below half HP) before the kill.
    pub skill_needed: bool,
    /// Percent chance (`Rnd.get(100) <= chance`) that the crystal levels.
    pub chance: i32,
}

/// A Soul Crystal item: its stage and the item it becomes when it levels.
#[derive(Debug, Clone, Copy)]
pub struct SoulCrystal {
    pub level: i32,
    pub leveled_item_id: i32,
}

#[derive(Debug, Default)]
pub struct SoulCrystalData {
    /// item id → the crystal's stage + next-stage item id.
    crystals: HashMap<i32, SoulCrystal>,
    /// npc id → (crystal level → how that level absorbs from this mob).
    npc_info: HashMap<i32, HashMap<i32, LevelingInfo>>,
}

impl SoulCrystalData {
    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        let path = format!("{file_path}{CRYSTAL_FILE}");
        match std::fs::read_to_string(&path) {
            Ok(content) => data.parse(&content),
            Err(e) => warn!("SoulCrystalData: cannot read {path}: {e}"),
        }
        info!(
            "SoulCrystalData: Loaded {} crystals, {} npc leveling entries.",
            data.crystals.len(),
            data.npc_info.len()
        );
        data
    }

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

    fn parse(&mut self, content: &str) {
        let mut reader = Reader::from_str(content);
        // Which section we are inside (`<item>` means different things in each).
        let mut in_npc_section = false;
        // The npc id of the `<item>` currently open in the npc section.
        let mut cur_npc: Option<i32> = None;
        loop {
            let event = match reader.read_event() {
                Ok(e) => e,
                Err(_) => break,
            };
            match event {
                Event::Start(e) | Event::Empty(e) => {
                    let attr = |key: &[u8]| attr_str(&e, key);
                    let num = |key: &[u8]| attr(key).and_then(|v| v.parse::<i32>().ok());
                    match e.name().as_ref() {
                        b"npc" => in_npc_section = true,
                        b"crystal" => in_npc_section = false,
                        b"item" if !in_npc_section => {
                            if let (Some(item_id), Some(level), Some(leveled)) =
                                (num(b"itemId"), num(b"level"), num(b"leveledItemId"))
                            {
                                self.crystals.insert(
                                    item_id,
                                    SoulCrystal {
                                        level,
                                        leveled_item_id: leveled,
                                    },
                                );
                            }
                        }
                        b"item" if in_npc_section => {
                            cur_npc = num(b"npcId");
                            if let Some(id) = cur_npc {
                                self.npc_info.entry(id).or_default();
                            }
                        }
                        b"detail" => {
                            let Some(npc_id) = cur_npc else { continue };
                            let info = LevelingInfo {
                                absorb_type: attr(b"absorbType")
                                    .map(|s| AbsorbType::parse(&s))
                                    .unwrap_or_default(),
                                skill_needed: attr(b"skill").map(|s| s == "true").unwrap_or(false),
                                chance: num(b"chance").unwrap_or(5),
                            };
                            let map = self.npc_info.entry(npc_id).or_default();
                            // `maxLevel="N"` → levels 0..=N; `levelList="a,b,c"`
                            // → exactly those levels.
                            if let Some(max_level) = num(b"maxLevel") {
                                for lvl in 0..=max_level {
                                    map.insert(lvl, info);
                                }
                            } else if let Some(list) = attr(b"levelList") {
                                for tok in list.split(',') {
                                    if let Ok(lvl) = tok.trim().parse::<i32>() {
                                        map.insert(lvl, info);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(e) => {
                    if e.name().as_ref() == b"item" && in_npc_section {
                        cur_npc = None;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_crystals_and_npc_sections() {
        let xml = r#"<?xml version="1.0"?>
        <list>
          <crystal>
            <item itemId="4629" level="0" leveledItemId="4630" />
            <item itemId="4630" level="1" leveledItemId="4631" />
          </crystal>
          <npc>
            <item npcId="20583">
              <detail chance="100" skill="true" maxLevel="1" />
            </item>
            <item npcId="29022">
              <detail chance="30" absorbType="FULL_PARTY" levelList="10,11,12" />
            </item>
          </npc>
        </list>"#;
        let mut data = SoulCrystalData::default();
        data.parse(xml);

        // Crystal chain.
        assert_eq!(data.crystal(4629).unwrap().level, 0);
        assert_eq!(data.crystal(4629).unwrap().leveled_item_id, 4630);
        assert_eq!(data.crystal(4630).unwrap().level, 1);
        assert!(data.crystal(9999).is_none());

        // `maxLevel="1"` → levels 0 and 1, skill-needed, LAST_HIT (default).
        let l0 = data.leveling_info(20583, 0).unwrap();
        assert!(l0.skill_needed);
        assert_eq!(l0.chance, 100);
        assert_eq!(l0.absorb_type, AbsorbType::LastHit);
        assert!(data.leveling_info(20583, 1).is_some());
        assert!(data.leveling_info(20583, 2).is_none());

        // `levelList` + explicit absorbType, and `skill` defaults to false.
        let z = data.leveling_info(29022, 11).unwrap();
        assert!(!z.skill_needed);
        assert_eq!(z.absorb_type, AbsorbType::FullParty);
        assert!(data.leveling_info(29022, 10).is_some());
        assert!(data.leveling_info(29022, 13).is_none());
    }

    #[test]
    fn loads_the_real_dist_file() {
        let root = crate::data::DIST_GAME;
        let data = SoulCrystalData::load_from(root);
        // The Red/Green/Blue stage-0 crystals and a well-known leveling mob.
        assert_eq!(data.crystal(4629).unwrap().level, 0); // Red 0
        assert_eq!(data.crystal(4640).unwrap().level, 0); // Green 0
        assert_eq!(data.crystal(4651).unwrap().level, 0); // Blue 0
        assert!(
            data.leveling_npc_ids().count() > 100,
            "the dist registers 100+ leveling mobs"
        );
    }
}
