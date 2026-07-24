//! Saga of the Fortune Seeker (99) — Bounty Hunter (55) -> Fortune Seeker (117).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 99,
        name: "Q00099_SagaOfTheFortuneSeeker",
        html_dir: "quests/Q00099_SagaOfTheFortuneSeeker",
        npc: [
            31594, 31623, 31600, 31600, 31601, 31646, 31649, 31650, 31654, 31655, 31657, 31600,
        ],
        items: [
            7080, 7608, 7081, 7514, 7297, 7328, 7359, 7390, 7421, 7452, 7109, 0,
        ],
        mob: [27259, 27248, 27309],
        class_id: 117,
        prev_class: 55,
        spawn: [
            (191046, -40640, -3042),
            (46066, -36396, -1685),
            (46087, -36372, -1685),
        ],
    }
}
