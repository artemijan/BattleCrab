//! Saga of the Mystic Muse (89) — Spellsinger (27) → Mystic Muse (103).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 89,
        name: "Q00089_SagaOfTheMysticMuse",
        html_dir: "quests/Q00089_SagaOfTheMysticMuse",
        npc: [
            30174, 31627, 31283, 31283, 31643, 31646, 31648, 31651, 31654, 31655, 31658, 31283,
        ],
        items: [
            7080, 7530, 7081, 7504, 7287, 7318, 7349, 7380, 7411, 7442, 7083, 0,
        ],
        mob: [27251, 27238, 27255],
        class_id: 103,
        prev_class: 27,
        spawn: [
            (119518, -28658, -3811),
            (181227, 36703, -4816),
            (181215, 36676, -4812),
        ],
    }
}
