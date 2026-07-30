//! Port of `VariationData` (`data/stats/augmentation/Variations.xml`) — the
//! **augmentation roll engine**. A life stone ("mineral") augments a weapon
//! with two rolled options; this resolves *which* two option ids come out, plus
//! the gemstone/adena fees.
//!
//! Structure (per Java `VariationData.parseDocument`):
//! - `<variation mineralId=…>` → per-weapon-type (`warrior`/`mage`) pair of
//!   `<optionGroup order="0"|"1">`. Each group is a list of `<optionCategory
//!   chance=…>`, each holding weighted `<option id chance>` / `<optionRange
//!   from-to chance>` entries.
//! - The augment rolls one option from the `order=0` group and one from
//!   `order=1` (Java `generateRandomVariation`): a weighted pick of a category
//!   by its chance, then a weighted pick of an option within it (`OptionDataGroup`
//!   / `OptionDataCategory.getRandom*`).
//! - `<itemGroups>` + `<fees>` map (weapon item id, mineral id) → the make
//!   gemstone cost and the cancel fee.
//!
//! Scope: this is the pure data + roll core (the enchant-engine pattern). What
//! each rolled option *does* — the 390k-line `stats/augmentation/options/*`
//! effect set (stat bonuses / granted skills) — and the refine Ex-packet client
//! flow are separate layers built on top of the option ids this produces.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const VARIATIONS_FILE: &str = "data/stats/augmentation/Variations.xml";

/// The two augment weapon-type routes (Java `VariationWeaponType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeaponKind {
    Warrior,
    Mage,
}

/// One `<optionCategory>` — a weighted set of option ids (Java
/// `OptionDataCategory`).
#[derive(Debug, Clone, Default)]
struct OptionCategory {
    chance: f64,
    /// `(option_id, chance)` in document order.
    options: Vec<(i32, f64)>,
}

impl OptionCategory {
    /// Weighted pick of one option id (Java `getRandomOptions`). `rng` yields
    /// `[0,1)`; the iteration-cap replaces Java's `do/while(result==null)`.
    fn random_option(&self, rng: &mut dyn FnMut() -> f64) -> Option<i32> {
        if self.options.is_empty() {
            return None;
        }
        for _ in 0..64 {
            let mut random = rng() * 100.0;
            for (id, chance) in &self.options {
                if *chance >= random {
                    return Some(*id);
                }
                random -= chance;
            }
        }
        // Degenerate weights (sum < 100): fall back to the last entry.
        self.options.last().map(|(id, _)| *id)
    }
}

/// One `<optionGroup>` — a weighted list of categories (Java `OptionDataGroup`).
#[derive(Debug, Clone, Default)]
struct OptionGroup {
    categories: Vec<OptionCategory>,
}

impl OptionGroup {
    /// Weighted pick of a category, then an option within it (Java
    /// `getRandomEffect`).
    fn random_effect(&self, rng: &mut dyn FnMut() -> f64) -> Option<i32> {
        if self.categories.is_empty() {
            return None;
        }
        for _ in 0..64 {
            let mut random = rng() * 100.0;
            for cat in &self.categories {
                if cat.chance >= random {
                    if let Some(id) = cat.random_option(rng) {
                        return Some(id);
                    }
                    break;
                }
                random -= cat.chance;
            }
        }
        self.categories.last().and_then(|c| c.random_option(rng))
    }
}

/// A life stone's variation (Java `Variation`): the two ordered option groups
/// for each weapon type.
#[derive(Debug, Clone, Default)]
struct Variation {
    warrior: [Option<OptionGroup>; 2],
    mage: [Option<OptionGroup>; 2],
}

impl Variation {
    fn groups(&self, kind: WeaponKind) -> &[Option<OptionGroup>; 2] {
        match kind {
            WeaponKind::Warrior => &self.warrior,
            WeaponKind::Mage => &self.mage,
        }
    }
}

/// The gemstone/adena cost of augmenting (and cancelling) with a given mineral
/// on a given weapon (Java `VariationFee`).
#[derive(Debug, Clone, Copy)]
pub struct VariationFee {
    /// The gemstone item id spent on the augment.
    pub item_id: i32,
    pub item_count: i64,
    /// The adena cost to remove the augment.
    pub cancel_fee: i64,
}

#[derive(Debug, Default)]
pub struct VariationData {
    variations: HashMap<i32, Variation>,
    /// weapon item id → (mineral id → fee).
    fees: HashMap<i32, HashMap<i32, VariationFee>>,
}

