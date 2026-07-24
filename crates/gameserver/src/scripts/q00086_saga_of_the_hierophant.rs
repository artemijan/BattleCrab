//! Saga of the Hierophant (86) — Prophet (17) → Hierophant (98).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 86,
        name: "Q00086_SagaOfTheHierophant",
        html_dir: "quests/Q00086_SagaOfTheHierophant",
        npc: [
            30191, 31626, 31588, 31280, 31591, 31646, 31648, 31652, 31654, 31655, 31659, 31280,
        ],
        items: [
            7080, 7523, 7081, 7501, 7284, 7315, 7346, 7377, 7408, 7439, 7089, 0,
        ],
        mob: [27269, 27235, 27275],
        class_id: 98,
        prev_class: 17,
        spawn: [
            (161719, -92823, -1893),
            (124355, 82155, -2803),
            (124376, 82127, -2796),
        ],
    }
}
