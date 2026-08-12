//! Port of `EnchantItemGroupsData` + `EnchantItemData` (`data/EnchantItemGroups.xml`,
//! `data/EnchantItemData.xml`) — the enchant **chance engine**.
//!
//! Given the item being enchanted, a scroll's group id, and the item's current
//! enchant level, this resolves the success chance the way Java's
//! `EnchantScroll.getChance` / `EnchantItemGroupsData.getItemGroup` do:
//!
//! 1. Pick the scroll group (`<enchantScrollGroup id=…>`) — this dist ships a
//!    single group `0`, the default enchant route.
//! 2. Walk the group's `<enchantRate>` rate-items in document order; the first
//!    whose `slot` mask intersects the item's `bodypart`, whose `magicWeapon`
//!    flag matches, and (if given) whose `itemId` set contains the item wins
//!    (Java `EnchantRateItem.validate` + `EnchantScrollGroup.getRateGroup`).
//! 3. That rate-item names an `<enchantRateGroup>` (`ARMOR_GROUP`,
//!    `FULL_ARMOR_GROUP`, `FIGHTER_WEAPON_GROUP`, `MAGE_WEAPON_GROUP`); its
//!    `<current enchant="min-max" chance=…>` ranges give the chance for the
//!    current enchant level (Java `EnchantItemGroup.getChance`).
//! 4. A scroll with `safeEnchant > current` returns 100 (`EnchantScroll.getChance`).
//!
//! `EnchantItemData.xml` supplies the special/branded scrolls (their
//! `targetGrade`, `maxEnchant`, `safeEnchant`, `bonusRate`, and optional
//! `<item>` whitelist). The standard grade scrolls (Enchant Weapon/Armor D–S)
//! are **not** listed there — they fall back to scroll group `0` with no safe
//! level and no bonus, i.e. pure `group.getChance(current)`.
//!
//! Scope note: this is the pure data + chance core. The client enchant flow
//! (the `RequestExAddEnchantScrollItem` / `RequestExTryToPutEnchant*` /
//! `RequestEnchantItem` Ex-packet handshake, the `EnchantItemRequest` state,
//! and the success/fail item mutation) is a separate layer built on top of it.

use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use tracing::info;

use super::item_data::{CrystalType, ItemTemplate, slot_mask};
use crate::data::xml;
use crate::data::xml::attr_strict as attr;

pub const GROUPS_FILE: &str = "data/EnchantItemGroups.xml";
pub const ITEMS_FILE: &str = "data/EnchantItemData.xml";

/// One `<current enchant="min-max" chance=…>` row (Java `RangeChanceHolder`).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct RangeChance {
    min: i32,
    max: i32,
    chance: f64,
}

/// An `<enchantRateGroup>` — a named ladder of per-enchant-level chances
/// (Java `EnchantItemGroup`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct EnchantItemGroup {
    chances: Vec<RangeChance>,
}

impl EnchantItemGroup {
    /// `EnchantItemGroup.getChance(index)`: the first range containing `index`,
    /// else the last range's chance, else `-1` when the group is empty.
    fn chance(&self, index: i32) -> f64 {
        if self.chances.is_empty() {
            return -1.0;
        }
        for h in &self.chances {
            if h.min <= index && index <= h.max {
                return h.chance;
            }
        }
        self.chances.last().map(|h| h.chance).unwrap_or(-1.0)
    }
}

/// One `<enchantRate group=…>` binding inside a scroll group (Java
/// `EnchantRateItem`): matches items by slot mask, magic-weapon flag, and/or an
/// explicit item-id whitelist, and points at a named rate group.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct EnchantRateItem {
    group_name: String,
    /// OR of every `<item slot=…>` mask; `0` means "no slot restriction".
    slot: i32,
    /// `<item magicWeapon=…>`; `None` means "don't care".
    magic_weapon: Option<bool>,
    /// `<item itemId=…>` whitelist; empty means "any item".
    item_ids: HashSet<i32>,
}

