//! Orc Hunting (260) — port of
//! `dist/game/data/scripts/quests/Q00260_OrcHunting/`. Elf only: Rayen
//! buys Kaboo Orc amulets (4a) and necklaces (10a, +1000 for 10+ total);
//! each monster drops its own item at 50%.

use super::orc_amulet_hunt::OrcAmuletHuntData;

const RAYEN: i32 = 30221;
const ORC_AMULET: i32 = 1114;
const ORC_NECKLACE: i32 = 1115;
/// monster id → dropped item.
const MONSTERS: [(i32, i32); 6] = [
    (20468, ORC_AMULET),
    (20469, ORC_AMULET),
    (20470, ORC_AMULET),
    (20471, ORC_NECKLACE),
    (20472, ORC_NECKLACE),
    (20473, ORC_NECKLACE),
];
const RACE_ELF: i32 = 1;

pub fn data() -> OrcAmuletHuntData {
    OrcAmuletHuntData {
        id: 260,
        name: "Q00260_OrcHunting",
        html_dir: "quests/Q00260_OrcHunting",
        npc: RAYEN,
        amulet: ORC_AMULET,
        necklace: ORC_NECKLACE,
        monsters: &MONSTERS,
        min_level: 6,
        race: RACE_ELF,
        amulet_price: 4,
        necklace_price: 10,
        bulk_bonus: 1000,
        wrong_race_page: "30221-01.html",
        too_low_page: "30221-02.html",
    }
}
