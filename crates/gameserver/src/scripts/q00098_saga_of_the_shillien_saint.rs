//! Saga of the Shillien Saint (98) — Shillien Elder (43) -> Shillien Saint (112).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 98,
        name: "Q00098_SagaOfTheShillienSaint",
        html_dir: "quests/Q00098_SagaOfTheShillienSaint",
        npc: [
            31581, 31626, 31588, 31287, 31621, 31646, 31647, 31651, 31654, 31655, 31658, 31287,
        ],
        items: [
            7080, 7525, 7081, 7513, 7296, 7327, 7358, 7389, 7420, 7451, 7090, 0,
        ],
        mob: [27270, 27247, 27277],
        class_id: 112,
        prev_class: 43,
        spawn: [
            (119518, -28658, -3811),
            (181215, 36676, -4812),
            (181227, 36703, -4816),
        ],
    }
}
