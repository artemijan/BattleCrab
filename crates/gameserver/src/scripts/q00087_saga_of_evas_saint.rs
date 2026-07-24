//! Saga of Eva's Saint (87) — Elder (30) → Eva's Saint (105).
use super::saga::SagaData;
pub fn saga() -> SagaData {
    SagaData {
        id: 87,
        name: "Q00087_SagaOfEvasSaint",
        html_dir: "quests/Q00087_SagaOfEvasSaint",
        npc: [
            30191, 31626, 31588, 31280, 31620, 31646, 31649, 31653, 31654, 31655, 31657, 31280,
        ],
        items: [
            7080, 7524, 7081, 7502, 7285, 7316, 7347, 7378, 7409, 7440, 7088, 0,
        ],
        mob: [27266, 27236, 27276],
        class_id: 105,
        prev_class: 30,
        spawn: [
            (164650, -74121, -2871),
            (46087, -36372, -1685),
            (46066, -36396, -1685),
        ],
    }
}
