//! Saga of the Cardinal (85) — Bishop (16) → Cardinal (97).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 85,
        name: "Q00085_SagaOfTheCardinal",
        html_dir: "quests/Q00085_SagaOfTheCardinal",
        npc: [
            30191, 31626, 31588, 31280, 31644, 31646, 31647, 31651, 31654, 31655, 31658, 31280,
        ],
        items: [
            7080, 7522, 7081, 7500, 7283, 7314, 7345, 7376, 7407, 7438, 7087, 0,
        ],
        mob: [27267, 27234, 27274],
        class_id: 97,
        prev_class: 16,
        spawn: [
            (119518, -28658, -3811),
            (181215, 36676, -4812),
            (181227, 36703, -4816),
        ],
    }
}
