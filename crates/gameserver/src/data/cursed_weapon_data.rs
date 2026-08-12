//! Port of `data/xml/CursedWeaponsManager`'s XML load (`data/CursedWeapons.xml`).
//! Produces the static [`CursedWeapon`] config list; the live wielder state is
//! overlaid at boot from the `cursed_weapons` table (see `game_loop/net.rs`).

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

use crate::data::xml::attr_strict as attr;
use crate::model::cursed_weapon::CursedWeapon;

const CURSED_WEAPONS_XML: &str = "data/CursedWeapons.xml";

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CursedWeaponData {
    /// The configured weapons, with default (inactive) runtime state.
    pub weapons: Vec<CursedWeapon>,
}

impl CursedWeaponData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let content =
            std::fs::read_to_string(format!("{file_path}{CURSED_WEAPONS_XML}")).unwrap_or_default();
        let weapons = parse(&content);
        info!("CursedWeaponData: Loaded {} cursed weapons.", weapons.len());
        Self { weapons }
    }

    pub fn empty() -> Self {
        Self {
            weapons: Vec::new(),
        }
    }
}

/// `val="..."` on a child element, parsed to i32.
fn val(e: &quick_xml::events::BytesStart) -> i32 {
    attr(e, "val").and_then(|s| s.parse().ok()).unwrap_or(0)
}

fn parse(content: &str) -> Vec<CursedWeapon> {
    let mut reader = Reader::from_str(content);
    let mut out = Vec::new();
    let mut cur: Option<CursedWeapon> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"item" => {
                    cur = Some(CursedWeapon {
                        item_id: attr(&e, "id").and_then(|s| s.parse().ok()).unwrap_or(0),
                        skill_id: attr(&e, "skillId")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0),
                        name: attr(&e, "name").unwrap_or_default(),
                        disappear_chance: 0,
                        drop_rate: 0,
                        duration: 0,
                        duration_lost: 0,
                        stage_kills: 0,
                        skill_max_level: 1,
                        is_activated: false,
                        is_dropped: false,
                        player_id: 0,
                        player_reputation: 0,
                        player_pk_kills: 0,
                        nb_kills: 0,
                        end_time: 0,
                        dropped_item_oid: 0,
                    });
                }
                tag => {
                    if let Some(cw) = cur.as_mut() {
                        match tag {
                            b"disapearChance" => cw.disappear_chance = val(&e),
                            b"dropRate" => cw.drop_rate = val(&e),
                            b"duration" => cw.duration = val(&e),
                            b"durationLost" => cw.duration_lost = val(&e),
                            b"stageKills" => cw.stage_kills = val(&e),
                            _ => {}
                        }
                    }
                }
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"item" => {
                if let Some(cw) = cur.take()
                    && cw.item_id != 0
                {
                    out.push(cw);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}
