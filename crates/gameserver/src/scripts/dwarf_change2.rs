//! Dwarf second-class transfers — ports of
//! `dist/game/data/scripts/village_master/DwarfBlacksmithChange2/` and
//! `DwarfWarehouseChange2/`.
//!
//! | Script | From → to | Proofs (all three) |
//! |---|---|---|
//! | Blacksmith | Artisan (56) → Warsmith (57) | Guildsman, Prosperity, Maestro |
//! | Warehouse | Scavenger (54) → Bounty Hunter (55) | Guildsman, Prosperity, Searcher |
//!
//! **Three things differ from the `*Change1` pair:**
//!
//! 1. The level gate is **40**, not 20.
//! 2. **Three** proof items are required — and all three are consumed. Java's
//!    `hasQuestItems(player, a, b, c)` is an *and*, not an *or*; treating it as
//!    "any" would let a player transfer holding one mark.
//! 3. The reward is a **C**-grade shadow coupon (8870), not D-grade.
//!
//! And one structural quirk: **every** page is hard-coded to the *first* NPC's
//! id (`30512-…` / `30511-…`) whichever of the eight masters you talk to — the
//! dist ships exactly one 12-page set per script, not eight. The `*Change1`
//! scripts only did this for the fourth-class refusal.

use crate::game_loop::quests::{QuestCtx, QuestScript, echoed_page};

const SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE: i32 = 8870;

const MARK_OF_GUILDSMAN: i32 = 3119;
const MARK_OF_PROSPERITY: i32 = 3238;
const MARK_OF_MAESTRO: i32 = 2867;
const MARK_OF_SEARCHER: i32 = 2809;

const ARTISAN: i32 = 56;
const WARSMITH: i32 = 57;
const SCAVENGER: i32 = 54;
const BOUNTY_HUNTER: i32 = 55;

/// Kusto, Flutter, Vergara, Ferris, Roman, Noel, Lombert, Newyear.
const BLACKSMITH_NPCS: [i32; 8] = [30512, 30677, 30687, 30847, 30897, 31272, 31317, 31961];
/// Gesto, Croop, Baxt, Klump, Natools, Mona, Donal, Yasheni.
const WAREHOUSE_NPCS: [i32; 8] = [30511, 30676, 30685, 30845, 30894, 31269, 31314, 31958];

#[derive(Clone, Copy)]
pub enum Branch {
    Blacksmith,
    Warehouse,
}

pub struct DwarfChange2(Branch);

impl DwarfChange2 {
    pub const fn blacksmith() -> Self {
        Self(Branch::Blacksmith)
    }
    pub const fn warehouse() -> Self {
        Self(Branch::Warehouse)
    }

    fn npcs(&self) -> &'static [i32] {
        match self.0 {
            Branch::Blacksmith => &BLACKSMITH_NPCS,
            Branch::Warehouse => &WAREHOUSE_NPCS,
        }
    }

    /// Every page belongs to this id, whichever master is being talked to.
    fn page_npc(&self) -> i32 {
        self.npcs()[0]
    }

    /// `(from_class, to_class, [proofs], talk_category)`.
    fn spec(&self) -> (i32, i32, [i32; 3], &'static str) {
        match self.0 {
            Branch::Blacksmith => (
                ARTISAN,
                WARSMITH,
                [MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_MAESTRO],
                "WARSMITH_GROUP",
            ),
            Branch::Warehouse => (
                SCAVENGER,
                BOUNTY_HUNTER,
                [MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_SEARCHER],
                "BOUNTY_HUNTER_GROUP",
            ),
        }
    }

    fn class_change(&self, ctx: &mut QuestCtx, class_id: i32) -> Option<String> {
        let page = self.page_npc();
        let (from, to, proofs, _) = self.spec();
        if ctx.is_in_category("THIRD_CLASS_GROUP") {
            return Some(format!("{page}-08.htm"));
        }
        if class_id != to || ctx.player_class_id() != from {
            return None;
        }
        // Java's `hasQuestItems(player, a, b, c)` requires *all three*.
        let has_proofs = proofs.iter().all(|id| ctx.quest_items_count(*id) > 0);
        if ctx.player_level() < 40 {
            return Some(format!(
                "{page}-{}.htm",
                if has_proofs { "09" } else { "10" }
            ));
        }
        if !has_proofs {
            return Some(format!("{page}-12.htm"));
        }
        for id in proofs {
            ctx.take_items(id, -1);
        }
        ctx.set_class_id(to);
        ctx.give_items(SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, 15);
        Some(format!("{page}-11.htm"))
    }
}

impl QuestScript for DwarfChange2 {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        match self.0 {
            Branch::Blacksmith => "DwarfBlacksmithChange2",
            Branch::Warehouse => "DwarfWarehouseChange2",
        }
    }
    fn html_dir(&self) -> &'static str {
        match self.0 {
            Branch::Blacksmith => "village_master/DwarfBlacksmithChange2",
            Branch::Warehouse => "village_master/DwarfWarehouseChange2",
        }
    }
    fn start_npcs(&self) -> &[i32] {
        self.npcs()
    }
    fn talk_npcs(&self) -> &[i32] {
        self.npcs()
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let page = self.page_npc();
        let (from, to, _, category) = self.spec();
        // Java's blacksmith checks FOURTH alone; the warehouse checks FOURTH
        // *and* its group. Both reduce to the same thing for a real character,
        // and the shared form matches the warehouse's stricter reading.
        if ctx.is_in_category("FOURTH_CLASS_GROUP") && ctx.is_in_category(category) {
            return Some(format!("{page}-01.htm"));
        }
        if ctx.is_in_category(category) {
            let cid = ctx.player_class_id();
            if cid == from || cid == to {
                return Some(format!("{page}-02.htm"));
            }
            // In the group but not yet first-occupation.
            return Some(format!("{page}-06.htm"));
        }
        Some(format!("{page}-07.htm"))
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let (_, to, _, _) = self.spec();
        if event.parse::<i32>() == Ok(to) {
            return self.class_change(ctx, to);
        }
        echoed_page(event, &[self.page_npc()])
    }
}
