//! Saga of the Spectral Master (93) — Phantom Summoner (41) -> Spectral Master (111).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 93,
        name: "Q00093_SagaOfTheSpectralMaster",
        html_dir: "quests/Q00093_SagaOfTheSpectralMaster",
        npc: [
            30175, 31287, 31613, 30175, 31632, 31646, 31649, 31653, 31654, 31655, 31656, 31613,
        ],
        items: [
            7080, 7606, 7081, 7508, 7291, 7322, 7353, 7384, 7415, 7446, 7112, 0,
        ],
        mob: [27315, 27242, 27312],
        class_id: 111,
        prev_class: 41,
        spawn: [
            (164650, -74121, -2871),
            (47429, -56923, -2383),
            (47391, -56929, -2370),
        ],
    }
}
