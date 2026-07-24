//! Saga of the Archmage (88) — Sorcerer (12) → Archmage (94).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 88,
        name: "Q00088_SagaOfTheArchmage",
        html_dir: "quests/Q00088_SagaOfTheArchmage",
        npc: [
            30176, 31627, 31282, 31282, 31590, 31646, 31647, 31650, 31654, 31655, 31657, 31282,
        ],
        items: [
            7080, 7529, 7081, 7503, 7286, 7317, 7348, 7379, 7410, 7441, 7082, 0,
        ],
        mob: [27250, 27237, 27254],
        class_id: 94,
        prev_class: 12,
        spawn: [
            (191046, -40640, -3042),
            (46066, -36396, -1685),
            (46087, -36372, -1685),
        ],
    }
}
