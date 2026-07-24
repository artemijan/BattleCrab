//! Saga of the Wind Rider (80) — Plains Walker (23) → Wind Rider (101).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 80,
        name: "Q00080_SagaOfTheWindRider",
        html_dir: "quests/Q00080_SagaOfTheWindRider",
        npc: [
            31603, 31624, 31284, 31615, 31612, 31646, 31648, 31652, 31654, 31655, 31659, 31616,
        ],
        items: [
            7080, 7517, 7081, 7495, 7278, 7309, 7340, 7371, 7402, 7433, 7103, 0,
        ],
        mob: [27300, 27229, 27303],
        class_id: 101,
        prev_class: 23,
        spawn: [
            (161719, -92823, -1893),
            (124314, 82155, -2803),
            (124355, 82155, -2803),
        ],
    }
}
