//! Saga of the Elemental Master (92) — Elemental Summoner (28) -> Elemental Master (104).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 92,
        name: "Q00092_SagaOfTheElementalMaster",
        html_dir: "quests/Q00092_SagaOfTheElementalMaster",
        npc: [
            30174, 31281, 31614, 31614, 31629, 31646, 31648, 31652, 31654, 31655, 31659, 31614,
        ],
        items: [
            7080, 7605, 7081, 7507, 7290, 7321, 7352, 7383, 7414, 7445, 7111, 0,
        ],
        mob: [27314, 27241, 27311],
        class_id: 104,
        prev_class: 28,
        spawn: [
            (161719, -92823, -1893),
            (124376, 82127, -2796),
            (124355, 82155, -2803),
        ],
    }
}
