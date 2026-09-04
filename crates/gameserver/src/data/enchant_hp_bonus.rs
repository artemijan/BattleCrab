//! Port of `data/xml/EnchantItemHPBonusData` — `data/stats/enchantHPBonus.xml`,
//! the flat max-HP an **enchanted armour piece** grants on top of its own
//! stats.
//!
//! One list of twelve bonuses per crystal grade, indexed by enchant level: a
//! +4 B-grade breastplate is worth 21 HP, a +12 one 315. Nothing read this
//! file, so every enchanted armour set in the game was worth exactly its
//! unenchanted stats — which is the sort of gap a player notices as "my +10
//! set has less HP than it should" and a test suite never notices at all.

use crate::data::item_data::kinds::CrystalType;
use crate::data::xml;
use quick_xml::events::Event;
use std::collections::HashMap;
use tracing::info;

pub const ENCHANT_HP_BONUS_FILE: &str = "data/stats/enchantHPBonus.xml";

/// Java's `FULL_ARMOR_MODIFIER`, with its own `// TODO: Move it to config!`
/// still attached upstream: a one-piece suit is worth half again as much as the
/// same grade's chest piece.
const FULL_ARMOR_MODIFIER: f64 = 1.5;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EnchantHpBonusData {
    by_grade: HashMap<CrystalType, Vec<i32>>,
}

impl EnchantHpBonusData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let full_path = format!("{file_path}{ENCHANT_HP_BONUS_FILE}");
        let content = std::fs::read_to_string(&full_path).unwrap_or_else(|e| {
            let cwd = std::env::current_dir().unwrap();
            panic!("EnchantHpBonusData: cannot read {full_path}: {e}, CWD: {cwd:?}")
        });
        let mut by_grade: HashMap<CrystalType, Vec<i32>> = HashMap::new();
        let mut current: Option<CrystalType> = None;
        let mut text = String::new();
        for event in xml::events(&content) {
            match event {
                Event::Start(e) if e.name().as_ref() == b"enchantHP" => {
                    // The file declares `grade="S80"` **twice**. Java stores
                    // into an `EnumMap`, so the second row silently replaces the
                    // first; inserting into a map here does the same thing.
                    current = Some(CrystalType::from_name(
                        xml::attr_str(&e, b"grade").as_deref(),
                    ));
                    if let Some(g) = current {
                        by_grade.insert(g, Vec::with_capacity(12));
                    }
                }
                Event::Start(e) if e.name().as_ref() == b"bonus" => {
                    text.clear();
                    let _ = e;
                }
                Event::Text(t) => {
                    text = String::from_utf8_lossy(t.as_ref()).trim().to_string();
                }
                Event::End(e) if e.name().as_ref() == b"bonus" => {
                    if let Some(g) = current
                        && let Ok(v) = text.parse::<i32>()
                        && let Some(list) = by_grade.get_mut(&g)
                    {
                        list.push(v);
                    }
                    text.clear();
                }
                Event::End(e) if e.name().as_ref() == b"enchantHP" => current = None,
                _ => {}
            }
        }
        info!(
            "EnchantHpBonusData: Loaded {} enchant HP bonuses.",
            by_grade.len()
        );
        Self { by_grade }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Java `getHPBonus(item)`.
    ///
    /// `enchant <= 0` is worth nothing, and the level indexes the grade's list
    /// clamped to its length — so a +20 piece keeps the +12 figure rather than
    /// running off the end. `SLOT_FULL_ARMOR` takes the ×1.5.
    ///
    /// The grade is `getCrystalTypePlus()`, which folds S80/S84 into S — a
    /// no-op below S, i.e. everywhere on this chronicle.
    ///
    /// Java reads `getOlyEnchantLevel()` rather than the raw level, which caps
    /// the enchant during an Olympiad match at `AltOlyArmorEnchantLimit`. That
    /// key is **-1 on this dist** (no limit), so the two are the same number
    /// here and the caller needs no Olympiad context.
    pub fn bonus(&self, grade: CrystalType, enchant: i32, body_part: i32) -> f64 {
        if enchant <= 0 {
            return 0.0;
        }
        let Some(values) = self.by_grade.get(&grade.plus()) else {
            return 0.0;
        };
        if values.is_empty() {
            return 0.0;
        }
        let index = (enchant.min(values.len() as i32) - 1) as usize;
        let bonus = f64::from(values[index]);
        if body_part == crate::data::item_data::SLOT_FULL_ARMOR {
            // Java truncates the product to `int`.
            (bonus * FULL_ARMOR_MODIFIER).trunc()
        } else {
            bonus
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DIST_GAME;
    use crate::data::item_data::SLOT_FULL_ARMOR;

    /// The shipped table, read back at the grades and levels Interlude uses.
    #[test]
    fn real_dist_enchant_hp_bonus_loads() {
        let d = EnchantHpBonusData::load_from(DIST_GAME);

        // No enchant, no bonus — the `<= 0` gate, not a zero row.
        assert_eq!(d.bonus(CrystalType::B, 0, 0), 0.0);
        // The B-grade ladder climbs with the enchant level.
        let b4 = d.bonus(CrystalType::B, 4, 0);
        let b12 = d.bonus(CrystalType::B, 12, 0);
        assert!(b4 > 0.0, "a +4 B-grade piece is worth something");
        assert!(b12 > b4, "and a +12 one more ({b4} → {b12})");

        // Past the table's end the top row stands in, rather than panicking or
        // reading zero.
        assert_eq!(d.bonus(CrystalType::B, 30, 0), b12);

        // A one-piece suit is worth 1.5x the same figure.
        assert_eq!(
            d.bonus(CrystalType::B, 12, SLOT_FULL_ARMOR),
            (b12 * 1.5).trunc()
        );

        // `getCrystalTypePlus` — S80 reads the S row.
        assert_eq!(
            d.bonus(CrystalType::S80, 6, 0),
            d.bonus(CrystalType::S, 6, 0)
        );
    }
}
