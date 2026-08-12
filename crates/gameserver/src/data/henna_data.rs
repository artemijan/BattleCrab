//! Port of `data/xml/HennaData` + `model/item/Henna` (G16): the dye symbols from
//! `dist/game/data/stats/hennaList.xml`. The runtime flow (draw/remove windows,
//! stat application) lives in `game_loop/henna.rs`.
//!
//! Interlude hennas carry six base-stat bonuses (STR/CON/DEX/INT/MEN/WIT); the
//! `luc`/`cha` stats present in the Classic XML are ignored by the Java `Henna`
//! constructor and dropped here too. `duration` is -1 (permanent) for every dye
//! on this dist, so the timed-henna scheduler is out of scope. `<skill>`
//! entries (none on Interlude dyes) are likewise skipped.

use std::collections::HashMap;

use quick_xml::events::Event;
use tracing::{info, warn};

use crate::data::xml;
use crate::data::xml::attr_str;
use crate::model::stats::BaseStat;

const HENNA_FILE: &str = "data/stats/hennaList.xml";

/// Port of `model/item/Henna` — one dye symbol.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Henna {
    pub dye_id: i32,
    pub dye_item_id: i32,
    /// STR/CON/DEX/INT/MEN/WIT bonus (may be negative).
    pub str_: i32,
    pub con: i32,
    pub dex: i32,
    pub int_: i32,
    pub men: i32,
    pub wit: i32,
    /// Dyes + adena consumed to draw the symbol.
    pub wear_count: i64,
    pub wear_fee: i64,
    /// Dyes returned + adena consumed to remove the symbol.
    pub cancel_count: i64,
    pub cancel_fee: i64,
    /// Class ids allowed to wear this dye (`<classId>` entries).
    pub wear_classes: Vec<i32>,
}

impl Henna {
    /// `Henna.getBaseStats(stat)` for the six Interlude stats (0 otherwise).
    pub fn base_stat(&self, stat: BaseStat) -> i32 {
        match stat {
            BaseStat::Str => self.str_,
            BaseStat::Con => self.con,
            BaseStat::Dex => self.dex,
            BaseStat::Int => self.int_,
            BaseStat::Men => self.men,
            BaseStat::Wit => self.wit,
        }
    }

    /// `Henna.isAllowedClass(classId)`.
    pub fn is_allowed_class(&self, class_id: i32) -> bool {
        self.wear_classes.contains(&class_id)
    }
}

/// The summed base-stat bonus of a player's worn dyes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HennaStatSums {
    pub str_: i32,
    pub con: i32,
    pub dex: i32,
    pub int_: i32,
    pub men: i32,
    pub wit: i32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HennaData {
    hennas: HashMap<i32, Henna>,
}

