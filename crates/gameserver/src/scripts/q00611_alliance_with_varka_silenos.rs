//! Alliance with Varka Silenos (611) — data table for the shared
//! [`AllianceQuest`](super::alliance::AllianceQuest) engine. The mirror of
//! quest 605: ally with the Varka Silenos (patron **Naran Ashanuk**, 31378) by
//! hunting the Ketra Orc camp for their badges.

use super::alliance::{AllianceData, MobDrop};

/// The Ketra Orc camp: `(npc_id, chance/1000, min_cond)`.
const MOBS: &[MobDrop] = &[
    (21324, 500, 1), // Ketra Orc Footman
    (21325, 500, 1), // Ketra's War Hound
    (21327, 509, 1), // Ketra Orc Raider
    (21328, 521, 1), // Ketra Orc Scout
    (21329, 519, 1), // Ketra Orc Shaman
    (21331, 500, 2), // Ketra Orc Warrior
    (21332, 500, 2), // Ketra Orc Lieutenant
    (21334, 509, 2), // Ketra Orc Medium
    (21335, 518, 2), // Ketra Orc Elite Soldier
    (21336, 518, 2), // Ketra Orc White Captain
    (21338, 527, 2), // Ketra Orc Seer
    (21339, 500, 3), // Ketra Orc General
    (21340, 500, 3), // Ketra Orc Battalion Commander
    (21342, 508, 3), // Ketra Orc Grand Seer
    (21343, 628, 2), // Ketra Commander
    (21344, 604, 2), // Ketra Elite Guard
    (21345, 627, 3), // Ketra's Head Shaman
    (21346, 604, 3), // Ketra's Head Guard
    (21347, 649, 3), // Ketra Prophet
    (21348, 626, 3), // Prophet's Guard
    (21349, 626, 3), // Prophet's Aide
];

pub fn data() -> AllianceData {
    AllianceData {
        start_npc: 31378,                            // Naran Ashanuk
        own_marks: [7221, 7222, 7223, 7224, 7225],   // Mark of Varka's Alliance Lv1-5
        enemy_marks: [7211, 7212, 7213, 7214, 7215], // Mark of Ketra's Alliance
        badges: [7226, 7227, 7228],                  // Ketra Badge: Soldier / Officer / Captain
        valor_totem: 7229,                           // Ketra's Valor Feather → here the valor gate
        wisdom_totem: 7230,                          // Ketra's Wisdom Feather → the wisdom gate
        mobs: MOBS,
    }
}
