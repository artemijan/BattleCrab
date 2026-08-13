//! Orc Subjugation (263) — port of
//! `dist/game/data/scripts/quests/Q00263_OrcSubjugation/`. Dark Elf only:
//! Kayleen buys Balor Orc amulets (8a) and necklaces (10a, +1100 for 10+
//! total); each registered monster drops its own item at 50%.

use super::orc_amulet_hunt::OrcAmuletHuntData;

const KAYLEEN: i32 = 30346;
const ORC_AMULET: i32 = 1116;
const ORC_NECKLACE: i32 = 1117;
/// monster id → dropped item.
const MONSTERS: [(i32, i32); 4] = [
    (20385, ORC_AMULET),
    (20386, ORC_NECKLACE),
    (20387, ORC_NECKLACE),
    (20388, ORC_NECKLACE),
];
const RACE_DARK_ELF: i32 = 2;

pub fn data() -> OrcAmuletHuntData {
    OrcAmuletHuntData {
        id: 263,
        name: "Q00263_OrcSubjugation",
        html_dir: "quests/Q00263_OrcSubjugation",
        npc: KAYLEEN,
        amulet: ORC_AMULET,
        necklace: ORC_NECKLACE,
        monsters: &MONSTERS,
        min_level: 8,
        race: RACE_DARK_ELF,
        amulet_price: 8,
        necklace_price: 10,
        bulk_bonus: 1100,
        wrong_race_page: "30346-01.htm",
        too_low_page: "30346-02.htm",
    }
}
