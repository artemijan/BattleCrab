//! Saga of the Maestro (100) — Warsmith (57) -> Maestro (118).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 100,
        name: "Q00100_SagaOfTheMaestro",
        html_dir: "quests/Q00100_SagaOfTheMaestro",
        npc: [
            31592, 31273, 31597, 31597, 31596, 31646, 31648, 31653, 31654, 31655, 31656, 31597,
        ],
        items: [
            7080, 7607, 7081, 7515, 7298, 7329, 7360, 7391, 7422, 7453, 7108, 0,
        ],
        mob: [27260, 27249, 27308],
        class_id: 118,
        prev_class: 57,
        spawn: [
            (164650, -74121, -2871),
            (47429, -56923, -2383),
            (47391, -56929, -2370),
        ],
    }
}
