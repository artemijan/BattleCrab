//! Orc first-class transfer — port of
//! `dist/game/data/scripts/village_master/OrcChange1/`. The four Orc
//! high prefects turn an Orc Fighter into a Raider (45, Mark of Raider) or
//! Monk (47, Khavatari Totem), and an Orc Mystic into a Shaman (50, Mask of
//! Medium), at level 20+, paying 15 D-grade shadow coupons. Dialog-only
//! utility script (id ≤ 0), driven through `Quest OrcChange1 <event>`
//! bypasses in the dist htmls.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const NPCS: [i32; 4] = [30500, 30505, 30508, 32097]; // Osborn, Drikus, Castor, Finker

const SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE: i32 = 8869;
const MARK_OF_RAIDER: i32 = 1592;
const KHAVATARI_TOTEM: i32 = 1615;
const MASK_OF_MEDIUM: i32 = 1631;

const ORC_FIGHTER: i32 = 44;
const ORC_MYSTIC: i32 = 49;

pub struct OrcChange1;

impl OrcChange1 {
    /// The `ClassChangeRequested` body: category gates, then the per-target
    /// (proof item, current class) matrix.
    fn class_change(ctx: &mut QuestCtx, class_id: i32) -> Option<String> {
        let npc = ctx.npc_id;
        if ctx.is_in_category("SECOND_CLASS_GROUP") {
            return Some(format!("{npc}-09.htm"));
        }
        if ctx.is_in_category("THIRD_CLASS_GROUP") {
            return Some(format!("{npc}-10.htm"));
        }
        if ctx.is_in_category("FOURTH_CLASS_GROUP") {
            return Some("30500-24.htm".to_string());
        }
        let (proof, required_class, low, low_no_proof, no_proof, done) = match class_id {
            45 => (MARK_OF_RAIDER, ORC_FIGHTER, 11, 12, 13, 14),
            47 => (KHAVATARI_TOTEM, ORC_FIGHTER, 15, 16, 17, 18),
            50 => (MASK_OF_MEDIUM, ORC_MYSTIC, 19, 20, 21, 22),
            _ => return None,
        };
        if ctx.player_class_id() != required_class {
            return None;
        }
        let has_proof = ctx.quest_items_count(proof) > 0;
        if ctx.player_level() < 20 {
            return Some(format!(
                "{npc}-{}.htm",
                if has_proof { low } else { low_no_proof }
            ));
        }
        if !has_proof {
            return Some(format!("{npc}-{no_proof}.htm"));
        }
        ctx.take_items(proof, -1);
        ctx.set_class_id(class_id);
        ctx.give_items(SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, 15);
        Some(format!("{npc}-{done}.htm"))
    }
}

impl QuestScript for OrcChange1 {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "OrcChange1"
    }
    fn html_dir(&self) -> &'static str {
        "village_master/OrcChange1"
    }
    fn start_npcs(&self) -> &[i32] {
        &NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        &NPCS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // The dialog pages echo back; the numeric events are transfers.
        match event {
            "45" | "47" | "50" => Self::class_change(ctx, event.parse().unwrap()),
            _ if event.ends_with(".htm")
                && NPCS.iter().any(|id| event.starts_with(&id.to_string())) =>
            {
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        const RACE_ORC: i32 = 3;
        let npc = ctx.npc_id;
        if ctx.player_race() == RACE_ORC {
            if ctx.is_in_category("FIGHTER_GROUP") {
                return Some(format!("{npc}-01.htm"));
            }
            if ctx.is_in_category("MAGE_GROUP") {
                return Some(format!("{npc}-06.htm"));
            }
            return None;
        }
        Some(format!("{npc}-23.htm"))
    }
}
