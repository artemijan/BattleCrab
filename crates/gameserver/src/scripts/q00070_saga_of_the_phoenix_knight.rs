//! Saga of the Phoenix Knight (70) — Paladin (5) → Phoenix Knight (90). A thin
//! [`SagaData`](super::saga::SagaData) table over the shared
//! [`SagaQuest`](super::saga::SagaQuest) engine; authentic Interlude data from
//! the C6 datapack (the dist ships the off-chronicle Classic version).

use super::saga::SagaData;

pub fn saga() -> SagaData {
    SagaData {
        id: 70,
        name: "Q00070_SagaOfThePhoenixKnight",
        html_dir: "quests/Q00070_SagaOfThePhoenixKnight",
        npc: [
            30849, 31624, 31277, 30849, 31631, 31646, 31647, 31650, 31654, 31655, 31657, 31277,
        ],
        items: [
            7080, 7534, 7081, 7485, 7268, 7299, 7330, 7361, 7392, 7423, 7093, 6482,
        ],
        mob: [27286, 27219, 27278],
        class_id: 90,
        prev_class: 5,
        spawn: [
            (191046, -40640, -3042),
            (46087, -36372, -1685),
            (46066, -36396, -1685),
        ],
    }
}
