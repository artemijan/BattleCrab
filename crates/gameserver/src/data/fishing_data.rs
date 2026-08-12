//! Port of `data/Fishing.xml` + `model/fishing/{FishingBait,FishingCatch,
//! FishingRod}` (G32). This dist runs the **new** single-action fishing system
//! (the old Pumping/Reeling minigame skills 1312–1314 are flagged "Unused"):
//! equip a fishing rod, hook a bait in the off-hand, cast in a fishing zone, and
//! after a wait the line reels in on the bait's win chance — a hit rolls the
//! bait's catch table for a fish (or a treasure box).

use std::collections::HashMap;

use crate::data::xml;
use crate::data::xml::attr_str;
use quick_xml::events::Event;
use tracing::{info, warn};

const FISHING_FILE: &str = "data/Fishing.xml";

/// One entry in a bait's catch table (`<catch>`).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct FishingCatch {
    pub item_id: i32,
    /// Weight within the bait's table (the `<catch chance>` values sum to 100).
    pub chance: i32,
    /// Java `getMultiplier` — scales the XP/SP the catch is worth.
    pub multiplier: i32,
}

/// A bait/lure (`<bait>`): what it can catch, how long a cast takes, and the
/// per-cast win chance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FishingBait {
    pub min_player_level: i32,
    pub max_player_level: i32,
    /// Percent chance a cast lands a catch (Java `getChance`, doubled by fish
    /// soulshots).
    pub chance: i32,
    /// Time-to-reel window (ms); `max` defaults to `min` when the XML omits it.
    pub time_min: i32,
    pub time_max: i32,
    /// Post-reel wait before the line auto-recasts (ms).
    pub wait_min: i32,
    pub wait_max: i32,
    pub premium_only: bool,
    pub catches: Vec<FishingCatch>,
}

impl FishingBait {
    /// Java `FishingBait.getRandom`: pick a catch weighted by its `chance`.
    /// `roll` is a caller-supplied `0..100` value (so the RNG stays testable).
    pub fn pick_catch(&self, roll: i32) -> Option<&FishingCatch> {
        let mut acc = 0;
        for c in &self.catches {
            acc += c.chance;
            if roll < acc {
                return Some(c);
            }
        }
        self.catches.last()
    }
}

/// A fishing rod (`<rod>`): shaves the reel time and can boost XP/SP.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct FishingRod {
    pub reduce_fishing_time: i32,
    pub xp_multiplier: f64,
    pub sp_multiplier: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FishingData {
    baits: HashMap<i32, FishingBait>,
    rods: HashMap<i32, FishingRod>,
    pub bait_distance_min: i32,
    pub bait_distance_max: i32,
    pub xp_rate_min: f64,
    pub xp_rate_max: f64,
    pub sp_rate_min: f64,
    pub sp_rate_max: f64,
}

impl Default for FishingData {
    fn default() -> Self {
        Self {
            baits: HashMap::new(),
            rods: HashMap::new(),
            bait_distance_min: 90,
            bait_distance_max: 250,
            xp_rate_min: 0.033,
            xp_rate_max: 0.033,
            sp_rate_min: 0.033,
            sp_rate_max: 0.033,
        }
    }
}

impl FishingData {
    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        let path = format!("{file_path}{FISHING_FILE}");
        match std::fs::read_to_string(&path) {
            Ok(content) => data.parse(&content),
            Err(e) => warn!("FishingData: cannot read {path}: {e}"),
        }
        info!(
            "FishingData: Loaded {} baits, {} rods.",
            data.baits.len(),
            data.rods.len()
        );
        data
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn bait(&self, item_id: i32) -> Option<&FishingBait> {
        self.baits.get(&item_id)
    }
    pub fn rod(&self, item_id: i32) -> Option<&FishingRod> {
        self.rods.get(&item_id)
    }

    #[doc(hidden)]
    pub fn insert_bait_for_test(&mut self, item_id: i32, bait: FishingBait) {
        self.baits.insert(item_id, bait);
    }
    #[doc(hidden)]
    pub fn insert_rod_for_test(&mut self, item_id: i32, rod: FishingRod) {
        self.rods.insert(item_id, rod);
    }

