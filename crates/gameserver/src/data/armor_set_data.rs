//! Port of `data/xml/ArmorSetData` (`data/stats/armorsets/*.xml`) — the set
//! bonuses a player gets for wearing matching armor pieces.
//!
//! A set is a list of `<requiredItems>` (the armor), an optional
//! `<optionalItems>` list (the shield), a `<skills>` list whose entries carry
//! their own `minimumPieces` / `minimumEnchant` / `optional` gates, and a
//! `<stats>` block of flat `BaseStat` bonuses.
//!
//! ## Reachability, since it decides the scope below
//!
//! 317 sets ship on this dist. **37** have every required piece obtainable
//! (drop / buylist / multisell / recipe / quest grant), and those 37 are the
//! ones this port has to be right about. Of them:
//!
//! - all **84** of their set skills are `operateType="P"` — **passive**, every
//!   one. Java's `applySkills` has a whole active-skill branch (equip reuse
//!   delay, `ARMOR_SET_EQUIP_ACTIVE_SKILL_REUSE`, the `SkillCoolTime` resend)
//!   which no reachable set can reach. SKIP(census) — noted at the grant site.
//! - **4** carry a `<stats>` block, and the swings are real: set 13 is STR +4 /
//!   CON −1, set 19 INT +4 / WIT −1.
//! - **0** come from `Visual_Sets.xml`. Visual sets key off an item's
//!   `visualId`/appearance stone, which is a post-Interlude mechanic with no
//!   items on this dist — SKIP(census), see [`ArmorSet::visual`].
//!
//! ## Why set skills ride the `SkillBook`
//!
//! Java grants them with `addSkill(skill, false)` — the `false` is *store*, so
//! they never reach `character_skills`. Here they go into the ordinary
//! [`SkillBook`](crate::model::components::skills::SkillBook) and are filtered out of
//! every flush by [`ArmorSetData::is_armor_set_skill`], exactly as transform
//! skills are. The payoff is that `conditioned_passive_buffs` already turns
//! any passive in the book into its stat modifiers, so a set's passives apply
//! through the machinery that was already there.
//!
//! That filter is only unambiguous because **no armor-set skill is learnable**:
//! of the 219 skill ids the sets grant and the 758 in every skill tree
//! combined, the intersection is **empty**. Nothing the filter drops could have
//! been earned. Re-check that before adding a set skill to a tree.

use std::collections::{HashMap, HashSet};

use quick_xml::Reader;
use quick_xml::events::Event;

use super::xml::{attr_f64, attr_i32, attr_str};
use tracing::info;

pub const ARMOR_SET_DIR: &str = "data/stats/armorsets";

/// One `<skill>` row of a set (Java `ArmorsetSkillHolder`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArmorSetSkill {
    pub skill_id: i32,
    pub level: i32,
    /// `minimumPieces`, **defaulting to the set's own** — Java
    /// `parseInteger(attrs, "minimumPieces", minimumPieces)`, where the
    /// fallback is the `<set>` attribute, not 0.
    pub minimum_pieces: i32,
    /// `minimumEnchant` — the set's *lowest* piece must be at least this.
    pub minimum_enchant: i32,
    /// `optional="true"` — also requires one of the set's `<optionalItems>`
    /// (the shield) to be equipped.
    pub optional: bool,
}

/// A `<set>` (Java `ArmorSet`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ArmorSet {
    pub id: i32,
    pub minimum_pieces: i32,
    /// `visual="true"`. Java matches these against an item's `visualId` behind
    /// an `AppearanceType.FIXED` stone. SKIP(census): no appearance stone or
    /// visual-id item exists on this dist, and no set in `Visual_Sets.xml` has
    /// obtainable pieces, so the visual branch is unreachable and unported —
    /// the flag is parsed only so such a set can be *excluded* from the
    /// by-item index rather than silently behaving like a real set.
    pub visual: bool,
    pub required_items: Vec<i32>,
    pub optional_items: Vec<i32>,
    pub skills: Vec<ArmorSetSkill>,
    /// `<stats>` — flat base-stat bonuses, keyed by the `type` attribute
    /// (`STR`/`DEX`/`CON`/`INT`/`WIT`/`MEN`). Java `getStatsBonus`, consumed by
    /// `BaseStatFinalizer`.
    pub stats: ArmorSetStats,
}

