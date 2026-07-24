//! Saga of the Ghost Hunter (81) — Abyss Walker (36) → Ghost Hunter (108).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 81,
        name: "Q00081_SagaOfTheGhostHunter",
        html_dir: "quests/Q00081_SagaOfTheGhostHunter",
        npc: [
            31603, 31624, 31286, 31615, 31617, 31646, 31649, 31653, 31654, 31655, 31656, 31616,
        ],
        items: [
            7080, 7518, 7081, 7496, 7279, 7310, 7341, 7372, 7403, 7434, 7104, 0,
        ],
        mob: [27301, 27230, 27304],
        class_id: 108,
        prev_class: 36,
        spawn: [
            (164650, -74121, -2871),
            (47391, -56929, -2370),
            (47429, -56923, -2383),
        ],
    }
}
