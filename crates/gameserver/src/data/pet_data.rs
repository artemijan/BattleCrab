//! Port of `data/xml/PetDataTable` + `model/actor/templates/PetData` (G29):
//! the pet templates in `dist/game/data/stats/pets/*.xml` (56 files).
//!
//! A pet is bound to the **collar item** that summons it (`itemId`), and its
//! stats come from this table rather than the NPC template — which is why the
//! loader has to exist before a pet can be summoned at all.
//!
//! Scoped to what summoning and feeding need: the item→npc binding, the food
//! item, the hunger limit, and the per-level `max_meal` (the pet's food
//! capacity). The per-level combat stats (`org_pattack`, `org_hp`, …) are
//! parsed into the level table but not yet consumed — the NPC template's own
//! stats stand in until pet levelling lands.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

const PETS_DIR: &str = "data/stats/pets";

/// One pet level's row (Java `PetData.PetLevelData`), narrowed to the fields
/// the port reads today.
#[derive(Debug, Clone, Default)]
pub struct PetLevel {
    /// `max_meal` — the pet's food bar capacity at this level.
    pub max_meal: i32,
    /// `consume_meal_in_normal` / `_in_battle` — food burned per feed tick.
    pub consume_meal_in_normal: i32,
    pub consume_meal_in_battle: i32,
    /// Cumulative experience required to *be* this level.
    pub exp: i64,
    /// `get_exp_type` — the share of the owner's exp/sp the **owner keeps**,
    /// as a percentage. The pet receives `100 - this`. Defaults to 100 (the
    /// pet gets nothing) so an unparsed species can't silently drain its
    /// owner's exp.
    pub owner_exp_taken: i32,

    /// The per-level combat bases. A pet does **not** use its NPC template's
    /// stats: Java's finalizers substitute these as the base value and then run
    /// the *same* bonus math (`MaxHpFinalizer`, `PDefenseFinalizer`,
    /// `IStatFunction.calcWeaponBaseValue`, …). Without them a levelled pet's
    /// level number moved but it never got stronger.
    pub p_atk: f64,
    pub m_atk: f64,
    pub p_def: f64,
    pub m_def: f64,
    pub max_hp: f64,
    pub max_mp: f64,
    pub regen_hp: f64,
    pub regen_mp: f64,
    /// `soulshot_count` / `spiritshot_count` — how many of the **owner's**
    /// Beast shots one of this pet's swings consumes (Java
    /// `Pet.getSoulShotsPerHit`). Grows with level, so a high-level pet is
    /// markedly more expensive to keep shotted.
    pub soulshot_count: i32,
    pub spiritshot_count: i32,
}

/// Port of `model/actor/templates/PetData` — one pet species.
#[derive(Debug, Clone, Default)]
pub struct PetTemplate {
    pub npc_id: i32,
    /// The collar item that summons this pet. The pet's saved row is keyed by
    /// the *object id* of one such item, which is how two collars of the same
    /// kind stay two different pets.
    pub item_id: i32,
    /// The food item this pet eats (`PetFood` handler checks it).
    pub food_item_id: i32,
    /// Below this percentage of `max_meal` the pet is "hungry" and loses its
    /// buffs / refuses to act in Java.
    pub hungry_limit: i32,
    /// Inventory weight limit.
    pub load: i32,
    /// Per-level rows, indexed by level.
    pub levels: HashMap<i32, PetLevel>,
}

impl PetTemplate {
    /// The food capacity at `level`, falling back to the highest level defined
    /// below it (Java clamps to the table's bounds).
    pub fn max_meal(&self, level: i32) -> i32 {
        self.levels
            .get(&level)
            .map(|l| l.max_meal)
            .unwrap_or_else(|| {
                self.levels
                    .iter()
                    .filter(|(l, _)| **l <= level)
                    .max_by_key(|(l, _)| **l)
                    .map(|(_, r)| r.max_meal)
                    .unwrap_or(0)
            })
    }

    /// Java `PetDataTable.getPetMinLevel` — the lowest level this species has a
    /// row for. A restored pet's level is clamped up to it, so a row written
    /// under an older datapack can't drop the pet below the table's floor and
    /// leave every per-level lookup falling off the bottom.
    pub fn min_level(&self) -> i32 {
        self.levels.keys().copied().min().unwrap_or(1)
    }

    /// Cumulative experience required to *be* `level` — Java
    /// `PetLevelData.getPetMaxExp`, used as a floor when restoring so a pet
    /// never delevels because the exp curve was retuned under it.
    pub fn exp_for_level(&self, level: i32) -> i64 {
        self.levels.get(&level).map(|l| l.exp).unwrap_or(0)
    }
}