impl EnchantRateItem {
    /// Java `EnchantRateItem.validate`.
    fn validate(&self, template: &ItemTemplate, is_magic_weapon: bool) -> bool {
        if !self.item_ids.is_empty() && !self.item_ids.contains(&template.item_id) {
            return false;
        }
        if self.slot != 0 && (template.body_part & self.slot) == 0 {
            return false;
        }
        self.magic_weapon.is_none_or(|m| m == is_magic_weapon)
    }
}

/// An `<enchantScrollGroup id=…>` — an ordered list of rate-item bindings
/// (Java `EnchantScrollGroup`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct EnchantScrollGroup {
    rate_groups: Vec<EnchantRateItem>,
}

/// A branded scroll from `EnchantItemData.xml` (Java `EnchantScroll`), narrowed
/// to the chance-relevant fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnchantScroll {
    pub id: i32,
    /// `targetGrade` (Java `getGrade()`), matched against the item's
    /// `crystalTypePlus` in `isValid`. `None` grade = "any / ungraded".
    pub target_grade: CrystalType,
    /// `minEnchant` (0 = no minimum).
    pub min_enchant: i32,
    /// `maxEnchant` (Java default 127).
    pub max_enchant: i32,
    /// `safeEnchant` — below this the chance is forced to 100.
    pub safe_enchant: i32,
    /// `bonusRate` — flat percentage added to the resolved group chance.
    pub bonus_rate: f64,
    /// `scrollGroupId` (default 0).
    pub scroll_group_id: i32,
    /// `randomEnchantMin`/`Max` — the success **step**, rolled inclusively.
    /// Java `AbstractEnchantItem`: min defaults to 1 and max defaults to *min*,
    /// so a scroll with neither attribute is a plain `+1` and the 20 rows that
    /// do carry them are the only ones that behave differently.
    pub random_min: i32,
    pub random_max: i32,
    /// `<item id=…>` whitelist; empty = the scroll works on any matching item.
    pub item_ids: HashSet<i32>,
}

/// A support item from `EnchantItemData.xml` `<support>` (Java
/// `EnchantSupportItem`): raises the success chance and can widen the success
/// step. Weapon/blessed/giant classification comes from the item's
/// `EtcItemType` at use time (like scrolls).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct EnchantSupport {
    pub id: i32,
    pub target_grade: CrystalType,
    pub min_enchant: i32,
    pub max_enchant: i32,
    /// `bonusRate` — added to the scroll chance.
    pub bonus_rate: f64,
    /// `randomEnchantMin`/`Max` (default 1) — the success step this support
    /// grants instead of the scroll's.
    pub random_min: i32,
    pub random_max: i32,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnchantData {
    item_groups: HashMap<String, EnchantItemGroup>,
    scroll_groups: HashMap<i32, EnchantScrollGroup>,
    scrolls: HashMap<i32, EnchantScroll>,
    supports: HashMap<i32, EnchantSupport>,
}

