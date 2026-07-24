//! Saga of the Ghost Sentinel (84) — Phantom Ranger (37) → Ghost Sentinel (109).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 84,
        name: "Q00084_SagaOfTheGhostSentinel",
        html_dir: "quests/Q00084_SagaOfTheGhostSentinel",
        npc: [
            30702, 31587, 31604, 31640, 31635, 31646, 31649, 31652, 31654, 31655, 31659, 31641,
        ],
        items: [
            7080, 7521, 7081, 7499, 7282, 7313, 7344, 7375, 7406, 7437, 7107, 0,
        ],
        mob: [27298, 27233, 27307],
        class_id: 109,
        prev_class: 37,
        spawn: [
            (161719, -92823, -1893),
            (124376, 82127, -2796),
            (124376, 82127, -2796),
        ],
    }
}
