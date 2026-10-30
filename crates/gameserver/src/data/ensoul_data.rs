//! Port of `data/xml/EnsoulData` — `data/stats/ensoul/*.xml`: the soul crystal
//! options a weapon can carry, the crystals (stones) that grant them, and the
//! gemstone fee per weapon grade.
//!
//! * `<option id name desc skillId skillLevel/>` — one ability. It is nothing
//!   but a skill the wearer gets while the weapon is equipped.
//! * `<stone id slotType>` + `<option id/>` children — a soul crystal item and
//!   the options it may grant. `slotType` 1 is a regular slot, 2 the special
//!   (mystic) one.
//! * `<fee crystalType>` — the gemstone cost of filling (`first`, `secondary`,
//!   `third`) or replacing (`reNormal`, `reSecondary`, `reThird`) a slot, plus
//!   any `remove` items. Index 2 (`third`/`reThird`) is the special slot.

use std::collections::HashMap;

use quick_xml::events::Event;
use tracing::info;

use crate::data::item_data::kinds::CrystalType;
use crate::data::xml;

const ENSOUL_DIR: &str = "data/stats/ensoul";

/// Java `EnsoulOption` — an ability and the skill it grants.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnsoulOption {
    pub id: i32,
    pub name: String,
    pub desc: String,
    pub skill_id: i32,
    pub skill_level: i32,
}

/// Java `EnsoulStone` — a soul crystal and the options it can grant.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnsoulStone {
    pub id: i32,
    /// 1 = regular slot, 2 = special slot.
    pub slot_type: i32,
    /// In document order.
    pub options: Vec<i32>,
}

/// Java `EnsoulFee` — one grade's costs, each an `(item_id, count)`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EnsoulFee {
    ensoul: [Option<(i32, i64)>; 3],
    resoul: [Option<(i32, i64)>; 3],
    removal: Vec<(i32, i64)>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EnsoulData {
    fees: HashMap<CrystalType, EnsoulFee>,
    options: HashMap<i32, EnsoulOption>,
    stones: HashMap<i32, EnsoulStone>,
}

