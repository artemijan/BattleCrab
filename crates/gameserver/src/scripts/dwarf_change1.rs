//! Dwarf first-class transfers — ports of
//! `dist/game/data/scripts/village_master/DwarfBlacksmithChange1/` and
//! `DwarfWarehouseChange1/`.
//!
//! Both turn a **Dwarven Fighter** (53) into one of the two Dwarf first
//! occupations at level 20+, taking the proof item and paying 15 D-grade
//! shadow coupons:
//!
//! | Script | Target | Proof |
//! |---|---|---|
//! | Blacksmith (Tapoy, Mendio, Opix, Bolin) | Artisan (56) | Final Pass Certificate |
//! | Warehouse (Moke, Rikadio, Ranspo, Alder) | Scavenger (54) | Ring of Raven |
//!
//! The two scripts are identical bar the NPC list, target class, proof item
//! and the category their `onTalk` gates on, so they share one implementation
//! parameterised by [`Branch`]. Dialog-only utility scripts (id ≤ 0), driven
//! through `Quest <name> <event>` bypasses in the dist htmls — the same shape
//! as [`super::orc_change1`].

use crate::game_loop::quests::{QuestCtx, QuestScript};

const SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE: i32 = 8869;

const DWARVEN_FIGHTER: i32 = 53;
const SCAVENGER: i32 = 54;
const ARTISAN: i32 = 56;

const FINAL_PASS_CERTIFICATE: i32 = 1635;
const RING_OF_RAVEN: i32 = 1642;

/// Tapoy, Mendio, Opix, Bolin.
const BLACKSMITH_NPCS: [i32; 4] = [30499, 30504, 30595, 32093];
/// Moke, Rikadio, Ranspo, Alder.
const WAREHOUSE_NPCS: [i32; 4] = [30498, 30503, 30594, 32092];

/// Java uses `30499-12.htm` / `30498-12.htm` for the fourth-class refusal —
/// the *first* NPC's page, whichever NPC you are actually talking to.
const BLACKSMITH_FOURTH_CLASS_HTM: &str = "30499-12.htm";
const WAREHOUSE_FOURTH_CLASS_HTM: &str = "30498-12.htm";

pub struct DwarfChange1(Branch);

#[derive(Clone, Copy)]
pub enum Branch {
    Blacksmith,
    Warehouse,
}

impl DwarfChange1 {
    pub const fn blacksmith() -> Self {
        Self(Branch::Blacksmith)
    }
    pub const fn warehouse() -> Self {
        Self(Branch::Warehouse)
    }

    fn target_class(&self) -> i32 {
        match self.0 {
            Branch::Blacksmith => ARTISAN,
            Branch::Warehouse => SCAVENGER,
        }
    }

    fn proof_item(&self) -> i32 {
        match self.0 {
            Branch::Blacksmith => FINAL_PASS_CERTIFICATE,
            Branch::Warehouse => RING_OF_RAVEN,
        }
    }

    /// The category each script's `onTalk` gates on before offering the list.
    fn talk_category(&self) -> &'static str {
        match self.0 {
            Branch::Blacksmith => "WARSMITH_GROUP",
            Branch::Warehouse => "BOUNTY_HUNTER_GROUP",
        }
    }

    fn fourth_class_htm(&self) -> &'static str {
        match self.0 {
            Branch::Blacksmith => BLACKSMITH_FOURTH_CLASS_HTM,
            Branch::Warehouse => WAREHOUSE_FOURTH_CLASS_HTM,
        }
    }

    /// `ClassChangeRequested`: the already-advanced refusals first, then the
    /// level / proof matrix.
    fn class_change(&self, ctx: &mut QuestCtx, class_id: i32) -> Option<String> {
        let npc = ctx.npc_id;
        if ctx.is_in_category("SECOND_CLASS_GROUP") {
            return Some(format!("{npc}-06.htm"));
        }
        if ctx.is_in_category("THIRD_CLASS_GROUP") {
            return Some(format!("{npc}-07.htm"));
        }
        if ctx.is_in_category("FOURTH_CLASS_GROUP") {
            return Some(self.fourth_class_htm().to_string());
        }
        // Only the one target, and only from Dwarven Fighter.
        if class_id != self.target_class() || ctx.player_class_id() != DWARVEN_FIGHTER {
            return None;
        }

        let has_proof = ctx.quest_items_count(self.proof_item()) > 0;
        if ctx.player_level() < 20 {
            return Some(format!("{npc}-{}.htm", if has_proof { "08" } else { "09" }));
        }
        if !has_proof {
            return Some(format!("{npc}-11.htm"));
        }
        ctx.take_items(self.proof_item(), -1);
        // `setClassId` + `setBaseClass` in Java; the port's mechanic moves the
        // base class itself when the player is on the base slot (G17).
        ctx.set_class_id(self.target_class());
        ctx.give_items(SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, 15);
        Some(format!("{npc}-10.htm"))
    }

    fn npcs(&self) -> &'static [i32] {
        match self.0 {
            Branch::Blacksmith => &BLACKSMITH_NPCS,
            Branch::Warehouse => &WAREHOUSE_NPCS,
        }
    }
}

impl QuestScript for DwarfChange1 {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        match self.0 {
            Branch::Blacksmith => "DwarfBlacksmithChange1",
            Branch::Warehouse => "DwarfWarehouseChange1",
        }
    }
    fn html_dir(&self) -> &'static str {
        match self.0 {
            Branch::Blacksmith => "village_master/DwarfBlacksmithChange1",
            Branch::Warehouse => "village_master/DwarfWarehouseChange1",
        }
    }
    fn start_npcs(&self) -> &[i32] {
        self.npcs()
    }
    fn talk_npcs(&self) -> &[i32] {
        self.npcs()
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let target = self.target_class().to_string();
        if event == target {
            return self.class_change(ctx, self.target_class());
        }
        // The dialog pages echo back.
        if event.ends_with(".htm")
            && self
                .npcs()
                .iter()
                .any(|id| event.starts_with(&id.to_string()))
        {
            return Some(event.to_string());
        }
        None
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let npc = ctx.npc_id;
        if ctx.is_in_category(self.talk_category()) {
            Some(format!("{npc}-01.htm"))
        } else {
            Some(format!("{npc}-05.htm"))
        }
    }
}
