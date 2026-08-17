//! Dark Elf first-class transfer — port of
//! `dist/game/data/scripts/village_master/DarkElfChange1/`.
//!
//! Xenos, Tobias and Tronix turn a **Dark Fighter** (31) into a Palus Knight
//! (32) or Assassin (35), and a **Dark Mage** (38) into a Dark Wizard (39) or
//! Shillien Oracle (42), at level 20+ for the proof item and 15 D-grade shadow
//! coupons.
//!
//! **Three things differ from the other `Change1` scripts**, all of them easy
//! to get wrong by pattern-matching on the siblings:
//!
//! 1. Java already writes this one as a **table**, and the bypass event is the
//!    **row index** (`0..3`), *not* a class id.
//! 2. The page order inside a row is `lowNoProof, low, noProof, done` — the
//!    *opposite* pairing to `ElfHumanFighterChange1`'s
//!    `low, lowNoProof, done, noProof`.
//! 3. The pages are **`.html`**, not `.htm`.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const SHADOW_WEAPON_COUPON_DGRADE: i32 = 8869;

const DARK_FIGHTER: i32 = 31;
const DARK_MAGE: i32 = 38;
const RACE_DARK_ELF: i32 = 2;

/// Xenos, Tobias, Tronix.
const NPCS: [i32; 3] = [30290, 30297, 30462];

/// Java's `CLASSES` rows: `(to, from, low_no_proof, low, no_proof, done, proof)`.
const CLASSES: [(i32, i32, u32, u32, u32, u32, i32); 4] = [
    (32, DARK_FIGHTER, 15, 16, 17, 18, 1244), // Palus Knight — Gaze of Abyss
    (35, DARK_FIGHTER, 19, 20, 21, 22, 1252), // Assassin — Iron Heart
    (39, DARK_MAGE, 23, 24, 25, 26, 1261),    // Dark Wizard — Jewel of Darkness
    (42, DARK_MAGE, 27, 28, 29, 30, 1270),    // Shillien Oracle — Orb of Abyss
];

pub struct DarkElfChange1;

impl QuestScript for DarkElfChange1 {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "DarkElfChange1"
    }
    fn html_dir(&self) -> &'static str {
        "village_master/DarkElfChange1"
    }
    fn start_npcs(&self) -> &[i32] {
        &NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        &NPCS
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // `if (player.isSubClassActive()) return getNoQuestMsg(player);` — a
        // character on a subclass can't take a first occupation at all.
        if ctx.is_subclass_active() {
            return Some(ctx.no_quest_html());
        }
        let npc = ctx.npc_id;
        if ctx.player_race() != RACE_DARK_ELF {
            return Some(format!("{npc}-33.html"));
        }
        match ctx.player_class_id() {
            DARK_FIGHTER => Some(format!("{npc}-01.html")),
            DARK_MAGE => Some(format!("{npc}-08.html")),
            // Java switches on `cid.level()`: 1 = first occupation already
            // taken, ≥2 = second or third.
            _ if ctx.is_in_category("FIRST_CLASS_GROUP") => Some(format!("{npc}-32.html")),
            _ if ctx.is_in_category("SECOND_CLASS_GROUP")
                || ctx.is_in_category("THIRD_CLASS_GROUP") =>
            {
                Some(format!("{npc}-31.html"))
            }
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let npc = ctx.npc_id;
        // The event is the CLASSES row index, not a class id.
        if let Ok(i) = event.parse::<usize>() {
            let (to, from, low_no_proof, low, no_proof, done, proof) = *CLASSES.get(i)?;
            // Java checks the race *and* the exact source class.
            if ctx.player_race() != RACE_DARK_ELF || ctx.player_class_id() != from {
                return Some(event.to_string());
            }
            let has_proof = ctx.quest_items_count(proof) > 0;
            let suffix = if ctx.player_level() < 20 {
                if has_proof { low } else { low_no_proof }
            } else if !has_proof {
                no_proof
            } else {
                ctx.give_items(SHADOW_WEAPON_COUPON_DGRADE, 15);
                ctx.take_items(proof, -1);
                ctx.set_class_id(to);
                done
            };
            return Some(format!("{npc}-{suffix}.html"));
        }
        // Java's `onEvent` returns the event unchanged for anything else, so a
        // page link echoes back.
        Some(event.to_string())
    }
}
