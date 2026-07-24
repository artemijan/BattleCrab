//! Alliance with Ketra Orcs (605) — data table for the shared
//! [`AllianceQuest`](super::alliance::AllianceQuest) engine. Ally with the
//! Ketra Orcs (patron **Wahkan**, 31371) by hunting the Varka Silenos camp for
//! their badges. See `alliance.rs` for the ladder logic.

use super::alliance::{AllianceData, MobDrop};

/// The Varka Silenos camp: `(npc_id, chance/1000, min_cond)`.
const MOBS: &[MobDrop] = &[
    (21350, 500, 1), // Varka Silenos Recruit
    (21351, 500, 1), // Varka Silenos Footman
    (21353, 509, 1), // Varka Silenos Scout
    (21354, 521, 1), // Varka Silenos Hunter
    (21355, 519, 1), // Varka Silenos Shaman
    (21357, 500, 2), // Varka Silenos Priest
    (21358, 500, 2), // Varka Silenos Warrior
    (21360, 509, 2), // Varka Silenos Medium
    (21361, 518, 2), // Varka Silenos Magus
    (21362, 518, 2), // Varka Silenos Officer
    (21364, 527, 2), // Varka Silenos Seer
    (21365, 500, 3), // Varka Silenos Great Magus
    (21366, 500, 3), // Varka Silenos General
    (21368, 508, 3), // Varka Silenos Great Seer
    (21369, 628, 2), // Varka's Commander
    (21370, 604, 2), // Varka's Elite Guard
    (21371, 627, 3), // Varka's Head Magus
    (21372, 604, 3), // Varka's Head Guard
    (21373, 649, 3), // Varka's Prophet
    (21374, 626, 3), // Prophet's Guard
    (21375, 626, 3), // Disciple of Prophet
];

pub fn data() -> AllianceData {
    AllianceData {
        start_npc: 31371,                            // Wahkan
        own_marks: [7211, 7212, 7213, 7214, 7215],   // Mark of Ketra's Alliance Lv1-5
        enemy_marks: [7221, 7222, 7223, 7224, 7225], // Mark of Varka's Alliance
        badges: [7216, 7217, 7218],                  // Varka Badge: Soldier / Officer / Captain
        valor_totem: 7219,
        wisdom_totem: 7220,
        mobs: MOBS,
    }
}
