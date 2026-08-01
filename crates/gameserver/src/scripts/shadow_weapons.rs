//! Shadow Weapons (`custom/ShadowWeapons/ShadowWeapons.java`) — the village
//! master's "Use Shadow Weapon Exchange Coupon" desk.
//!
//! Every class transfer hands out 15 Shadow Item Exchange Coupons (8869
//! D-grade at first transfer, 8870 C-grade at second — see
//! [`crate::scripts::elf_human_change1`] and its siblings). This is where they
//! are spent: the script itself only picks which of four pages to show, and the
//! page's link opens the real exchange, one of three multisells, at 1 coupon
//! per weapon.
//!
//! | coupons held | page | multisell |
//! |---|---|---|
//! | D only | `exchange_d.html` | 306893001 (9 D-grade weapons) |
//! | C only | `exchange_c.html` | 306893002 (10 C-grade weapons) |
//! | both | `exchange_both.html` | 306893003 (all 19) |
//! | neither | `exchange_no.html` | — |
//!
//! **Restored, not ported.** This dist ships the coupons and the shadow items
//! but not the desk that joins them: the script folder is absent and the
//! `<Button …_Quest ShadowWeapons>` line is commented out in all 81
//! `html/villagemaster/*.htm`, so a character could earn 15 coupons per
//! transfer and never spend one. The script, its four htmls and the three
//! multisells come from the authentic Interlude datapack
//! (`L2J_Mobius_CT_0_Interlude`), and the button is uncommented for the 78
//! masters that appear in *both* that script's NPC list and this dist's
//! htmls. The three htmls whose master is in neither multisell's `<npcs>`
//! allow-list (30508, 30594, 31279) keep their button commented — upstream
//! never wired those, and an uncommented one would open a page whose exchange
//! link then refuses.

use crate::game_loop::quests::{QuestCtx, QuestScript};

pub struct ShadowWeapons;

/// Shadow Item Exchange Coupon (D-Grade) — first class transfer.
const COUPON_D: i32 = 8869;
/// Shadow Item Exchange Coupon (C-Grade) — second class transfer.
const COUPON_C: i32 = 8870;

/// Java `ShadowWeapons.NPCS` — every Grand Master / Magister / High Priest.
const NPCS: &[i32] = &[
    30037, 30066, 30070, 30109, 30115, 30120, 30174, 30175, 30176, 30187, 30191, 30195, 30288,
    30289, 30290, 30297, 30373, 30462, 30474, 30498, 30499, 30500, 30503, 30504, 30505, 30511,
    30512, 30513, 30595, 30676, 30677, 30681, 30685, 30687, 30689, 30694, 30699, 30704, 30845,
    30847, 30849, 30854, 30857, 30862, 30865, 30894, 30897, 30900, 30905, 30910, 30913, 31269,
    31272, 31276, 31285, 31288, 31314, 31317, 31321, 31324, 31326, 31328, 31331, 31334, 31336,
    31958, 31961, 31965, 31968, 31974, 31977, 31996, 32092, 32093, 32094, 32095, 32096, 32097,
    32098,
];

impl QuestScript for ShadowWeapons {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ShadowWeapons"
    }
    fn html_dir(&self) -> &'static str {
        "custom/ShadowWeapons"
    }
    fn start_npcs(&self) -> &[i32] {
        NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        NPCS
    }

    /// Java's whole `onTalk`: which coupons are in the bag decides the page.
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let has_d = ctx.quest_items_count(COUPON_D) > 0;
        let has_c = ctx.quest_items_count(COUPON_C) > 0;
        Some(
            match (has_d, has_c) {
                (true, true) => "exchange_both.html",
                (true, false) => "exchange_d.html",
                (false, true) => "exchange_c.html",
                (false, false) => "exchange_no.html",
            }
            .to_string(),
        )
    }
}