impl EnchantData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{GROUPS_FILE}")) {
            data.parse_groups(&content);
        }
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{ITEMS_FILE}")) {
            data.parse_scrolls(&content);
        }
        info!(
            "EnchantData: Loaded {} rate groups, {} scroll groups, {} scrolls, {} supports.",
            data.item_groups.len(),
            data.scroll_groups.len(),
            data.scrolls.len(),
            data.supports.len()
        );
        data
    }

    pub fn empty() -> Self {
        Self::default()
    }

    /// A branded scroll's definition, if `item_id` is one.
    /// Every scroll id in the file — for censuses and range invariants.
    #[cfg(test)]
    pub fn scroll_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.scrolls.keys().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn scroll(&self, item_id: i32) -> Option<&EnchantScroll> {
        self.scrolls.get(&item_id)
    }

    /// A support item's definition, if `item_id` is one.
    pub fn support(&self, item_id: i32) -> Option<&EnchantSupport> {
        self.supports.get(&item_id)
    }

    /// Resolve the named rate group for an item under a scroll group
    /// (Java `EnchantItemGroupsData.getItemGroup`), then read its chance for
    /// `current_enchant`. Returns `None` when the scroll group is unknown or no
    /// rate-item matches (Java's two `null`/`-1` warning paths).
    fn group_chance(
        &self,
        template: &ItemTemplate,
        is_magic_weapon: bool,
        scroll_group_id: i32,
        current_enchant: i32,
    ) -> Option<f64> {
        let group = self.scroll_groups.get(&scroll_group_id)?;
        let rate = group
            .rate_groups
            .iter()
            .find(|r| r.validate(template, is_magic_weapon))?;
        let item_group = self.item_groups.get(&rate.group_name)?;
        Some(item_group.chance(current_enchant))
    }

    /// The base success chance (before player `ENCHANT_RATE` stat and support
    /// items), following `EnchantScroll.getChance`: `safeEnchant` short-circuit
    /// to 100, else the resolved rate-group chance. `safe_enchant`/`bonus_rate`
    /// come from the branded scroll ([`EnchantScroll`]) or are 0 for a standard
    /// grade scroll. Returns `-1.0` when the item can't be resolved (Java's
    /// error sentinel).
    pub fn base_chance(
        &self,
        template: &ItemTemplate,
        is_magic_weapon: bool,
        scroll_group_id: i32,
        current_enchant: i32,
        safe_enchant: i32,
        bonus_rate: f64,
    ) -> f64 {
        let Some(chance) =
            self.group_chance(template, is_magic_weapon, scroll_group_id, current_enchant)
        else {
            return -1.0;
        };
        if chance < 0.0 {
            return -1.0;
        }
        if safe_enchant > 0 && current_enchant < safe_enchant {
            return 100.0;
        }
        (chance + bonus_rate).min(100.0)
    }

    /// Whether `scroll` may enchant `target` at its current level, mirroring
    /// `EnchantScroll.isValid` + `AbstractEnchantItem.isValid` for the
    /// no-support case:
    ///
    /// - if the scroll has an item whitelist, the target must be in it;
    /// - otherwise the target must not be whitelisted by some *other* scroll;
    /// - the target must be enchantable and not at its enchant limit;
    /// - a weapon scroll only fits weapons, an armor scroll only fits
    ///   armor/accessories (`isValidItemType` on `type2`);
    /// - the target's current level must be within `[minEnchant, maxEnchant)`;
    /// - the scroll's `targetGrade` must equal the target's `crystalTypePlus`.
    ///
    /// `scroll_is_weapon` is the scroll template's `EtcItemType::is_enchant_weapon`.
    pub fn is_target_valid(
        &self,
        scroll: &EnchantScroll,
        scroll_is_weapon: bool,
        target: &ItemTemplate,
        target_enchant: i32,
    ) -> bool {
        if !scroll.item_ids.is_empty() {
            if !scroll.item_ids.contains(&target.item_id) {
                return false;
            }
        } else if self.scrolls.values().any(|s| {
            s.id != scroll.id && !s.item_ids.is_empty() && s.item_ids.contains(&target.item_id)
        }) {
            // An item claimed by a branded scroll's whitelist can't be
            // enchanted by a generic scroll.
            return false;
        }

        if !target.is_enchantable() {
            return false;
        }
        if target.enchant_limit != 0 && target_enchant == target.enchant_limit {
            return false;
        }

        // isValidItemType(type2): weapon scroll ↔ weapon; armor scroll ↔
        // armor/accessory.
        let type_ok = match target.type2 {
            super::item_data::TYPE2_WEAPON => scroll_is_weapon,
            super::item_data::TYPE2_SHIELD_ARMOR | super::item_data::TYPE2_ACCESSORY => {
                !scroll_is_weapon
            }
            _ => false,
        };
        if !type_ok {
            return false;
        }

        if (scroll.min_enchant != 0 && target_enchant < scroll.min_enchant)
            || (scroll.max_enchant != 0 && target_enchant >= scroll.max_enchant)
        {
            return false;
        }

        scroll.target_grade == target.crystal_type.plus()
    }

    /// Whether a support item is compatible with the scroll + target (Java
    /// `EnchantScroll.isValid`'s support branch + `AbstractEnchantItem.isValid`
    /// on the support). The `*_flags` tuples are `(is_weapon, is_blessed,
    /// is_giant)` from each item's `EtcItemType`; a standard scroll + a plain
    /// `INC_PROP` support reduces to matching weapon/armor and the grade/range.
    #[allow(clippy::too_many_arguments)]
    pub fn is_support_valid(
        &self,
        scroll_flags: (bool, bool, bool, bool), // (is_weapon, is_blessed, is_blessed_down, is_giant)
        support: &EnchantSupport,
        support_flags: (bool, bool, bool), // (is_weapon, is_blessed, is_giant)
        target: &ItemTemplate,
        target_enchant: i32,
    ) -> bool {
        let (scroll_weapon, scroll_blessed, scroll_blessed_down, scroll_giant) = scroll_flags;
        let (support_weapon, support_blessed, support_giant) = support_flags;
        // Blessed/giant flags must agree between scroll and support.
        if scroll_blessed != support_blessed
            || scroll_blessed_down != support_blessed
            || scroll_giant != support_giant
        {
            return false;
        }
        // A weapon support only fits a weapon scroll and vice-versa.
        if support_weapon != scroll_weapon {
            return false;
        }
        // AbstractEnchantItem.isValid(item) for the support itself.
        if !target.is_enchantable() {
            return false;
        }
        if target.enchant_limit != 0 && target_enchant == target.enchant_limit {
            return false;
        }
        let type_ok = match target.type2 {
            super::item_data::TYPE2_WEAPON => support_weapon,
            super::item_data::TYPE2_SHIELD_ARMOR | super::item_data::TYPE2_ACCESSORY => {
                !support_weapon
            }
            _ => false,
        };
        if !type_ok {
            return false;
        }
        if (support.min_enchant != 0 && target_enchant < support.min_enchant)
            || (support.max_enchant != 0 && target_enchant >= support.max_enchant)
        {
            return false;
        }
        support.target_grade == target.crystal_type.plus()
    }

    // ---- parsing -------------------------------------------------------

    fn parse_groups(&mut self, content: &str) {
        // Which top-level block we're inside, plus the accumulator for it.
        let mut cur_group_name: Option<String> = None;
        let mut cur_group = EnchantItemGroup::default();
        let mut cur_scroll_id: Option<i32> = None;
        let mut cur_scroll = EnchantScrollGroup::default();
        let mut cur_rate: Option<EnchantRateItem> = None;

        for event in xml::events(content) {
            match event {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    b"enchantRateGroup" => {
                        cur_group_name = attr(&e, "name");
                        cur_group = EnchantItemGroup::default();
                    }
                    b"current" => {
                        if let (Some(range), Some(chance)) = (
                            attr(&e, "enchant"),
                            attr(&e, "chance").and_then(|s| s.parse::<f64>().ok()),
                        ) && let Some((min, max)) = parse_range(&range)
                        {
                            cur_group.chances.push(RangeChance { min, max, chance });
                        }
                    }
                    b"enchantScrollGroup" => {
                        cur_scroll_id = attr(&e, "id").and_then(|s| s.parse().ok());
                        cur_scroll = EnchantScrollGroup::default();
                    }
                    b"enchantRate" => {
                        let mut r = EnchantRateItem::default();
                        r.group_name = attr(&e, "group").unwrap_or_default();
                        cur_rate = Some(r);
                    }
                    b"item" => {
                        if let Some(r) = cur_rate.as_mut() {
                            if let Some(slot) = attr(&e, "slot") {
                                r.slot |= slot_mask(&slot);
                            }
                            if let Some(m) = attr(&e, "magicWeapon") {
                                r.magic_weapon = Some(m == "true");
                            }
                            if let Some(id) = attr(&e, "itemId").and_then(|s| s.parse().ok()) {
                                r.item_ids.insert(id);
                            }
                        }
                    }
                    _ => {}
                },
                Event::End(e) => match e.name().as_ref() {
                    b"enchantRateGroup" => {
                        if let Some(name) = cur_group_name.take() {
                            self.item_groups
                                .insert(name, std::mem::take(&mut cur_group));
                        }
                    }
                    b"enchantRate" => {
                        if let Some(r) = cur_rate.take() {
                            cur_scroll.rate_groups.push(r);
                        }
                    }
                    b"enchantScrollGroup" => {
                        if let Some(id) = cur_scroll_id.take() {
                            self.scroll_groups
                                .insert(id, std::mem::take(&mut cur_scroll));
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn parse_scrolls(&mut self, content: &str) {
        let mut cur: Option<EnchantScroll> = None;

        for event in xml::events(content) {
            match event {
                // Self-closing `<enchant .../>` (no whitelist) — build and store
                // at once; the child-bearing form is the `Start`/`End` arms below.
                Event::Empty(e) if e.name().as_ref() == b"enchant" => {
                    if let Some(s) = build_scroll(&e) {
                        self.scrolls.insert(s.id, s);
                    }
                }
                Event::Start(e) if e.name().as_ref() == b"enchant" => {
                    cur = build_scroll(&e);
                }
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"item" => {
                    if let (Some(c), Some(id)) =
                        (cur.as_mut(), attr(&e, "id").and_then(|s| s.parse().ok()))
                    {
                        c.item_ids.insert(id);
                    }
                }
                Event::End(e) if e.name().as_ref() == b"enchant" => {
                    if let Some(c) = cur.take() {
                        self.scrolls.insert(c.id, c);
                    }
                }
                // `<support .../>` — always self-closing.
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"support" => {
                    if let Some(s) = build_support(&e) {
                        self.supports.insert(s.id, s);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Build an [`EnchantSupport`] from a `<support …>` element's attributes.
fn build_support(e: &quick_xml::events::BytesStart) -> Option<EnchantSupport> {
    let id = attr(e, "id").and_then(|s| s.parse().ok())?;
    let min = attr(e, "randomEnchantMin")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    Some(EnchantSupport {
        id,
        target_grade: CrystalType::from_name(attr(e, "targetGrade").as_deref()),
        min_enchant: attr(e, "minEnchant")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        max_enchant: attr(e, "maxEnchant")
            .and_then(|s| s.parse().ok())
            .unwrap_or(127),
        bonus_rate: attr(e, "bonusRate")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        random_min: min,
        random_max: attr(e, "randomEnchantMax")
            .and_then(|s| s.parse().ok())
            .unwrap_or(min),
    })
}

/// Build an [`EnchantScroll`] from an `<enchant …>` element's attributes
/// (shared by the self-closing and child-bearing forms).
fn build_scroll(e: &quick_xml::events::BytesStart) -> Option<EnchantScroll> {
    let id = attr(e, "id").and_then(|s| s.parse().ok())?;
    let min = attr(e, "randomEnchantMin")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    Some(EnchantScroll {
        id,
        target_grade: CrystalType::from_name(attr(e, "targetGrade").as_deref()),
        min_enchant: attr(e, "minEnchant")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        max_enchant: attr(e, "maxEnchant")
            .and_then(|s| s.parse().ok())
            .unwrap_or(127),
        safe_enchant: attr(e, "safeEnchant")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        bonus_rate: attr(e, "bonusRate")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        scroll_group_id: attr(e, "scrollGroupId")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        random_min: min,
        random_max: attr(e, "randomEnchantMax")
            .and_then(|s| s.parse().ok())
            .unwrap_or(min),
        item_ids: HashSet::new(),
    })
}

/// `"0-2"` → `(0, 2)`, `"30-65535"` → `(30, 65535)`, `"5"` → `(5, 5)`.
fn parse_range(range: &str) -> Option<(i32, i32)> {
    if let Some((a, b)) = range.split_once('-') {
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    } else {
        let n = range.trim().parse().ok()?;
        Some((n, n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::item_data::{
        ItemKind, SLOT_FULL_ARMOR, SLOT_HEAD, SLOT_R_HAND, TYPE2_SHIELD_ARMOR, TYPE2_WEAPON,
    };

    const DIST: &str = crate::data::DIST_GAME;

    /// Minimal template carrying only the fields the enchant resolver reads
    /// (item id, body part, type2, grade).
    fn template(item_id: i32, body_part: i32, type2: i32) -> ItemTemplate {
        ItemTemplate {
            trade_flags: Default::default(),
            time: -1,
            duration: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::ActionType::Other,
            item_id,
            name: String::new(),
            kind: if type2 == TYPE2_WEAPON {
                ItemKind::Weapon
            } else {
                ItemKind::Armor
            },
            crystal_type: CrystalType::D,
            crystal_count: 0,
            attack_radius: 40,
            attack_angle: 0,
            mp_consume: 0,
            reduced_mp_consume: 0,
            reduced_mp_consume_chance: 0,
            body_part,
            weight: 0,
            is_stackable: false,
            is_infinite: false,
            type1: 0,
            type2,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: Default::default(),
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
            etc_item_type: crate::data::item_data::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
        }
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn armor_group_chance_ladder() {
        let d = EnchantData::load_from(DIST);
        let helm = template(100, SLOT_HEAD, TYPE2_SHIELD_ARMOR);
        // ARMOR_GROUP: 0-2 => 100, 3-15 => 66.67, 16-19 => 33, 20-29 => 20, 30+ => 0.
        for (enchant, want) in [
            (0, 100.0),
            (2, 100.0),
            (3, 66.67),
            (15, 66.67),
            (16, 33.0),
            (20, 20.0),
            (30, 0.0),
        ] {
            let got = d.base_chance(&helm, false, 0, enchant, 0, 0.0);
            assert!(
                approx(got, want),
                "helm @+{enchant}: got {got}, want {want}"
            );
        }
    }

    #[test]
    fn full_armor_group_is_distinct_from_armor() {
        let d = EnchantData::load_from(DIST);
        // At +3 the two groups diverge: FULL_ARMOR still 100, ARMOR already 66.67.
        let full = template(101, SLOT_FULL_ARMOR, TYPE2_SHIELD_ARMOR);
        let chest = template(102, crate::data::item_data::SLOT_CHEST, TYPE2_SHIELD_ARMOR);
        assert!(approx(d.base_chance(&full, false, 0, 3, 0, 0.0), 100.0));
        assert!(approx(d.base_chance(&chest, false, 0, 3, 0, 0.0), 66.67));
    }

    #[test]
    fn weapon_group_chance() {
        let d = EnchantData::load_from(DIST);
        let sword = template(103, SLOT_R_HAND, TYPE2_WEAPON);
        // Fighter/mage weapon groups share the same ladder, so magic flag is moot.
        assert!(approx(d.base_chance(&sword, false, 0, 0, 0, 0.0), 100.0));
        assert!(approx(d.base_chance(&sword, true, 0, 0, 0, 0.0), 100.0));
        assert!(approx(d.base_chance(&sword, false, 0, 5, 0, 0.0), 66.67));
    }

    #[test]
    fn safe_enchant_forces_full_chance() {
        let d = EnchantData::load_from(DIST);
        let helm = template(104, SLOT_HEAD, TYPE2_SHIELD_ARMOR);
        // +16 would be 33%, but a safe level above the current forces 100.
        assert!(approx(d.base_chance(&helm, false, 0, 16, 20, 0.0), 100.0));
        // At/above the safe level the group chance applies again.
        assert!(approx(d.base_chance(&helm, false, 0, 20, 20, 0.0), 20.0));
    }

    #[test]
    fn bonus_rate_adds_and_caps_at_100() {
        let d = EnchantData::load_from(DIST);
        let helm = template(105, SLOT_HEAD, TYPE2_SHIELD_ARMOR);
        // +20 armor is 20%; a +15 bonus → 35, a +90 bonus caps at 100.
        assert!(approx(d.base_chance(&helm, false, 0, 20, 0, 15.0), 35.0));
        assert!(approx(d.base_chance(&helm, false, 0, 20, 0, 90.0), 100.0));
    }

    #[test]
    fn unknown_scroll_group_returns_error_sentinel() {
        let d = EnchantData::load_from(DIST);
        let helm = template(106, SLOT_HEAD, TYPE2_SHIELD_ARMOR);
        assert_eq!(d.base_chance(&helm, false, 999, 0, 0, 0.0), -1.0);
    }

    #[test]
    fn branded_scrolls_parsed_from_item_data() {
        let d = EnchantData::load_from(DIST);
        // Heavenly Enchant Scroll: Weapon — R grade, +12 cap, +100 bonus rate.
        let s = d.scroll(22427).expect("scroll 22427");
        assert_eq!(s.target_grade, CrystalType::R);
        assert_eq!(s.max_enchant, 12);
        assert!(approx(s.bonus_rate, 100.0));
        assert!(s.item_ids.is_empty());
        // A scroll with an item whitelist keeps its allowed targets.
        let circlet = d.scroll(48211).expect("scroll 48211");
        assert!(circlet.item_ids.contains(&48202));
        assert_eq!(circlet.max_enchant, 5);
    }
}

#[cfg(test)]
mod random_range_tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    /// The dist's own ranges, and the invariant every scroll must satisfy.
    ///
    /// Note what this canNOT show: `randomEnchantMax`'s default. Java's is
    /// `_randomEnchantMin`, but **every** row in this file that omits `max`
    /// also leaves `min` at its default of 1, so `unwrap_or(min)` and
    /// `unwrap_or(1)` are indistinguishable here — sabotaging the default to 1
    /// leaves this test green. [`max_defaults_to_min_not_to_one`] pins it
    /// against a synthetic element instead, which is the only way to see it.
    #[test]
    fn the_dist_random_ranges_are_sane() {
        let data = EnchantData::load_from(DIST);

        // No attributes at all — an ordinary Scroll: Enchant Weapon (D).
        let plain = data.scroll(955).expect("955 loaded");
        assert_eq!(
            (plain.random_min, plain.random_max),
            (1, 1),
            "a scroll with neither attribute is a flat +1"
        );

        // Both attributes — the Giant's Scroll a player can earn from Q375.
        let ranged = data.scroll(33808).expect("33808 loaded");
        assert_eq!((ranged.random_min, ranged.random_max), (1, 3));

        // A non-positive span would make the step roll's `max - min + 1` zero
        // or negative, which `World::roll` would clamp rather than reject.
        for id in data.scroll_ids() {
            let s = data.scroll(id).unwrap();
            assert!(
                s.random_min >= 1 && s.random_max >= s.random_min,
                "scroll {id} has range {}..{}",
                s.random_min,
                s.random_max
            );
        }
    }

    /// Java `AbstractEnchantItem`: `_randomEnchantMax = set.getInt(
    /// "randomEnchantMax", _randomEnchantMin)` — the fallback is **min**, not 1
    /// and not the 127 `maxEnchant` uses. No row on this dist can tell the two
    /// apart (see above), so this drives the builders directly: a scroll with
    /// `min=2` and no `max` must be a flat +2, and would silently become a
    /// 1..2 roll under the wrong default.
    #[test]
    fn max_defaults_to_min_not_to_one() {
        use quick_xml::events::BytesStart;

        let e = BytesStart::from_content(r#"enchant id="1" randomEnchantMin="2""#, "enchant".len());
        let s = build_scroll(&e).expect("builds");
        assert_eq!((s.random_min, s.random_max), (2, 2));

        // And the support side, which shares the rule.
        let e = BytesStart::from_content(r#"support id="2" randomEnchantMin="4""#, "support".len());
        let s = build_support(&e).expect("builds");
        assert_eq!((s.random_min, s.random_max), (4, 4));
    }
}