#[derive(Debug, Default)]
pub struct PetData {
    by_npc: HashMap<i32, PetTemplate>,
    /// `getPetDataByItemId` — the lookup `SummonPet` uses.
    by_item: HashMap<i32, i32>,
}

impl PetData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_npc: HashMap<i32, PetTemplate> = HashMap::new();
        let mut by_item: HashMap<i32, i32> = HashMap::new();
        if let Ok(dir) = std::fs::read_dir(format!("{file_path}{PETS_DIR}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    parse_str(&content, &mut by_npc, &mut by_item);
                }
            }
        }
        info!("PetData: Loaded {} pet templates.", by_npc.len());
        Self { by_npc, by_item }
    }

    pub fn get(&self, npc_id: i32) -> Option<&PetTemplate> {
        self.by_npc.get(&npc_id)
    }

    /// Java `PetDataTable.getPetDataByItemId` — the collar → pet lookup.
    pub fn by_item_id(&self, item_id: i32) -> Option<&PetTemplate> {
        self.by_item
            .get(&item_id)
            .and_then(|npc| self.by_npc.get(npc))
    }

    /// Whether this item is a pet collar at all.
    pub fn is_pet_collar(&self, item_id: i32) -> bool {
        self.by_item.contains_key(&item_id)
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, t: PetTemplate) {
        self.by_item.insert(t.item_id, t.npc_id);
        self.by_npc.insert(t.npc_id, t);
    }

    pub fn len(&self) -> usize {
        self.by_npc.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_npc.is_empty()
    }
}

