//! Saga of the Doomcryer (78) — Warcryer (52) → Doomcryer (116).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 78,
        name: "Q00078_SagaOfTheDoomcryer",
        html_dir: "quests/Q00078_SagaOfTheDoomcryer",
        npc: [
            31336, 31624, 31589, 31290, 31642, 31646, 31649, 31650, 31654, 31655, 31657, 31290,
        ],
        items: [
            7080, 7539, 7081, 7493, 7276, 7307, 7338, 7369, 7400, 7431, 7101, 0,
        ],
        mob: [27295, 27227, 27285],
        class_id: 116,
        prev_class: 52,
        spawn: [
            (191046, -40640, -3042),
            (46087, -36372, -1685),
            (46066, -36396, -1685),
        ],
    }
}
