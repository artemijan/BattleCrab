//! Saga of the Arcana Lord (91) — Warlock (14) -> Arcana Lord (96).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 91,
        name: "Q00091_SagaOfTheArcanaLord",
        html_dir: "quests/Q00091_SagaOfTheArcanaLord",
        npc: [
            31605, 31622, 31585, 31608, 31586, 31646, 31647, 31651, 31654, 31655, 31658, 31608,
        ],
        items: [
            7080, 7604, 7081, 7506, 7289, 7320, 7351, 7382, 7413, 7444, 7110, 0,
        ],
        mob: [27313, 27240, 27310],
        class_id: 96,
        prev_class: 14,
        spawn: [
            (119518, -28658, -3811),
            (181215, 36676, -4812),
            (181227, 36703, -4816),
        ],
    }
}
