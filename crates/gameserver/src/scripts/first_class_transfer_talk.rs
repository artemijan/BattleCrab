//! Headmaster first-class-transfer *talk* — port of
//! `dist/game/data/scripts/village_master/FirstClassTransferTalk/`.
//!
//! The seven newbie-village headmasters. As the Java header says: **none of
//! them performs a class transfer — they only talk about it.** The real
//! transfers are the `*Change1` scripts; these NPCs explain the idea and link
//! onward.
//!
//! Page names use an **underscore** (`30026_fighter.html`), not the `-NN.htm`
//! convention of every other village-master script, and the extension is
//! `.html`.
//!
//! The reply depends on the master's race, the player's race, whether the
//! player is a mage, and how far the player has already advanced:
//!
//! - race mismatch → `_no.html`
//! - not yet advanced (class level 0) → `_fighter` / `_mystic`, per race rules
//! - first occupation taken (level 1) → `_transfer_1.html`
//! - second or beyond → `_transfer_2.html`

use crate::game_loop::quests::{QuestCtx, QuestScript};

const RACE_HUMAN: i32 = 0;
const RACE_ELF: i32 = 1;
const RACE_DARK_ELF: i32 = 2;
const RACE_ORC: i32 = 3;
const RACE_DWARF: i32 = 4;

/// `(npc_id, race)`. The two Human masters are additionally split by the NPC's
/// own type: Blitz is a fighter guild head, Biotin a temple high priest, and
/// each only answers its own side (the dist ships `30026_fighter.html` and
/// `30031_mystic.html` — never the reverse).
const MASTERS: [(i32, i32); 7] = [
    (30026, RACE_HUMAN),    // Blitz — TI Fighter Guild Head Master
    (30031, RACE_HUMAN),    // Biotin — TI Einhasad Temple High Priest
    (30154, RACE_ELF),      // Asterios — Elven Village Tetrarch
    (30358, RACE_DARK_ELF), // Thifiell — Dark Elf Village Tetrarch
    (30565, RACE_ORC),      // Kakai — Orc Village Flame Lord
    (30520, RACE_DWARF),    // Reed — Dwarven Village Warehouse Chief
    (30525, RACE_DWARF),    // Bronk — Dwarven Village Head Blacksmith
];

const NPC_IDS: [i32; 7] = [30026, 30031, 30154, 30358, 30565, 30520, 30525];

/// The Human fighter-guild master; the other Human master is the priest.
const HUMAN_FIGHTER_MASTER: i32 = 30026;

pub struct FirstClassTransferTalk;

impl FirstClassTransferTalk {
    /// Java `ClassId.level()`: 0 = no occupation yet, 1 = first, 2+ = second
    /// or third. Derived from the category groups rather than a class table.
    fn class_level(ctx: &QuestCtx) -> i32 {
        if ctx.is_in_category("FIRST_CLASS_GROUP") {
            1
        } else if ctx.is_in_category("SECOND_CLASS_GROUP")
            || ctx.is_in_category("THIRD_CLASS_GROUP")
            || ctx.is_in_category("FOURTH_CLASS_GROUP")
        {
            2
        } else {
            0
        }
    }
}

impl QuestScript for FirstClassTransferTalk {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FirstClassTransferTalk"
    }
    fn html_dir(&self) -> &'static str {
        "village_master/FirstClassTransferTalk"
    }
    fn start_npcs(&self) -> &[i32] {
        &NPC_IDS
    }
    fn talk_npcs(&self) -> &[i32] {
        &NPC_IDS
    }

    /// Java's `onEvent` returns the event unchanged — the pages link to each
    /// other and nothing is transacted here.
    fn on_event(&self, _ctx: &mut QuestCtx, event: &str) -> Option<String> {
        Some(event.to_string())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let npc = ctx.npc_id;
        let Some(&(_, master_race)) = MASTERS.iter().find(|(id, _)| *id == npc) else {
            return None;
        };
        if master_race != ctx.player_race() {
            return Some(format!("{npc}_no.html"));
        }

        let suffix = match Self::class_level(ctx) {
            1 => "transfer_1".to_string(),
            n if n >= 2 => "transfer_2".to_string(),
            _ => {
                let is_mage = ctx.is_in_category("MAGE_GROUP");
                match master_race {
                    // Each Human master answers only its own side; the other
                    // gets `no.html` (and the dist ships no page for it).
                    RACE_HUMAN => {
                        let fighter_master = npc == HUMAN_FIGHTER_MASTER;
                        if is_mage && !fighter_master {
                            "mystic".to_string()
                        } else if !is_mage && fighter_master {
                            "fighter".to_string()
                        } else {
                            "no".to_string()
                        }
                    }
                    // Elf, Dark Elf and Orc masters serve both sides.
                    RACE_ELF | RACE_DARK_ELF | RACE_ORC => {
                        if is_mage {
                            "mystic".to_string()
                        } else {
                            "fighter".to_string()
                        }
                    }
                    // Dwarves have no mage line at all.
                    RACE_DWARF => "fighter".to_string(),
                    _ => "no".to_string(),
                }
            }
        };
        Some(format!("{npc}_{suffix}.html"))
    }
}
