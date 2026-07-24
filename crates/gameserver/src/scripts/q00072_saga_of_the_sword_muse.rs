//! Saga of the Sword Muse (72) — Sword Singer (21) → Sword Muse (100).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 72,
        name: "Q00072_SagaOfTheSwordMuse",
        html_dir: "quests/Q00072_SagaOfTheSwordMuse",
        npc: [
            30853, 31624, 31583, 31537, 31618, 31646, 31649, 31652, 31654, 31655, 31659, 31281,
        ],
        items: [
            7080, 7536, 7081, 7487, 7270, 7301, 7332, 7363, 7394, 7425, 7095, 6482,
        ],
        mob: [27288, 27221, 27280],
        class_id: 100,
        prev_class: 21,
        spawn: [
            (161719, -92823, -1893),
            (124355, 82155, -2803),
            (124376, 82127, -2796),
        ],
    }
}