impl HennaData {
    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        let path = format!("{file_path}{HENNA_FILE}");
        match std::fs::read_to_string(&path) {
            Ok(content) => data.parse(&content),
            Err(e) => warn!("HennaData: cannot read {path}: {e}"),
        }
        info!("HennaData: Loaded {} henna data.", data.hennas.len());
        data
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, henna: Henna) {
        self.hennas.insert(henna.dye_id, henna);
    }

    /// `HennaData.getHenna(id)`.
    pub fn get(&self, dye_id: i32) -> Option<&Henna> {
        self.hennas.get(&dye_id)
    }

    /// `HennaData.getHennaList(classId)` — the dyes a class may draw, dye-id
    /// order (stable so the equip/remove windows list them consistently).
    pub fn list_for_class(&self, class_id: i32) -> Vec<&Henna> {
        let mut list: Vec<&Henna> = self
            .hennas
            .values()
            .filter(|h| h.is_allowed_class(class_id))
            .collect();
        list.sort_by_key(|h| h.dye_id);
        list
    }

    /// Sum the six base-stat bonuses of the worn dyes (Java `_hennaBaseStats`,
    /// rebuilt by `recalcHennaStats`). Unknown dye ids contribute nothing.
    pub fn stat_sums(&self, slots: &[Option<i32>; 3]) -> HennaStatSums {
        let mut s = HennaStatSums::default();
        for dye_id in slots.iter().filter_map(|d| *d) {
            if let Some(h) = self.get(dye_id) {
                s.str_ += h.str_;
                s.con += h.con;
                s.dex += h.dex;
                s.int_ += h.int_;
                s.men += h.men;
                s.wit += h.wit;
            }
        }
        s
    }

    pub fn len(&self) -> usize {
        self.hennas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hennas.is_empty()
    }

    fn parse(&mut self, content: &str) {
        let mut cur: Option<Henna> = None;
        for event in xml::events(content) {
            match event {
                Event::Start(e) | Event::Empty(e) => {
                    let attr = |key: &[u8]| attr_str(&e, key);
                    let num = |key: &[u8]| attr(key).and_then(|v| v.parse::<i64>().ok());
                    match e.name().as_ref() {
                        b"henna" => {
                            let (Some(dye_id), Some(dye_item_id)) =
                                (num(b"dyeId"), num(b"dyeItemId"))
                            else {
                                cur = None;
                                continue;
                            };
                            cur = Some(Henna {
                                dye_id: dye_id as i32,
                                dye_item_id: dye_item_id as i32,
                                str_: 0,
                                con: 0,
                                dex: 0,
                                int_: 0,
                                men: 0,
                                wit: 0,
                                wear_count: 0,
                                wear_fee: 0,
                                cancel_count: 0,
                                cancel_fee: 0,
                                wear_classes: Vec::new(),
                            });
                        }
                        b"stats" => {
                            if let Some(h) = cur.as_mut() {
                                h.str_ = num(b"str").unwrap_or(0) as i32;
                                h.con = num(b"con").unwrap_or(0) as i32;
                                h.dex = num(b"dex").unwrap_or(0) as i32;
                                h.int_ = num(b"int").unwrap_or(0) as i32;
                                h.men = num(b"men").unwrap_or(0) as i32;
                                h.wit = num(b"wit").unwrap_or(0) as i32;
                            }
                        }
                        b"wear" => {
                            if let Some(h) = cur.as_mut() {
                                h.wear_count = num(b"count").unwrap_or(0);
                                h.wear_fee = num(b"fee").unwrap_or(0);
                            }
                        }
                        b"cancel" => {
                            if let Some(h) = cur.as_mut() {
                                h.cancel_count = num(b"count").unwrap_or(0);
                                h.cancel_fee = num(b"fee").unwrap_or(0);
                            }
                        }
                        // <duration>/<skill> are absent on Interlude dyes; ignored.
                        _ => {}
                    }
                }
                // `<classId>N</classId>`: the text child is the allowed class id.
                Event::Text(t) => {
                    if let Some(h) = cur.as_mut()
                        && let Ok(s) = t.unescape()
                        && let Ok(id) = s.trim().parse::<i32>()
                    {
                        h.wear_classes.push(id);
                    }
                }
                Event::End(e) if e.name().as_ref() == b"henna" => {
                    if let Some(h) = cur.take() {
                        self.hennas.insert(h.dye_id, h);
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

    fn load() -> HennaData {
        HennaData::load_from(&format!("{}/../../dist/game/", env!("CARGO_MANIFEST_DIR")))
    }

    #[test]
    fn loads_real_dist_hennas() {
        let data = load();
        assert_eq!(data.len(), 372);

        // dye 1 (dye_s1c3_d): STR +1, CON -3, item 4445, wear 10/37000, cancel 5/7400.
        let h = data.get(1).expect("henna 1");
        assert_eq!(h.dye_item_id, 4445);
        assert_eq!(h.str_, 1);
        assert_eq!(h.con, -3);
        assert_eq!(h.dex, 0);
        assert_eq!(h.wear_count, 10);
        assert_eq!(h.wear_fee, 37000);
        assert_eq!(h.cancel_count, 5);
        assert_eq!(h.cancel_fee, 7400);
        assert!(h.is_allowed_class(11), "Human Wizard allowed");
        assert!(
            !h.is_allowed_class(0),
            "Human Fighter not in this dye's list"
        );

        // The class filter returns only allowed dyes.
        assert!(data.list_for_class(11).iter().any(|h| h.dye_id == 1));
    }
}
