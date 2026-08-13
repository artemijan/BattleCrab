//! The item catalog: id → name / type / icon, read from the datapack.
//!
//! `dist/game/data/stats/items/*.xml` is the game server's own item source, so
//! the dashboard reading the same files can never disagree with the game about
//! what item 12000 is called. Loaded once at boot and kept in memory — the
//! files change only on deploy.
//!
//! Parsing is line-oriented rather than a full XML walk, matching how these
//! generated files are actually shaped (`tools`' npc_xml takes the same view):
//! every `<item …>` opening tag sits on one line with `id`, `name` and `type`
//! as its first attributes, and the icon is a `<set name="icon" …/>` line
//! inside the element. A malformed file degrades to fewer catalog entries, not
//! a boot failure — the endpoint falls back to placeholder names.

use std::collections::HashMap;
use std::path::Path;

pub struct ItemDef {
    pub name: String,
    /// `Weapon`, `Armor` or `EtcItem` — the tag's own `type` attribute.
    pub kind: String,
    /// Icon reference as the grp files spell it (`icon.weapon_arcana_mace_i00`),
    /// lowercased to match the web's atlas map keys.
    pub icon: Option<String>,
    /// `is_questitem` — the client lists these on their own inventory tab,
    /// and the dashboard mirrors that split.
    pub quest: bool,
}

#[derive(Default)]
pub struct Catalog {
    items: HashMap<i32, ItemDef>,
}

impl Catalog {
    /// Reads every item XML under `<game_data_dir>/data/stats/items`.
    ///
    /// Missing directory (fresh checkout, tests, or the key set empty) is a
    /// warning and an empty catalog, never an error: the dashboard must come
    /// up even where the datapack is absent.
    pub fn load(game_data_dir: &str) -> Catalog {
        if game_data_dir.is_empty() {
            return Catalog::default();
        }
        let dir = Path::new(game_data_dir).join("data/stats/items");
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(
                    "item catalog disabled — {} is unreadable: {e}",
                    dir.display()
                );
                return Catalog::default();
            }
        };

        // `additionalName` is the soul-crystal / special-ability suffix
        // ("Arcana Mace" + "Acumen") and sits between `name` and `type` on
        // 14k+ items — optional here, joined into the display name below.
        let item_tag = regex::Regex::new(
            r#"<item id="(\d+)" name="([^"]*)"(?: additionalName="([^"]*)")? type="(\w+)""#,
        )
        .unwrap();
        let icon_set = regex::Regex::new(r#"<set name="icon" val="([^"]+)""#).unwrap();
        let quest_set = r#"<set name="is_questitem" val="true""#;

        let mut items = HashMap::new();
        let mut current: Option<i32> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "xml") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                tracing::warn!("item catalog: skipping unreadable {}", path.display());
                continue;
            };
            for line in text.lines() {
                if let Some(c) = item_tag.captures(line) {
                    let id: i32 = c[1].parse().unwrap_or(-1);
                    let name = match c.get(3) {
                        Some(extra) => {
                            format!("{} - {}", unescape(&c[2]), unescape(extra.as_str()))
                        }
                        None => unescape(&c[2]),
                    };
                    items.insert(
                        id,
                        ItemDef {
                            name,
                            kind: c[4].to_string(),
                            icon: None,
                            quest: false,
                        },
                    );
                    current = Some(id);
                } else if let Some(c) = icon_set.captures(line)
                    && let Some(def) = current.and_then(|id| items.get_mut(&id))
                {
                    // A property line belongs to the item tag most recently
                    // opened; files never interleave items.
                    def.icon = Some(c[1].to_lowercase());
                } else if line.contains(quest_set)
                    && let Some(def) = current.and_then(|id| items.get_mut(&id))
                {
                    def.quest = true;
                }
            }
            current = None;
        }

        tracing::info!("item catalog: {} items from {}", items.len(), dir.display());
        Catalog { items }
    }

    pub fn get(&self, id: i32) -> Option<&ItemDef> {
        self.items.get(&id)
    }
}

/// The five XML character entities. Item names carry `&amp;` and the odd
/// `&lt;`; anything fancier would be a datapack bug worth seeing raw.
fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against the real datapack, so the regexes are tested on the actual
    /// shape of the files rather than a hand-made fixture.
    #[test]
    fn reads_the_real_datapack() {
        let catalog = Catalog::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));

        let arcana = catalog.get(12000).expect("item 12000 exists");
        assert_eq!(arcana.name, "Common Item - Arcana Mace");
        assert_eq!(arcana.kind, "Weapon");
        assert_eq!(arcana.icon.as_deref(), Some("icon.weapon_arcana_mace_i00"));

        // Adena, the one row every database has.
        let adena = catalog.get(57).expect("item 57 exists");
        assert_eq!(adena.kind, "EtcItem");
        assert!(!adena.quest);

        // A quest item: `is_questitem` puts it on its own tab.
        let book = catalog.get(1001).expect("item 1001 exists");
        assert_eq!(book.name, "Book of Aklantoth - Part 4");
        assert!(book.quest);

        // A soul-crystal weapon: `additionalName` sits between `name` and
        // `type` in the tag, and skipping it silently dropped 14k+ items.
        let acumen = catalog.get(6608).expect("item 6608 exists");
        assert_eq!(acumen.name, "Arcana Mace - Acumen");
        assert_eq!(acumen.icon.as_deref(), Some("icon.weapon_arcana_mace_i01"));
    }

    #[test]
    fn a_missing_directory_yields_an_empty_catalog() {
        assert!(Catalog::load("/nonexistent").get(57).is_none());
        assert!(Catalog::load("").get(57).is_none());
    }
}
