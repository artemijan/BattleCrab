//! Saga of the Adventurer (79) — Treasure Hunter (8) → Adventurer (93).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 79,
        name: "Q00079_SagaOfTheAdventurer",
        html_dir: "quests/Q00079_SagaOfTheAdventurer",
        npc: [
            31603, 31584, 31579, 31615, 31619, 31646, 31647, 31651, 31654, 31655, 31658, 31616,
        ],
        items: [
            7080, 7516, 7081, 7494, 7277, 7308, 7339, 7370, 7401, 7432, 7102, 0,
        ],
        mob: [27299, 27228, 27302],
        class_id: 93,
        prev_class: 8,
        spawn: [
            (119518, -28658, -3811),
            (181205, 36676, -4816),
            (181215, 36676, -4812),
        ],
    }
}
