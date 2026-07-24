//! Saga of the Hell Knight (95) — Dark Avenger (6) -> Hell Knight (91).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 95,
        name: "Q00095_SagaOfTheHellKnight",
        html_dir: "quests/Q00095_SagaOfTheHellKnight",
        npc: [
            31582, 31623, 31297, 31297, 31599, 31646, 31647, 31653, 31654, 31655, 31656, 31297,
        ],
        items: [
            7080, 7532, 7081, 7510, 7293, 7324, 7355, 7386, 7417, 7448, 7086, 0,
        ],
        mob: [27258, 27244, 27263],
        class_id: 91,
        prev_class: 6,
        spawn: [
            (164650, -74121, -2871),
            (47391, -56929, -2370),
            (47429, -56923, -2383),
        ],
    }
}