/// The six flat base-stat bonuses a set can carry, in the order the rest of
/// the port names them. Kept as a struct rather than a map so the summing at
/// the call site is total.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArmorSetStats {
    pub str_: f64,
    pub dex: f64,
    pub con: f64,
    pub int_: f64,
    pub wit: f64,
    pub men: f64,
}

impl ArmorSetStats {
    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }

    fn set(&mut self, key: &str, val: f64) {
        match key {
            "STR" => self.str_ = val,
            "DEX" => self.dex = val,
            "CON" => self.con = val,
            "INT" => self.int_ = val,
            "WIT" => self.wit = val,
            "MEN" => self.men = val,
            _ => {}
        }
    }
}

impl std::ops::AddAssign for ArmorSetStats {
    fn add_assign(&mut self, o: Self) {
        self.str_ += o.str_;
        self.dex += o.dex;
        self.con += o.con;
        self.int_ += o.int_;
        self.wit += o.wit;
        self.men += o.men;
    }
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArmorSetData {
    by_id: HashMap<i32, ArmorSet>,
    /// item id → the sets that item belongs to, required **or** optional
    /// (Java's `_itemSets`, built from `Stream.concat` of both lists).
    by_item: HashMap<i32, Vec<i32>>,
    /// Every skill id any set can grant — the persistence filter.
    skill_ids: HashSet<i32>,
}

impl ArmorSetData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_id: HashMap<i32, ArmorSet> = HashMap::new();
        {
            // Sorted by `xml_files_in`: Java's `putIfAbsent` makes the *first*
            // file to claim an id win, so readdir order must not decide it.
            for path in super::xml::xml_files_in(format!("{file_path}{ARMOR_SET_DIR}")) {
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for set in parse(&content) {
                    // Java logs and keeps the first on a duplicate id.
                    by_id.entry(set.id).or_insert(set);
                }
            }
        }
        let mut by_item: HashMap<i32, Vec<i32>> = HashMap::new();
        let mut skill_ids = HashSet::new();
        for set in by_id.values() {
            for &item in set.required_items.iter().chain(set.optional_items.iter()) {
                by_item.entry(item).or_default().push(set.id);
            }
            skill_ids.extend(set.skills.iter().map(|s| s.skill_id));
        }
        // `by_item` is iterated to build the granted-skill set; hash order
        // would make that traversal order unstable between runs, which shows up
        // as a flaky `SkillList`. Sort each bucket.
        for sets in by_item.values_mut() {
            sets.sort_unstable();
        }
        info!("ArmorSetData: Loaded {} armor sets.", by_id.len());
        Self {
            by_id,
            by_item,
            skill_ids,
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn get(&self, set_id: i32) -> Option<&ArmorSet> {
        self.by_id.get(&set_id)
    }

    /// Java `getSets(itemId)` — every set the item takes part in.
    pub fn sets_for_item(&self, item_id: i32) -> &[i32] {
        self.by_item.get(&item_id).map_or(&[], |v| v.as_slice())
    }

    /// Whether `id` is a skill some armor set grants. These live in the
    /// `SkillBook` only while the set is worn and must never reach
    /// `character_skills` — see the module header for why the test is
    /// unambiguous.
    pub fn is_armor_set_skill(&self, id: i32) -> bool {
        self.skill_ids.contains(&id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Which `<item>` list we are inside — the two are structurally identical, so
/// the enclosing element is the only thing that tells them apart.
#[derive(Clone, Copy, PartialEq)]
enum ItemList {
    None,
    Required,
    Optional,
}

/// Parse one armorsets file. A file holds many `<set>` elements.
fn parse(content: &str) -> Vec<ArmorSet> {
    let mut reader = Reader::from_str(content);
    let mut out = Vec::new();
    let mut cur: Option<ArmorSet> = None;
    let mut list = ItemList::None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"set" => {
                    let id = attr_i32(&e, b"id").unwrap_or(-1);
                    cur = Some(ArmorSet {
                        id,
                        minimum_pieces: attr_i32(&e, b"minimumPieces").unwrap_or(0),
                        visual: attr_str(&e, b"visual").as_deref() == Some("true"),
                        ..Default::default()
                    });
                    list = ItemList::None;
                }
                b"requiredItems" => list = ItemList::Required,
                b"optionalItems" => list = ItemList::Optional,
                b"item" => {
                    if let Some(set) = cur.as_mut()
                        && let Some(id) = attr_i32(&e, b"id")
                    {
                        // Java uses a `LinkedHashSet` and warns on a duplicate;
                        // keep insertion order and drop repeats.
                        let target = match list {
                            ItemList::Required => Some(&mut set.required_items),
                            ItemList::Optional => Some(&mut set.optional_items),
                            ItemList::None => None,
                        };
                        if let Some(v) = target
                            && !v.contains(&id)
                        {
                            v.push(id);
                        }
                    }
                }
                b"skill" => {
                    if let Some(set) = cur.as_mut()
                        && let (Some(skill_id), Some(level)) =
                            (attr_i32(&e, b"id"), attr_i32(&e, b"level"))
                    {
                        set.skills.push(ArmorSetSkill {
                            skill_id,
                            level,
                            minimum_pieces: attr_i32(&e, b"minimumPieces")
                                .unwrap_or(set.minimum_pieces),
                            minimum_enchant: attr_i32(&e, b"minimumEnchant").unwrap_or(0),
                            optional: attr_str(&e, b"optional").as_deref() == Some("true"),
                        });
                    }
                }
                b"stat" => {
                    if let Some(set) = cur.as_mut()
                        && let (Some(ty), Some(val)) = (attr_str(&e, b"type"), attr_f64(&e, b"val"))
                    {
                        set.stats.set(&ty, val);
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"set" => {
                    if let Some(set) = cur.take()
                        && set.id >= 0
                    {
                        out.push(set);
                    }
                }
                b"requiredItems" | b"optionalItems" => list = ItemList::None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two `<item>` lists are structurally identical — only the enclosing
    /// element separates a required piece from the optional shield. Mixing them
    /// up would make a shield count toward `minimumPieces`, which is exactly
    /// the sort of silent off-by-one this port keeps finding.
    #[test]
    fn required_and_optional_items_stay_apart() {
        let xml = r#"
        <list>
          <set id="7" minimumPieces="3">
            <requiredItems><item id="58" /><item id="59" /><item id="47" /></requiredItems>
            <optionalItems><item id="628" /></optionalItems>
            <skills>
              <skill id="3006" level="1" minimumPieces="3" />
              <skill id="3543" level="1" optional="true" />
              <skill id="3612" level="1" minimumEnchant="6" />
            </skills>
            <stats><stat type="STR" val="4" /><stat type="CON" val="-1" /></stats>
          </set>
        </list>"#;
        let sets = parse(xml);
        assert_eq!(sets.len(), 1);
        let s = &sets[0];
        assert_eq!(s.required_items, vec![58, 59, 47]);
        assert_eq!(s.optional_items, vec![628]);
        assert!(!s.visual);

        // `minimumPieces` on a skill falls back to the *set's* value, not 0 —
        // a 0 default would make every skill apply with one piece worn.
        let optional_skill = s.skills.iter().find(|k| k.skill_id == 3543).unwrap();
        assert_eq!(optional_skill.minimum_pieces, 3, "inherits the set's value");
        assert!(optional_skill.optional);
        assert_eq!(optional_skill.minimum_enchant, 0);

        let enchant_skill = s.skills.iter().find(|k| k.skill_id == 3612).unwrap();
        assert_eq!(enchant_skill.minimum_enchant, 6);
        assert!(!enchant_skill.optional);

        assert_eq!(s.stats.str_, 4.0);
        assert_eq!(s.stats.con, -1.0);
        assert_eq!(s.stats.dex, 0.0);
    }

    /// A file holds many sets; the parser must not leak state between them.
    #[test]
    fn multiple_sets_in_one_file_do_not_bleed() {
        let xml = r#"
        <list>
          <set id="1" minimumPieces="2">
            <requiredItems><item id="10" /><item id="11" /></requiredItems>
            <skills><skill id="100" level="1" /></skills>
          </set>
          <set id="2" minimumPieces="3" visual="true">
            <requiredItems><item id="20" /></requiredItems>
            <skills><skill id="200" level="2" /></skills>
          </set>
        </list>"#;
        let sets = parse(xml);
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].required_items, vec![10, 11]);
        assert!(sets[0].optional_items.is_empty());
        assert_eq!(sets[1].required_items, vec![20], "set 2 keeps only its own");
        assert!(sets[1].visual);
        assert_eq!(sets[1].skills[0].minimum_pieces, 3);
    }
}