impl EnsoulData {
    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        for path in xml::xml_files_under(format!("{file_path}{ENSOUL_DIR}")) {
            match std::fs::read_to_string(&path) {
                Ok(content) => data.parse(&content),
                Err(e) => tracing::warn!("EnsoulData: cannot read {}: {e}", path.display()),
            }
        }
        info!("EnsoulData: Loaded {} fees.", data.fees.len());
        info!("EnsoulData: Loaded {} options.", data.options.len());
        info!("EnsoulData: Loaded {} stones.", data.stones.len());
        data
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    fn parse(&mut self, content: &str) {
        // The open `<fee>`: its grade, what has been read so far, and whether
        // a `first`/`secondary`/`third` child has been seen.
        let mut fee: Option<(CrystalType, EnsoulFee, bool)> = None;
        let mut stone: Option<EnsoulStone> = None;
        for event in xml::events(content) {
            match event {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let self_closing = matches!(event, Event::Empty(_));
                    let name = e.name();
                    let item = || {
                        (
                            xml::attr_i32(e, "itemId").unwrap_or(0),
                            xml::attr_i64(e, "count").unwrap_or(0),
                        )
                    };
                    match name.as_ref() {
                        "fee" => {
                            let grade =
                                CrystalType::from_name(xml::attr_str(e, "crystalType").as_deref());
                            fee = Some((grade, EnsoulFee::default(), false));
                        }
                        "first" | "secondary" | "third" => {
                            if let Some((_, f, put)) = fee.as_mut() {
                                let index = match name.as_ref() {
                                    "first" => 0,
                                    "secondary" => 1,
                                    _ => 2,
                                };
                                f.ensoul[index] = Some(item());
                                *put = true;
                            }
                        }
                        "reNormal" | "reSecondary" | "reThird" => {
                            if let Some((_, f, _)) = fee.as_mut() {
                                let index = match name.as_ref() {
                                    "reNormal" => 0,
                                    "reSecondary" => 1,
                                    _ => 2,
                                };
                                f.resoul[index] = Some(item());
                            }
                        }
                        "remove" => {
                            if let Some((_, f, _)) = fee.as_mut() {
                                f.removal.push(item());
                            }
                        }
                        "option" if stone.is_some() => {
                            if let Some(s) = stone.as_mut() {
                                s.options.push(xml::attr_i32(e, "id").unwrap_or(0));
                            }
                        }
                        "option" => {
                            let option = EnsoulOption {
                                id: xml::attr_i32(e, "id").unwrap_or(0),
                                name: xml::attr_str(e, "name").unwrap_or_default(),
                                desc: xml::attr_str(e, "desc").unwrap_or_default(),
                                skill_id: xml::attr_i32(e, "skillId").unwrap_or(0),
                                skill_level: xml::attr_i32(e, "skillLevel").unwrap_or(0),
                            };
                            self.options.insert(option.id, option);
                        }
                        "stone" => {
                            let s = EnsoulStone {
                                id: xml::attr_i32(e, "id").unwrap_or(0),
                                slot_type: xml::attr_i32(e, "slotType").unwrap_or(0),
                                options: Vec::new(),
                            };
                            // A self-closing `<stone/>` has no children and no
                            // end tag to file it on.
                            if self_closing {
                                self.stones.insert(s.id, s);
                            } else {
                                stone = Some(s);
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(e) => match e.name().as_ref() {
                    // Java files the fee from `parseFee`, i.e. on each
                    // `first`/`secondary`/`third` child, with a `put` that
                    // replaces whatever the grade held. So a block with none of
                    // those is never filed, and a **second block for the same
                    // grade replaces the first wholesale**. The shipped file
                    // does exactly that for `C`: its second block, a lone
                    // `first` of 10 Soul Crystal Practice Gemstones, wipes the
                    // gemstone costs of the first and leaves C with no
                    // `third` and no re-ensoul fees at all.
                    "fee" => {
                        if let Some((grade, f, true)) = fee.take() {
                            self.fees.insert(grade, f);
                        }
                    }
                    "stone" => {
                        if let Some(s) = stone.take() {
                            self.stones.insert(s.id, s);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    /// Java `getEnsoulFee(type, index)`. `None` for a grade with no fee block
    /// and for an index the block leaves empty.
    ///
    /// Java indexes a three-slot array, so any index past 2 throws. Callers
    /// that can reach one have to decide what that exception means at their
    /// site; here it is just `None`.
    pub fn ensoul_fee(&self, grade: CrystalType, index: usize) -> Option<(i32, i64)> {
        self.fees.get(&grade)?.ensoul.get(index).copied().flatten()
    }

    /// Java `getResoulFee(type, index)` — the cost of replacing a filled slot.
    pub fn resoul_fee(&self, grade: CrystalType, index: usize) -> Option<(i32, i64)> {
        self.fees.get(&grade)?.resoul.get(index).copied().flatten()
    }

    /// Java `getRemovalFee(type)`.
    pub fn removal_fee(&self, grade: CrystalType) -> &[(i32, i64)] {
        self.fees.get(&grade).map_or(&[], |f| f.removal.as_slice())
    }

    /// Java `getOption(id)`.
    pub fn option(&self, id: i32) -> Option<&EnsoulOption> {
        self.options.get(&id)
    }

    /// Java `getStone(id)`.
    pub fn stone(&self, id: i32) -> Option<&EnsoulStone> {
        self.stones.get(&id)
    }

    /// Java `getStone(type, optionId)` — the first stone of `slot_type` that
    /// lists `option_id`, 0 when none does.
    ///
    /// Java walks a `HashMap`, so when several stones list the same option the
    /// one returned is whichever the hash order meets first. The port takes the
    /// lowest stone id, which is at least the same answer on every run.
    pub fn stone_for_option(&self, slot_type: i32, option_id: i32) -> i32 {
        self.stones
            .values()
            .filter(|s| s.slot_type == slot_type && s.options.contains(&option_id))
            .map(|s| s.id)
            .min()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist() -> EnsoulData {
        EnsoulData::load_from(crate::data::DIST_GAME)
    }

    #[test]
    fn the_dist_loads_every_option_stone_and_grade() {
        let d = dist();
        assert_eq!(d.options.len(), 742);
        assert_eq!(d.stones.len(), 104);
        // Eleven blocks, ten grades: C's two blocks file once.
        assert_eq!(d.fees.len(), 10);

        let might = d.option(1).unwrap();
        assert_eq!((might.skill_id, might.skill_level), (13856, 1));
        let stone = d.stone(46444).unwrap();
        assert_eq!(stone.slot_type, 1);
        assert_eq!(stone.options, vec![1, 16, 31, 46, 61, 76, 91]);
        assert_eq!(d.stone_for_option(1, 16), 46444);
        assert_eq!(d.stone_for_option(2, 16), 0, "wrong slot type");
    }

    /// The second `C` block replaces the first, so C keeps only the practice
    /// gemstone fee for the first slot and loses everything else.
    #[test]
    fn the_second_c_block_replaces_the_first() {
        let d = dist();
        assert_eq!(d.ensoul_fee(CrystalType::C, 0), Some((36722, 10)));
        assert_eq!(d.ensoul_fee(CrystalType::C, 2), None);
        assert_eq!(d.resoul_fee(CrystalType::C, 0), None);
        // A grade with a single block keeps all of it.
        assert_eq!(d.ensoul_fee(CrystalType::B, 0), Some((2132, 112)));
        assert_eq!(d.ensoul_fee(CrystalType::B, 1), None, "B has no secondary");
        assert_eq!(d.resoul_fee(CrystalType::B, 2), Some((2132, 19)));
        assert_eq!(d.ensoul_fee(CrystalType::D, 0), None, "D has no block");
        assert_eq!(d.ensoul_fee(CrystalType::R, 3), None, "past the array");
    }
}
