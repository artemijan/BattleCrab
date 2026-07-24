//! Saga of the Dreadnought (74) — Warlord (3) → Dreadnought (89).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 74,
        name: "Q00074_SagaOfTheDreadnought",
        html_dir: "quests/Q00074_SagaOfTheDreadnought",
        npc: [
            30850, 31624, 31298, 31276, 31595, 31646, 31648, 31650, 31654, 31655, 31657, 31522,
        ],
        items: [
            7080, 7538, 7081, 7489, 7272, 7303, 7334, 7365, 7396, 7427, 7097, 6480,
        ],
        mob: [27290, 27223, 27282],
        class_id: 89,
        prev_class: 3,
        spawn: [
            (191046, -40640, -3042),
            (46087, -36372, -1685),
            (46066, -36396, -1685),
        ],
    }
}
