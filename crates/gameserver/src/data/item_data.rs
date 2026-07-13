//! Port of `data/xml/ItemData` + `util/DocumentItem`, scoped to what G5 needs:
//! identity, equip slot, weight, stackability, and the `type1`/`type2` pair the
//! client needs for `ItemList` sorting/icons. The combat-stat bonuses under
//! `<stats>` (and `<skills>`/`<cond>`/`<capsuled_items>`) feed the later stats
//! engine milestone and are not parsed here.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const ITEMS_DIR: &str = "data/stats/items";

/// `Inventory.ADENA_ID` / `ANCIENT_ADENA_ID`.
pub const ADENA_ID: i32 = 57;
const ANCIENT_ADENA_ID: i32 = 5575;

// `ItemTemplate.SLOT_*` (bodypart bitmask constants).
pub const SLOT_NONE: i32 = 0x0000;
pub const SLOT_UNDERWEAR: i32 = 0x0001;
pub const SLOT_R_EAR: i32 = 0x0002;
pub const SLOT_L_EAR: i32 = 0x0004;
/// Combined `rear;lear` bodypart value (both bits set) — the dual-slot ear
/// items actually carry this, not the single `SLOT_*_EAR` bits.
pub const SLOT_LR_EAR: i32 = 0x0006;
pub const SLOT_NECK: i32 = 0x0008;
pub const SLOT_R_FINGER: i32 = 0x0010;
pub const SLOT_L_FINGER: i32 = 0x0020;
/// Combined `rfinger;lfinger` bodypart value, see `SLOT_LR_EAR`.
pub const SLOT_LR_FINGER: i32 = 0x0030;
pub const SLOT_HEAD: i32 = 0x0040;
pub const SLOT_R_HAND: i32 = 0x0080;
pub const SLOT_L_HAND: i32 = 0x0100;
pub const SLOT_GLOVES: i32 = 0x0200;
pub const SLOT_CHEST: i32 = 0x0400;
pub const SLOT_LEGS: i32 = 0x0800;
pub const SLOT_FEET: i32 = 0x1000;
pub const SLOT_BACK: i32 = 0x2000;
pub const SLOT_LR_HAND: i32 = 0x4000;
pub const SLOT_FULL_ARMOR: i32 = 0x8000;
pub const SLOT_HAIR: i32 = 0x010000;
pub const SLOT_ALLDRESS: i32 = 0x020000;
pub const SLOT_HAIR2: i32 = 0x040000;
pub const SLOT_HAIRALL: i32 = 0x080000;
pub const SLOT_R_BRACELET: i32 = 0x100000;
pub const SLOT_L_BRACELET: i32 = 0x200000;
pub const SLOT_DECO: i32 = 0x400000;
pub const SLOT_BELT: i32 = 0x1000_0000_u32 as i32;
pub const SLOT_BROOCH: i32 = 0x2000_0000_u32 as i32;
pub const SLOT_BROOCH_JEWEL: i32 = 0x4000_0000_u32 as i32;
pub const SLOT_WOLF: i32 = -100;
pub const SLOT_HATCHLING: i32 = -101;
pub const SLOT_STRIDER: i32 = -102;
pub const SLOT_BABYPET: i32 = -103;
pub const SLOT_GREATWOLF: i32 = -104;

// `ItemTemplate.TYPE1_*` / `TYPE2_*`.
pub const TYPE1_WEAPON_RING_EARRING_NECKLACE: i32 = 0;
pub const TYPE1_SHIELD_ARMOR: i32 = 1;
pub const TYPE1_ITEM_QUESTITEM_ADENA: i32 = 4;
pub const TYPE2_WEAPON: i32 = 0;
pub const TYPE2_SHIELD_ARMOR: i32 = 1;
pub const TYPE2_ACCESSORY: i32 = 2;
pub const TYPE2_QUEST: i32 = 3;
pub const TYPE2_MONEY: i32 = 4;
pub const TYPE2_OTHER: i32 = 5;

