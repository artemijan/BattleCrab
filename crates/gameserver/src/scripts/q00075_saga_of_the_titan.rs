//! Saga of the Titan (75) — Destroyer (46) → Titan (113). Note items[11]==0
//! (no secondary hand-in item), exercising the engine's optional-item branch.
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 75,
        name: "Q00075_SagaOfTheTitan",
        html_dir: "quests/Q00075_SagaOfTheTitan",
        npc: [
            31327, 31624, 31289, 31290, 31607, 31646, 31649, 31651, 31654, 31655, 31658, 31290,
        ],
        items: [
            7080, 7539, 7081, 7490, 7273, 7304, 7335, 7366, 7397, 7428, 7098, 0,
        ],
        mob: [27292, 27224, 27283],
        class_id: 113,
        prev_class: 46,
        spawn: [
            (119518, -28658, -3811),
            (181215, 36676, -4812),
            (181227, 36703, -4816),
        ],
    }
}
