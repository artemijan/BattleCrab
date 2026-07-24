//! Saga of the Moonlight Sentinel (83) — Silver Ranger (24) → Moonlight Sentinel (102).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 83,
        name: "Q00083_SagaOfTheMoonlightSentinel",
        html_dir: "quests/Q00083_SagaOfTheMoonlightSentinel",
        npc: [
            30702, 31627, 31604, 31640, 31634, 31646, 31648, 31652, 31654, 31655, 31658, 31641,
        ],
        items: [
            7080, 7520, 7081, 7498, 7281, 7312, 7343, 7374, 7405, 7436, 7106, 0,
        ],
        mob: [27297, 27232, 27306],
        class_id: 102,
        prev_class: 24,
        spawn: [
            (161719, -92823, -1893),
            (181227, 36703, -4816),
            (181215, 36676, -4812),
        ],
    }
}