impl VariationData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{VARIATIONS_FILE}")) {
            data.parse(&content);
        }
        info!(
            "VariationData: Loaded {} variations, {} fee items.",
            data.variations.len(),
            data.fees.len()
        );
        data
    }

    pub fn empty() -> Self {
        Self::default()
    }

    /// Number of loaded life-stone variations (Java `getVariationCount`).
    pub fn variation_count(&self) -> usize {
        self.variations.len()
    }

    /// Whether `mineral_id` is a known life stone.
    pub fn has_variation(&self, mineral_id: i32) -> bool {
        self.variations.contains_key(&mineral_id)
    }

    /// Roll the two augment options for `mineral_id` on a weapon of the given
    /// magic-ness (Java `generateRandomVariation`): option from the `order=0`
    /// group and from the `order=1` group. `rng` yields `[0,1)` (drive it with
    /// `World::roll_f64`). `None` if the mineral is unknown or a group is
    /// missing.
    pub fn generate(
        &self,
        mineral_id: i32,
        is_magic_weapon: bool,
        rng: &mut dyn FnMut() -> f64,
    ) -> Option<(i32, i32)> {
        let variation = self.variations.get(&mineral_id)?;
        let kind = if is_magic_weapon {
            WeaponKind::Mage
        } else {
            WeaponKind::Warrior
        };
        let groups = variation.groups(kind);
        let o1 = groups[0].as_ref()?.random_effect(rng)?;
        let o2 = groups[1].as_ref()?.random_effect(rng)?;
        Some((o1, o2))
    }

    /// The make/cancel fee for augmenting `item_id` with `mineral_id`.
    pub fn fee(&self, item_id: i32, mineral_id: i32) -> Option<&VariationFee> {
        self.fees.get(&item_id)?.get(&mineral_id)
    }

    /// Java `hasFeeData`: whether `item_id` can be augmented at all, whichever
    /// mineral is used.
    pub fn has_fee_data(&self, item_id: i32) -> bool {
        self.fees.contains_key(&item_id)
    }

    /// Java `getCancelFee`: the adena cost to remove `item_id`'s augment. Falls
    /// back to any fee for the item when the exact mineral isn't listed;
    /// `None` when the item has no fee data at all.
    pub fn cancel_fee(&self, item_id: i32, mineral_id: i32) -> Option<i64> {
        let fees = self.fees.get(&item_id)?;
        fees.get(&mineral_id)
            .or_else(|| fees.values().next())
            .map(|f| f.cancel_fee)
    }

    // ---- parsing -------------------------------------------------------

    fn parse(&mut self, content: &str) {
        let mut reader = Reader::from_str(content);

        // variation-building state
        let mut cur_variation = Variation::default();
        let mut cur_mineral: Option<i32> = None;
        let mut cur_kind = WeaponKind::Warrior;
        let mut cur_order = 0usize;
        let mut cur_group = OptionGroup::default();
        let mut cur_category = OptionCategory::default();

        // itemGroups / fees state
        let mut item_groups: HashMap<i32, Vec<i32>> = HashMap::new();
        let mut cur_group_id: Option<i32> = None;
        let mut cur_items: Vec<i32> = Vec::new();
        let mut cur_fee: Option<(i32, VariationFee)> = None; // (itemGroup id, fee)
        let mut cur_fee_minerals: Vec<i32> = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                    b"variation" => cur_mineral = attr(&e, "mineralId").and_then(parse_i32),
                    b"optionGroup" => {
                        cur_kind = match attr(&e, "weaponType").as_deref() {
                            Some("mage") => WeaponKind::Mage,
                            _ => WeaponKind::Warrior,
                        };
                        cur_order = attr(&e, "order").and_then(parse_i32).unwrap_or(0) as usize;
                        cur_group = OptionGroup::default();
                    }
                    b"optionCategory" => {
                        cur_category = OptionCategory {
                            chance: attr(&e, "chance").and_then(parse_f64).unwrap_or(0.0),
                            options: Vec::new(),
                        };
                    }
                    b"option" => {
                        if let (Some(id), chance) = (
                            attr(&e, "id").and_then(parse_i32),
                            attr(&e, "chance").and_then(parse_f64).unwrap_or(0.0),
                        ) {
                            cur_category.options.push((id, chance));
                        }
                    }
                    b"optionRange" => {
                        if let (Some(from), Some(to), chance) = (
                            attr(&e, "from").and_then(parse_i32),
                            attr(&e, "to").and_then(parse_i32),
                            attr(&e, "chance").and_then(parse_f64).unwrap_or(0.0),
                        ) {
                            for id in from..=to {
                                cur_category.options.push((id, chance));
                            }
                        }
                    }
                    b"itemGroup" => {
                        cur_group_id = attr(&e, "id").and_then(parse_i32);
                        cur_items = Vec::new();
                    }
                    b"item" => {
                        if let Some(id) = attr(&e, "id").and_then(parse_i32) {
                            cur_items.push(id);
                        }
                    }
                    b"fee" => {
                        let group_id = attr(&e, "itemGroup").and_then(parse_i32).unwrap_or(0);
                        let fee = VariationFee {
                            item_id: attr(&e, "itemId").and_then(parse_i32).unwrap_or(0),
                            item_count: attr(&e, "itemCount").and_then(parse_i32).unwrap_or(0)
                                as i64,
                            cancel_fee: attr(&e, "cancelFee").and_then(parse_i32).unwrap_or(0)
                                as i64,
                        };
                        cur_fee = Some((group_id, fee));
                        cur_fee_minerals = Vec::new();
                    }
                    b"mineral" => {
                        if let Some(id) = attr(&e, "id").and_then(parse_i32) {
                            cur_fee_minerals.push(id);
                        }
                    }
                    b"mineralRange" => {
                        if let (Some(from), Some(to)) = (
                            attr(&e, "from").and_then(parse_i32),
                            attr(&e, "to").and_then(parse_i32),
                        ) {
                            cur_fee_minerals.extend(from..=to);
                        }
                    }
                    _ => {}
                },
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"optionCategory" => {
                        cur_group.categories.push(std::mem::take(&mut cur_category))
                    }
                    b"optionGroup" => {
                        let slot = cur_order.min(1);
                        let group = std::mem::take(&mut cur_group);
                        match cur_kind {
                            WeaponKind::Warrior => cur_variation.warrior[slot] = Some(group),
                            WeaponKind::Mage => cur_variation.mage[slot] = Some(group),
                        }
                    }
                    b"variation" => {
                        if let Some(mineral) = cur_mineral.take() {
                            self.variations
                                .insert(mineral, std::mem::take(&mut cur_variation));
                        }
                    }
                    b"itemGroup" => {
                        if let Some(id) = cur_group_id.take() {
                            item_groups.insert(id, std::mem::take(&mut cur_items));
                        }
                    }
                    b"fee" => {
                        if let Some((group_id, fee)) = cur_fee.take()
                            && let Some(items) = item_groups.get(&group_id)
                        {
                            for &item in items {
                                let entry = self.fees.entry(item).or_default();
                                for &mineral in &cur_fee_minerals {
                                    entry.insert(mineral, fee);
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

fn parse_i32(s: String) -> Option<i32> {
    s.trim().parse().ok()
}

fn parse_f64(s: String) -> Option<f64> {
    s.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    #[test]
    fn loads_variations_and_fees() {
        let d = VariationData::load_from(DIST);
        // The full dist has 211 warrior + 211 mage optionGroups across its
        // life stones; every `<variation>` must parse.
        assert!(
            d.variation_count() > 100,
            "loaded {} variations",
            d.variation_count()
        );
        // Lv.46 life stone (8723) is the first variation.
        assert!(d.has_variation(8723), "known life stone loaded");
        assert!(!d.has_variation(1), "non-mineral absent");
        // Gemstone-D fee for a whitelisted weapon (2551) + Lv.46 mineral (8723).
        let fee = d.fee(2551, 8723).expect("fee for augmented weapon 2551");
        assert_eq!(fee.item_id, 2130, "gemstone D");
        assert!(fee.item_count > 0);
        assert!(d.cancel_fee(2551, 8723).unwrap() > 0);
    }

    #[test]
    fn generate_produces_two_options_from_the_right_groups() {
        let d = VariationData::load_from(DIST);
        // A deterministic low roll always lands in the first category / option.
        let mut low = || 0.0_f64;
        let (o1, o2) = d.generate(8723, false, &mut low).expect("warrior augment");
        // order=0 warrior first optionRange starts at id 1; order=1 first
        // category's first optionRange starts at 7281 (per Variations.xml).
        assert_eq!(o1, 1, "order 0 first option");
        assert_eq!(o2, 7281, "order 1 first option");
        // Mage route rolls a different option pool (order 0 starts at 3641).
        let (m1, _m2) = d.generate(8723, true, &mut low).expect("mage augment");
        assert_eq!(m1, 3641, "mage order 0 first option");
    }

    #[test]
    fn generate_unknown_mineral_is_none() {
        let d = VariationData::load_from(DIST);
        let mut rng = || 0.5_f64;
        assert!(d.generate(1, false, &mut rng).is_none());
    }
}
