//! Saga of the Grand Khavatari (76) — Tyrant (48) → Grand Khavatari (114).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 76,
        name: "Q00076_SagaOfTheGrandKhavatari",
        html_dir: "quests/Q00076_SagaOfTheGrandKhavatari",
        npc: [
            31339, 31624, 31589, 31290, 31637, 31646, 31647, 31652, 31654, 31655, 31659, 31290,
        ],
        items: [
            7080, 7539, 7081, 7491, 7274, 7305, 7336, 7367, 7398, 7429, 7099, 0,
        ],
        mob: [27293, 27226, 27284],
        class_id: 114,
        prev_class: 48,
        spawn: [
            (161719, -92823, -1893),
            (124355, 82155, -2803),
            (124376, 82127, -2796),
        ],
    }
}
