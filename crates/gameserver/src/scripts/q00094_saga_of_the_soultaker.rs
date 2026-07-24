//! Saga of the Soultaker (94) — Necromancer (13) -> Soultaker (95).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 94,
        name: "Q00094_SagaOfTheSoultaker",
        html_dir: "quests/Q00094_SagaOfTheSoultaker",
        npc: [
            30832, 31623, 31279, 31279, 31645, 31646, 31648, 31650, 31654, 31655, 31657, 31279,
        ],
        items: [
            7080, 7533, 7081, 7509, 7292, 7323, 7354, 7385, 7416, 7447, 7085, 0,
        ],
        mob: [27257, 27243, 27265],
        class_id: 95,
        prev_class: 13,
        spawn: [
            (191046, -40640, -3042),
            (46066, -36396, -1685),
            (46087, -36372, -1685),
        ],
    }
}
