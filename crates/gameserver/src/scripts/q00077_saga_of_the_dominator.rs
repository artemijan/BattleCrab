//! Saga of the Dominator (77) — Overlord (51) → Dominator (115).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 77,
        name: "Q00077_SagaOfTheDominator",
        html_dir: "quests/Q00077_SagaOfTheDominator",
        npc: [
            31336, 31624, 31371, 31290, 31636, 31646, 31648, 31653, 31654, 31655, 31656, 31290,
        ],
        items: [
            7080, 7539, 7081, 7492, 7275, 7306, 7337, 7368, 7399, 7430, 7100, 0,
        ],
        mob: [27294, 27226, 27262],
        class_id: 115,
        prev_class: 51,
        spawn: [
            (164650, -74121, -2871),
            (47429, -56923, -2383),
            (47391, -56929, -2370),
        ],
    }
}
