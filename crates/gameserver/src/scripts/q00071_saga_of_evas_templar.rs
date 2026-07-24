//! Saga of Eva's Templar (71) — Temple Knight (20) → Eva's Templar (99).
//! A [`SagaData`](super::saga::SagaData) table over the shared Saga engine.
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 71,
        name: "Q00071_SagaOfEvasTemplar",
        html_dir: "quests/Q00071_SagaOfEvasTemplar",
        npc: [
            30852, 31624, 31278, 30852, 31638, 31646, 31648, 31651, 31654, 31655, 31658, 31281,
        ],
        items: [
            7080, 7535, 7081, 7486, 7269, 7300, 7331, 7362, 7393, 7424, 7094, 6482,
        ],
        mob: [27287, 27220, 27279],
        class_id: 99,
        prev_class: 20,
        spawn: [
            (119518, -28658, -3811),
            (181215, 36676, -4812),
            (181227, 36703, -4816),
        ],
    }
}
