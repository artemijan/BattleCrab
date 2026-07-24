//! Saga of the Storm Screamer (90) — Spellhowler (40) → Storm Screamer (110).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 90,
        name: "Q00090_SagaOfTheStormScreamer",
        html_dir: "quests/Q00090_SagaOfTheStormScreamer",
        npc: [
            30175, 31627, 31287, 31287, 31598, 31646, 31649, 31652, 31654, 31655, 31659, 31287,
        ],
        items: [
            7080, 7531, 7081, 7505, 7288, 7319, 7350, 7381, 7412, 7443, 7084, 0,
        ],
        mob: [27252, 27239, 27256],
        class_id: 110,
        prev_class: 40,
        spawn: [
            (161719, -92823, -1893),
            (124376, 82127, -2796),
            (124355, 82155, -2803),
        ],
    }
}
