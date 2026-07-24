//! Saga of the Duelist (73) — Gladiator (2) → Duelist (88).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 73,
        name: "Q00073_SagaOfTheDuelist",
        html_dir: "quests/Q00073_SagaOfTheDuelist",
        npc: [
            30849, 31624, 31226, 31331, 31639, 31646, 31647, 31653, 31654, 31655, 31656, 31277,
        ],
        items: [
            7080, 7537, 7081, 7488, 7271, 7302, 7333, 7364, 7395, 7426, 7096, 7546,
        ],
        mob: [27289, 27222, 27281],
        class_id: 88,
        prev_class: 2,
        spawn: [
            (164650, -74121, -2871),
            (47429, -56923, -2383),
            (47391, -56929, -2370),
        ],
    }
}