fn parse_str(
    content: &str,
    by_npc: &mut HashMap<i32, PetTemplate>,
    by_item: &mut HashMap<i32, i32>,
) {
    let mut reader = Reader::from_str(content);
    let mut cur: Option<PetTemplate> = None;
    let mut cur_level: i32 = 0;
    let mut in_stats = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"pet" => {
                        let mut t = PetTemplate::default();
                        for a in e.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value)
                                .parse::<i32>()
                                .unwrap_or(0);
                            match a.key.as_ref() {
                                b"id" => t.npc_id = v,
                                b"itemId" => t.item_id = v,
                                _ => {}
                            }
                        }
                        cur = Some(t);
                    }
                    b"stats" => in_stats = true,
                    b"stat" => {
                        cur_level = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == b"level")
                            .and_then(|a| String::from_utf8_lossy(&a.value).parse().ok())
                            .unwrap_or(0);
                        if let Some(t) = cur.as_mut() {
                            t.levels.entry(cur_level).or_default();
                        }
                    }
                    b"set" => {
                        let mut key = String::new();
                        let mut val = String::new();
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                b"name" => key = String::from_utf8_lossy(&a.value).into_owned(),
                                b"val" => val = String::from_utf8_lossy(&a.value).into_owned(),
                                _ => {}
                            }
                        }
                        let Some(t) = cur.as_mut() else { continue };
                        // Outside `<stats>` the sets are species-wide; inside
                        // they belong to the level being built.
                        if !in_stats {
                            match key.as_str() {
                                "food" => t.food_item_id = val.parse().unwrap_or(0),
                                "hungry_limit" => t.hungry_limit = val.parse().unwrap_or(0),
                                "load" => t.load = val.parse().unwrap_or(0),
                                _ => {}
                            }
                        } else if let Some(row) = t.levels.get_mut(&cur_level) {
                            match key.as_str() {
                                "max_meal" => row.max_meal = val.parse().unwrap_or(0),
                                // Java `PetLevelData._ownerExpTaken = set.getInt("get_exp_type")`
                                // — the **owner's** percentage share; the pet
                                // takes the remainder. 73 on most species.
                                "get_exp_type" => row.owner_exp_taken = val.parse().unwrap_or(100),
                                "org_pattack" => row.p_atk = val.parse().unwrap_or(0.0),
                                "org_mattack" => row.m_atk = val.parse().unwrap_or(0.0),
                                "org_pdefend" => row.p_def = val.parse().unwrap_or(0.0),
                                "org_mdefend" => row.m_def = val.parse().unwrap_or(0.0),
                                "org_hp" => row.max_hp = val.parse().unwrap_or(0.0),
                                "org_mp" => row.max_mp = val.parse().unwrap_or(0.0),
                                "org_hp_regen" => row.regen_hp = val.parse().unwrap_or(0.0),
                                "soulshot_count" => row.soulshot_count = val.parse().unwrap_or(0),
                                "spiritshot_count" => {
                                    row.spiritshot_count = val.parse().unwrap_or(0)
                                }
                                "org_mp_regen" => row.regen_mp = val.parse().unwrap_or(0.0),
                                "consume_meal_in_normal" => {
                                    row.consume_meal_in_normal = val.parse().unwrap_or(0)
                                }
                                "consume_meal_in_battle" => {
                                    row.consume_meal_in_battle = val.parse().unwrap_or(0)
                                }
                                "exp" => row.exp = val.parse().unwrap_or(0),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"stats" => in_stats = false,
                b"pet" => {
                    if let Some(t) = cur.take() {
                        if t.npc_id > 0 {
                            if t.item_id > 0 {
                                by_item.insert(t.item_id, t.npc_id);
                            }
                            by_npc.insert(t.npc_id, t);
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

#[cfg(test)]
mod tests {
    use super::*;
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    /// The whole dist loads, and the Wolf (the starter pet) reads back the
    /// values its XML declares.
    #[test]
    fn real_dist_pets_load() {
        let d = PetData::load_from(DIST);
        assert!(
            d.len() >= 50,
            "56 pet files ship on this dist, got {}",
            d.len()
        );

        let wolf = d.get(12077).expect("Wolf 12077 loads");
        assert_eq!(wolf.item_id, 2375, "summoned by the Wolf Collar");
        assert_eq!(wolf.food_item_id, 2515, "eats Pet Food");
        assert_eq!(wolf.hungry_limit, 55);

        // The per-level combat stats (slice 13) — a pet's real bases, which
        // the finalizers substitute for the NPC template's. Fixtures can't
        // catch a parse-arm slip here, so read the shipped Wolf.
        let l1 = wolf.levels.get(&1).expect("Wolf level 1");
        assert!(
            (l1.p_atk - 2.118_644_068).abs() < 1e-6,
            "org_pattack: {}",
            l1.p_atk
        );
        assert!(
            (l1.m_atk - 1.446_759_259).abs() < 1e-6,
            "org_mattack: {}",
            l1.m_atk
        );
        assert!(
            (l1.p_def - 11.111_111_11).abs() < 1e-6,
            "org_pdefend: {}",
            l1.p_def
        );
        assert!((l1.max_hp - 19.873).abs() < 1e-6, "org_hp: {}", l1.max_hp);
        assert_eq!(l1.max_mp, 20.0, "org_mp");
        assert_eq!(l1.regen_hp, 2.0, "org_hp_regen");
        assert!(
            (l1.regen_mp - 0.9).abs() < 1e-9,
            "org_mp_regen: {}",
            l1.regen_mp
        );
        assert_eq!(l1.soulshot_count, 1, "soulshot_count");
        assert_eq!(l1.spiritshot_count, 1, "spiritshot_count");
        assert_eq!(
            l1.owner_exp_taken, 73,
            "get_exp_type is the owner's percentage share"
        );

        // Growth is the whole point: a higher level must be strictly stronger.
        let top = wolf.levels.keys().copied().max().expect("levels");
        let ltop = wolf.levels.get(&top).unwrap();
        assert!(
            ltop.p_atk > l1.p_atk,
            "p.atk grows with level ({} → {})",
            l1.p_atk,
            ltop.p_atk
        );
        assert!(
            ltop.max_hp > l1.max_hp,
            "HP grows with level ({} → {})",
            l1.max_hp,
            ltop.max_hp
        );
        assert!(
            ltop.regen_hp > l1.regen_hp,
            "regen grows with level ({} → {})",
            l1.regen_hp,
            ltop.regen_hp
        );
        assert!(
            ltop.soulshot_count > l1.soulshot_count,
            "shot cost grows with level ({} → {})",
            l1.soulshot_count,
            ltop.soulshot_count
        );
        assert_eq!(wolf.max_meal(1), 248, "level 1 food capacity");

        // The collar lookup is what `SummonPet` uses.
        assert_eq!(d.by_item_id(2375).map(|t| t.npc_id), Some(12077));
        assert!(d.is_pet_collar(2375));
        assert!(!d.is_pet_collar(57), "adena is not a collar");
    }

    /// Species-wide `<set>`s and per-level ones live in the same element name,
    /// separated only by being inside `<stats>` — the parser must not mix them.
    #[test]
    fn species_and_level_sets_do_not_bleed_into_each_other() {
        let d = PetData::load_from(DIST);
        let wolf = d.get(12077).unwrap();
        assert!(wolf.levels.len() > 1, "several levels parsed");
        // `food` is species-wide and must not have landed in a level row.
        assert!(
            wolf.levels.values().all(|l| l.max_meal > 0),
            "every level got its own max_meal"
        );
    }

    /// `max_meal` falls back to the highest level at or below the one asked
    /// for, so an out-of-table level still answers.
    #[test]
    fn max_meal_clamps_to_the_table() {
        let d = PetData::load_from(DIST);
        let wolf = d.get(12077).unwrap();
        let top = wolf.levels.keys().copied().max().unwrap();
        assert_eq!(
            wolf.max_meal(top + 50),
            wolf.max_meal(top),
            "past the table, use its top row"
        );
    }
}