    fn parse(&mut self, content: &str) {
        let mut cur_bait: Option<(i32, FishingBait)> = None;
        for ev in xml::events(content) {
            match ev {
                Event::Start(e) | Event::Empty(e) => {
                    let attr = |key: &[u8]| attr_str(&e, key);
                    let num = |key: &[u8]| attr(key).and_then(|v| v.parse::<i32>().ok());
                    let flt = |key: &[u8]| attr(key).and_then(|v| v.parse::<f64>().ok());
                    match e.name().as_ref() {
                        b"baitDistance" => {
                            self.bait_distance_min = num(b"min").unwrap_or(90);
                            self.bait_distance_max = num(b"max").unwrap_or(250);
                        }
                        b"xpRate" => {
                            self.xp_rate_min = flt(b"min").unwrap_or(0.033);
                            self.xp_rate_max = flt(b"max").unwrap_or(self.xp_rate_min);
                        }
                        b"spRate" => {
                            self.sp_rate_min = flt(b"min").unwrap_or(0.033);
                            self.sp_rate_max = flt(b"max").unwrap_or(self.sp_rate_min);
                        }
                        b"bait" => {
                            if let Some(item_id) = num(b"itemId") {
                                let time_min = num(b"timeMin").unwrap_or(0);
                                let wait_min = num(b"waitMin").unwrap_or(0);
                                cur_bait = Some((
                                    item_id,
                                    FishingBait {
                                        min_player_level: num(b"minPlayerLevel").unwrap_or(1),
                                        max_player_level: num(b"maxPlayerLevel")
                                            .unwrap_or(i32::MAX),
                                        chance: num(b"chance").unwrap_or(0),
                                        time_min,
                                        time_max: num(b"timeMax").unwrap_or(time_min),
                                        wait_min,
                                        wait_max: num(b"waitMax").unwrap_or(wait_min),
                                        premium_only: attr(b"isPremiumOnly")
                                            .map(|s| s == "true")
                                            .unwrap_or(false),
                                        catches: Vec::new(),
                                    },
                                ));
                            }
                        }
                        b"catch" => {
                            if let (Some((_, bait)), Some(item_id)) =
                                (cur_bait.as_mut(), num(b"itemId"))
                            {
                                bait.catches.push(FishingCatch {
                                    item_id,
                                    chance: num(b"chance").unwrap_or(0),
                                    multiplier: num(b"multiplier").unwrap_or(1),
                                });
                            }
                        }
                        b"rod" => {
                            if let Some(item_id) = num(b"itemId") {
                                self.rods.insert(
                                    item_id,
                                    FishingRod {
                                        reduce_fishing_time: num(b"reduceFishingTime").unwrap_or(0),
                                        xp_multiplier: flt(b"xpMultiplier").unwrap_or(1.0),
                                        sp_multiplier: flt(b"spMultiplier").unwrap_or(1.0),
                                    },
                                );
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(e) => {
                    if e.name().as_ref() == b"bait"
                        && let Some((id, bait)) = cur_bait.take()
                    {
                        self.baits.insert(id, bait);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_real_dist_file() {
        let root = crate::data::DIST_GAME;
        let data = FishingData::load_from(root);
        // The starter bait and its catch table.
        let bait = data.bait(47547).expect("starter bait");
        assert_eq!(bait.chance, 40);
        assert_eq!(bait.time_min, 105000);
        assert_eq!(bait.catches.len(), 4);
        assert_eq!(bait.pick_catch(0).unwrap().item_id, 47550); // Ugly Fish (0..70)
        assert_eq!(bait.pick_catch(95).unwrap().item_id, 47552); // Powerful Fish (95..98)
        // A time-reducing rod.
        assert_eq!(data.rod(47557).expect("master rod").reduce_fishing_time, 10);
        assert!((data.xp_rate_min - 0.033).abs() < 1e-9);
    }
}
