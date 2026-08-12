//! Port of `CastleManorManager`'s seed catalogue — `data/Seeds.xml`. Each
//! `<castle>` block lists the crops that can be farmed at that castle's manor
//! (Java `model/Seed`). Manor is gated by `AllowManor` (off on this dist), but
//! the data is loaded regardless so it works the moment an operator enables it.

use crate::data::xml::attr_i32_trimmed as attr_i32;
use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

const SEEDS_XML: &str = "data/Seeds.xml";

/// One farmable crop line (Java `model/Seed`). The seed/crop **reference
/// prices** Java resolves from item data are not stored here — the sow/harvest
/// slices resolve them from `ItemData` at use time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Seed {
    /// The castle whose manor offers this seed (from the parent `<castle id>`).
    pub castle_id: i32,
    /// `seedId` — the item sown into a mob.
    pub seed_id: i32,
    /// `id` — the crop item the mob then yields.
    pub crop_id: i32,
    /// `mature_Id` — the crop matured at the manor into the reward.
    pub mature_id: i32,
    pub level: i32,
    pub reward1: i32,
    pub reward2: i32,
    /// `alternative` — the alternative (higher-limit, lower-yield) seed line.
    pub alternative: bool,
    pub limit_seeds: i32,
    pub limit_crops: i32,
}

impl Seed {
    /// `Seed.getReward(type)` — reward item id for type 1 or 2.
    pub fn reward(&self, reward_type: i32) -> i32 {
        if reward_type == 1 {
            self.reward1
        } else {
            self.reward2
        }
    }
}

/// The seed catalogue keyed by castle id (Java `CastleManorManager._seeds`).
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManorData {
    by_castle: HashMap<i32, Vec<Seed>>,
}

impl ManorData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let content =
            std::fs::read_to_string(format!("{file_path}{SEEDS_XML}")).unwrap_or_default();
        let by_castle = parse(&content);
        let total: usize = by_castle.values().map(Vec::len).sum();
        info!(
            "ManorData: Loaded {total} seeds across {} castles.",
            by_castle.len()
        );
        Self { by_castle }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            by_castle: HashMap::new(),
        }
    }

    /// The seeds farmable at a castle's manor (Java `getSeedsForCastle`).
    pub fn seeds_for_castle(&self, castle_id: i32) -> &[Seed] {
        self.by_castle
            .get(&castle_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The castle ids that offer a manor (Java `ExSendManorList` — every castle
    /// with seed data), sorted ascending.
    pub fn manor_castle_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.by_castle.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// The reference crop table (Java `getCrops`) — one seed per distinct crop
    /// id across every castle, used by `ExShowManorDefaultInfo`. Java iterates
    /// `_seeds.values()` in `HashMap` order and keeps the first seed seen for
    /// each crop; we iterate castles in id order (then file order within a
    /// castle) so the list is deterministic — the client doesn't rely on order.
    pub fn all_crops(&self) -> Vec<&Seed> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for castle_id in self.manor_castle_ids() {
            for seed in self.seeds_for_castle(castle_id) {
                if seen.insert(seed.crop_id) {
                    out.push(seed);
                }
            }
        }
        out
    }

    /// The seed with a given seed id at a castle (Java `getSeed`).
    pub fn seed(&self, castle_id: i32, seed_id: i32) -> Option<&Seed> {
        self.seeds_for_castle(castle_id)
            .iter()
            .find(|s| s.seed_id == seed_id)
    }

    /// Java global `getSeed(seedId)` — the seed with this id, regardless of
    /// castle (Java's `_seeds` map is keyed by seed id). Seed ids are unique
    /// across the catalogue, so the first match is the only one.
    pub fn seed_by_id(&self, seed_id: i32) -> Option<&Seed> {
        self.by_castle
            .values()
            .flatten()
            .find(|s| s.seed_id == seed_id)
    }

    /// Java global `getSeedByCrop(cropId)` — the first seed yielding this crop,
    /// regardless of castle (used to resolve a procure line's level/rewards).
    pub fn seed_by_crop(&self, crop_id: i32) -> Option<&Seed> {
        self.by_castle
            .values()
            .flatten()
            .find(|s| s.crop_id == crop_id)
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, seed: Seed) {
        self.by_castle.entry(seed.castle_id).or_default().push(seed);
    }
}

fn parse(content: &str) -> HashMap<i32, Vec<Seed>> {
    let mut reader = Reader::from_str(content);
    let mut out: HashMap<i32, Vec<Seed>> = HashMap::new();
    let mut castle_id = 0;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"castle" => {
                castle_id = attr_i32(&e, b"id").unwrap_or(0);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"crop" => {
                if let (Some(crop_id), Some(seed_id)) =
                    (attr_i32(&e, b"id"), attr_i32(&e, b"seedId"))
                {
                    let alternative = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"alternative")
                        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
                        .as_deref()
                        == Some("true");
                    out.entry(castle_id).or_default().push(Seed {
                        castle_id,
                        seed_id,
                        crop_id,
                        mature_id: attr_i32(&e, b"mature_Id").unwrap_or(0),
                        level: attr_i32(&e, b"level").unwrap_or(0),
                        reward1: attr_i32(&e, b"reward1").unwrap_or(0),
                        reward2: attr_i32(&e, b"reward2").unwrap_or(0),
                        alternative,
                        limit_seeds: attr_i32(&e, b"limit_seed").unwrap_or(0),
                        limit_crops: attr_i32(&e, b"limit_crops").unwrap_or(0),
                    });
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    /// The dist `Seeds.xml` loads: all 9 castles offer a manor, and Gludio's
    /// first seed carries its full crop/reward/limit fields.
    #[test]
    fn loads_the_seed_catalogue() {
        let m = ManorData::load_from(DIST);
        assert_eq!(
            m.manor_castle_ids(),
            (1..=9).collect::<Vec<_>>(),
            "9 castles have manor data"
        );

        let seed = m.seed(1, 5016).expect("Gludio's seed 5016");
        assert_eq!(seed.crop_id, 5073);
        assert_eq!(seed.mature_id, 5103);
        assert_eq!(seed.level, 10);
        assert_eq!(seed.reward(1), 1864);
        assert_eq!(seed.reward(2), 1878);
        assert!(!seed.alternative);
        assert_eq!(seed.limit_seeds, 8100);

        // The same-level alternative line is flagged.
        assert!(
            m.seed(1, 5650).is_some_and(|s| s.alternative),
            "the alternative seed is flagged"
        );
        // A castle with no such seed returns nothing.
        assert!(m.seed(1, 999_999).is_none());
    }
}
