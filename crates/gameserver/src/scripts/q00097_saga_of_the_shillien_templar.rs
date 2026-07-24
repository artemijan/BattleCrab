//! Saga of the Shillien Templar (97) — Shillien Knight (33) -> Shillien Templar (106).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 97,
        name: "Q00097_SagaOfTheShillienTemplar",
        html_dir: "quests/Q00097_SagaOfTheShillienTemplar",
        npc: [
            31580, 31623, 31285, 31285, 31610, 31646, 31648, 31652, 31654, 31655, 31659, 31285,
        ],
        items: [
            7080, 7526, 7081, 7512, 7295, 7326, 7357, 7388, 7419, 7450, 7091, 0,
        ],
        mob: [27271, 27246, 27273],
        class_id: 106,
        prev_class: 33,
        spawn: [
            (161719, -92823, -1893),
            (124355, 82155, -2803),
            (124376, 82127, -2796),
        ],
    }
}