/// `ItemData.SLOTS` — the `bodypart` XML attribute string → slot bitmask table.
fn body_part(name: &str) -> i32 {
    match name {
        "shirt" | "underwear" => SLOT_UNDERWEAR,
        "lbracelet" => SLOT_L_BRACELET,
        "rbracelet" => SLOT_R_BRACELET,
        "talisman" | "deco1" => SLOT_DECO,
        "chest" => SLOT_CHEST,
        "fullarmor" | "onepiece" => SLOT_FULL_ARMOR,
        "head" => SLOT_HEAD,
        "hair" => SLOT_HAIR,
        "hairall" | "dhair" => SLOT_HAIRALL,
        "back" => SLOT_BACK,
        "neck" => SLOT_NECK,
        "legs" => SLOT_LEGS,
        "feet" => SLOT_FEET,
        "gloves" => SLOT_GLOVES,
        "chest,legs" => SLOT_CHEST | SLOT_LEGS,
        "belt" | "waist" => SLOT_BELT,
        "rhand" => SLOT_R_HAND,
        "lhand" => SLOT_L_HAND,
        "lrhand" => SLOT_LR_HAND,
        "rear;lear" => SLOT_R_EAR | SLOT_L_EAR,
        "rfinger;lfinger" => SLOT_R_FINGER | SLOT_L_FINGER,
        "wolf" => SLOT_WOLF,
        "greatwolf" => SLOT_GREATWOLF,
        "hatchling" => SLOT_HATCHLING,
        "strider" => SLOT_STRIDER,
        "babypet" => SLOT_BABYPET,
        "brooch" => SLOT_BROOCH,
        "brooch_jewel" => SLOT_BROOCH_JEWEL,
        "hair2" => SLOT_HAIR2,
        "alldress" => SLOT_ALLDRESS,
        _ => SLOT_NONE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Weapon,
    Armor,
    Etc,
}

#[derive(Debug, Clone)]
pub struct ItemTemplate {
    pub item_id: i32,
    pub name: String,
    pub kind: ItemKind,
    pub body_part: i32,
    pub weight: i32,
    pub is_stackable: bool,
    pub type1: i32,
    pub type2: i32,
    pub is_quest_item: bool,
    /// `<set name="price">` — the reference price (sell value = half of it;
    /// the `CorrectPrices` buylist floor uses it too). 0 when undeclared.
    pub price: i64,
}

impl ItemTemplate {
    /// `ItemTemplate.isEquipable`: has a body part and isn't an `EtcItem`.
    pub fn is_equipable(&self) -> bool {
        self.kind != ItemKind::Etc && self.body_part != SLOT_NONE
    }
}

pub struct ItemData {
    by_id: HashMap<i32, ItemTemplate>,
}

impl ItemData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_id = HashMap::new();
        let dir = format!("{file_path}{ITEMS_DIR}");
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut paths: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("xml"))
                .collect();
            paths.sort();
            for path in paths {
                parse_file(&path, &mut by_id);
            }
        }
        info!("ItemData: Loaded {} item templates.", by_id.len());
        Self { by_id }
    }

    pub fn get(&self, item_id: i32) -> Option<&ItemTemplate> {
        self.by_id.get(&item_id)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { by_id: HashMap::new() }
    }

    /// Synthetic catalog for unit tests that need specific templates without
    /// reading `dist/game` XML.
    #[doc(hidden)]
    pub fn from_templates(templates: Vec<ItemTemplate>) -> Self {
        Self { by_id: templates.into_iter().map(|t| (t.item_id, t)).collect() }
    }

    /// Register one synthetic template (same hook as `NpcData`'s).
    #[doc(hidden)]
    pub fn insert_for_test(&mut self, t: ItemTemplate) {
        self.by_id.insert(t.item_id, t);
    }
}

fn parse_file(path: &std::path::Path, out: &mut HashMap<i32, ItemTemplate>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);

    let mut cur_id: Option<i32> = None;
    let mut cur_name = String::new();
    let mut cur_kind = ItemKind::Etc;
    let mut attrs: HashMap<String, String> = HashMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"item" => {
                cur_id = attr_i32(&e, b"id");
                cur_name = attr_str(&e, b"name").unwrap_or_default();
                cur_kind = match attr_str(&e, b"type").as_deref() {
                    Some("Weapon") => ItemKind::Weapon,
                    Some("Armor") => ItemKind::Armor,
                    _ => ItemKind::Etc,
                };
                attrs.clear();
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"set" => {
                if cur_id.is_none() {
                    continue;
                }
                if let (Some(name), Some(val)) = (attr_str(&e, b"name"), attr_str(&e, b"val")) {
                    attrs.insert(name, val);
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"item" => {
                if let Some(item_id) = cur_id.take() {
                    out.insert(item_id, make_template(item_id, std::mem::take(&mut cur_name), cur_kind, &attrs));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

fn make_template(item_id: i32, name: String, kind: ItemKind, attrs: &HashMap<String, String>) -> ItemTemplate {
    let weight = attrs.get("weight").and_then(|v| v.parse().ok()).unwrap_or(0);
    let is_stackable = attrs.get("is_stackable").map(|v| v == "true").unwrap_or(false);
    let is_quest_item = attrs.get("is_questitem").map(|v| v == "true").unwrap_or(false);
    let part = body_part(attrs.get("bodypart").map(|s| s.as_str()).unwrap_or("none"));

    let (type1, type2) = match kind {
        ItemKind::Weapon => (TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_WEAPON),
        ItemKind::Armor => {
            if part == SLOT_NECK || (part & SLOT_L_EAR) != 0 || (part & SLOT_L_FINGER) != 0 || (part & SLOT_R_BRACELET) != 0 || (part & SLOT_L_BRACELET) != 0 {
                (TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_ACCESSORY)
            } else {
                (TYPE1_SHIELD_ARMOR, TYPE2_SHIELD_ARMOR)
            }
        }
        ItemKind::Etc => {
            let type2 = if is_quest_item {
                TYPE2_QUEST
            } else if item_id == ADENA_ID || item_id == ANCIENT_ADENA_ID {
                TYPE2_MONEY
            } else {
                TYPE2_OTHER
            };
            (TYPE1_ITEM_QUESTITEM_ADENA, type2)
        }
    };

    ItemTemplate {
        item_id,
        name,
        kind,
        body_part: part,
        weight,
        is_stackable,
        type1,
        type2,
        is_quest_item,
        price: attrs.get("price").and_then(|v| v.parse().ok()).unwrap_or(0),
    }
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_short_sword_and_adena() {
        let data = ItemData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        let sword = data.get(1).expect("item 1 (Short Sword)");
        assert_eq!(sword.name, "Short Sword");
        assert_eq!(sword.kind, ItemKind::Weapon);
        assert_eq!(sword.body_part, SLOT_R_HAND);
        assert!(sword.is_equipable());

        let adena = data.get(ADENA_ID).expect("adena");
        assert!(adena.is_stackable);
        assert_eq!(adena.type2, TYPE2_MONEY);
        assert!(!adena.is_equipable());

        assert!(data.by_id.len() > 5000);
    }
}
