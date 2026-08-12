//! The Four Sepulchers wave-spawn table —
//! `data/scripts/ai/areas/ImperialTomb/FourSepulchers/FourSepulchers.xml`
//! (the datapack file the Java script's own `IXmlReader` loads): one row per
//! monster, keyed by sepulcher (1–4) and wave (1–7).

use crate::data::xml;

/// One `<spawn>` row.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct FsSpawn {
    pub sepulcher: i32,
    pub wave: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FourSepulchersData {
    pub spawns: Vec<FsSpawn>,
}

impl FourSepulchersData {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn load_from(base: &str) -> Self {
        let path =
            format!("{base}data/scripts/ai/areas/ImperialTomb/FourSepulchers/FourSepulchers.xml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            tracing::warn!("FourSepulchers: no spawn data at {path}");
            return Self::default();
        };
        let mut out = Self::default();
        for event in xml::events(&text) {
            match event {
                quick_xml::events::Event::Empty(e) | quick_xml::events::Event::Start(e)
                    if e.name().as_ref() == b"spawn" =>
                {
                    let attr = |key: &[u8]| super::xml::attr_i32(&e, key).unwrap_or(0);
                    out.spawns.push(FsSpawn {
                        sepulcher: attr(b"sepulcherId"),
                        wave: attr(b"wave"),
                        npc_id: attr(b"npcId"),
                        x: attr(b"x"),
                        y: attr(b"y"),
                        z: attr(b"z"),
                        heading: attr(b"heading"),
                    });
                }
                _ => {}
            }
        }
        tracing::info!("FourSepulchers: loaded {} spawn rows.", out.spawns.len());
        out
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, s: FsSpawn) {
        self.spawns.push(s);
    }
}
