//! Saga of the Sagittarius (82) — Hawkeye (9) → Sagittarius (92).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 82,
        name: "Q00082_SagaOfTheSagittarius",
        html_dir: "quests/Q00082_SagaOfTheSagittarius",
        npc: [
            30702, 31627, 31604, 31640, 31633, 31646, 31647, 31650, 31654, 31655, 31657, 31641,
        ],
        items: [
            7080, 7519, 7081, 7497, 7280, 7311, 7342, 7373, 7404, 7435, 7105, 0,
        ],
        mob: [27296, 27231, 27305],
        class_id: 92,
        prev_class: 9,
        spawn: [
            (191046, -40640, -3042),
            (46066, -36396, -1685),
            (46066, -36396, -1685),
        ],
    }
}
