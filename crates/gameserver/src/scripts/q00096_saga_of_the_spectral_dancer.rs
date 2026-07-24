//! Saga of the Spectral Dancer (96) — Bladedancer (34) -> Spectral Dancer (107).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 96,
        name: "Q00096_SagaOfTheSpectralDancer",
        html_dir: "quests/Q00096_SagaOfTheSpectralDancer",
        npc: [
            31582, 31623, 31284, 31284, 31611, 31646, 31649, 31653, 31654, 31655, 31656, 31284,
        ],
        items: [
            7080, 7527, 7081, 7511, 7294, 7325, 7356, 7387, 7418, 7449, 7092, 0,
        ],
        mob: [27272, 27245, 27264],
        class_id: 107,
        prev_class: 34,
        spawn: [
            (164650, -74121, -2871),
            (47429, -56923, -2383),
            (47391, -56929, -2370),
        ],
    }
}
