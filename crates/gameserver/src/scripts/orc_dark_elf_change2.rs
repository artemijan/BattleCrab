//! Orc and Dark Elf second-class transfers — ports of
//! `dist/game/data/scripts/village_master/OrcChange2/` and
//! `DarkElfChange2/`.
//!
//! Both are level-40, three-proof second occupations, but they differ from each
//! other in three ways that are silent if you pattern-match:
//!
//! | | Orc | Dark Elf |
//! |---|---|---|
//! | bypass event | the **class id** | the **row index** |
//! | pages | `30513-NN.htm` | `30474-NN.html` |
//! | page order in a row | low, lowNoProof, done, noProof | **lowNoProof, low**, noProof, done |
//! | coupon reward | 15 × C-grade | **none at all** |
//!
//! The page owner is also not the first NPC in either list: Orc pages belong to
//! 30513 (which *is* first) but Dark Elf pages belong to **30474 — the third**.
//! One page set serves every master in both cases.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE: i32 = 8870;

// Shared marks.
const MARK_OF_CHALLENGER: i32 = 2627;
const MARK_OF_DUTY: i32 = 2633;
const MARK_OF_SEEKER: i32 = 2673;
const MARK_OF_SCHOLAR: i32 = 2674;
const MARK_OF_PILGRIM: i32 = 2721;
const MARK_OF_DUELIST: i32 = 2762;
const MARK_OF_SEARCHER: i32 = 2809;
const MARK_OF_REFORMER: i32 = 2821;
const MARK_OF_MAGUS: i32 = 2840;
const MARK_OF_WARSPIRIT: i32 = 2879;
const MARK_OF_FATE: i32 = 3172;
const MARK_OF_GLORY: i32 = 3203;
const MARK_OF_CHAMPION: i32 = 3276;
const MARK_OF_SAGITTARIUS: i32 = 3293;
const MARK_OF_WITCHCRAFT: i32 = 3307;
const MARK_OF_SUMMONER: i32 = 3336;
const MARK_OF_LORD: i32 = 3390;

const RACE_DARK_ELF: i32 = 2;

/// `(to, from, first_page, [three proofs])`.
type Row = (i32, i32, u32, [i32; 3]);

const ORC_NPCS: [i32; 9] = [
    30513, 30681, 30704, 30865, 30913, 31288, 31326, 31336, 31977,
];
const ORC_PAGE_NPC: i32 = 30513;
const ORC_ROWS: [Row; 4] = [
    (
        46,
        45,
        20,
        [MARK_OF_CHALLENGER, MARK_OF_GLORY, MARK_OF_CHAMPION],
    ), // Destroyer ← Orc Raider
    (
        48,
        47,
        24,
        [MARK_OF_CHALLENGER, MARK_OF_GLORY, MARK_OF_DUELIST],
    ), // Tyrant ← Orc Monk
    (51, 50, 28, [MARK_OF_PILGRIM, MARK_OF_GLORY, MARK_OF_LORD]), // Overlord ← Orc Shaman
    (
        52,
        50,
        32,
        [MARK_OF_PILGRIM, MARK_OF_GLORY, MARK_OF_WARSPIRIT],
    ), // Warcryer ← Orc Shaman
];

const DE_NPCS: [i32; 10] = [
    30195, 30699, 30474, 31324, 30862, 30910, 31285, 31334, 31974, 32096,
];
const DE_PAGE_NPC: i32 = 30474;
/// Java's `CLASSES` rows, keeping its `lowNoProof, low, noProof, done` order.
const DE_ROWS: [Row; 7] = [
    (33, 32, 26, [MARK_OF_DUTY, MARK_OF_FATE, MARK_OF_WITCHCRAFT]), // Shillien Knight
    (
        34,
        32,
        30,
        [MARK_OF_CHALLENGER, MARK_OF_FATE, MARK_OF_DUELIST],
    ), // Bladedancer
    (
        43,
        42,
        34,
        [MARK_OF_PILGRIM, MARK_OF_FATE, MARK_OF_REFORMER],
    ), // Shillien Elder
    (36, 35, 38, [MARK_OF_SEEKER, MARK_OF_FATE, MARK_OF_SEARCHER]), // Abyss Walker
    (
        37,
        35,
        42,
        [MARK_OF_SEEKER, MARK_OF_FATE, MARK_OF_SAGITTARIUS],
    ), // Phantom Ranger
    (40, 39, 46, [MARK_OF_SCHOLAR, MARK_OF_FATE, MARK_OF_MAGUS]),   // Spellhowler
    (
        41,
        39,
        50,
        [MARK_OF_SCHOLAR, MARK_OF_FATE, MARK_OF_SUMMONER],
    ), // Phantom Summoner
];

#[derive(Clone, Copy)]
pub enum Branch {
    Orc,
    DarkElf,
}

pub struct Change2(Branch);

impl Change2 {
    pub const fn orc() -> Self {
        Self(Branch::Orc)
    }
    pub const fn dark_elf() -> Self {
        Self(Branch::DarkElf)
    }

    fn npcs(&self) -> &'static [i32] {
        match self.0 {
            Branch::Orc => &ORC_NPCS,
            Branch::DarkElf => &DE_NPCS,
        }
    }
    fn rows(&self) -> &'static [Row] {
        match self.0 {
            Branch::Orc => &ORC_ROWS,
            Branch::DarkElf => &DE_ROWS,
        }
    }
    fn page_npc(&self) -> i32 {
        match self.0 {
            Branch::Orc => ORC_PAGE_NPC,
            Branch::DarkElf => DE_PAGE_NPC,
        }
    }
    fn ext(&self) -> &'static str {
        match self.0 {
            Branch::Orc => "htm",
            Branch::DarkElf => "html",
        }
    }

    fn page(&self, n: u32) -> String {
        format!("{}-{n}.{}", self.page_npc(), self.ext())
    }

    /// Run one row's level/proof matrix. `row.2` is the first of four
    /// consecutive pages, whose meaning differs per branch — see the module
    /// table.
    fn apply(&self, ctx: &mut QuestCtx, row: Row) -> String {
        let (to, _, first, proofs) = row;
        let has_all = proofs.iter().all(|id| ctx.quest_items_count(*id) > 0);
        let (low, low_no_proof, no_proof, done) = match self.0 {
            Branch::Orc => (first, first + 1, first + 3, first + 2),
            Branch::DarkElf => (first + 1, first, first + 2, first + 3),
        };
        if ctx.player_level() < 40 {
            return self.page(if has_all { low } else { low_no_proof });
        }
        if !has_all {
            return self.page(no_proof);
        }
        for id in proofs {
            ctx.take_items(id, -1);
        }
        ctx.set_class_id(to);
        // Only the Orc masters hand out a coupon; the Dark Elf script gives
        // nothing (verified against the dist: zero `giveItems` calls).
        if matches!(self.0, Branch::Orc) {
            ctx.give_items(SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, 15);
        }
        self.page(done)
    }
}

impl QuestScript for Change2 {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        match self.0 {
            Branch::Orc => "OrcChange2",
            Branch::DarkElf => "DarkElfChange2",
        }
    }
    fn html_dir(&self) -> &'static str {
        match self.0 {
            Branch::Orc => "village_master/OrcChange2",
            Branch::DarkElf => "village_master/DarkElfChange2",
        }
    }
    fn start_npcs(&self) -> &[i32] {
        self.npcs()
    }
    fn talk_npcs(&self) -> &[i32] {
        self.npcs()
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match self.0 {
            // Orc: the event is the target class id.
            Branch::Orc => {
                if let Ok(class_id) = event.parse::<i32>() {
                    if ctx.is_in_category("THIRD_CLASS_GROUP") {
                        return Some(self.page(19));
                    }
                    if let Some(&row) = self
                        .rows()
                        .iter()
                        .find(|(to, from, _, _)| *to == class_id && *from == ctx.player_class_id())
                    {
                        return Some(self.apply(ctx, row));
                    }
                }
            }
            // Dark Elf: the event is the row index.
            Branch::DarkElf => {
                if let Ok(i) = event.parse::<usize>() {
                    let Some(&row) = self.rows().get(i) else {
                        return Some(event.to_string());
                    };
                    if ctx.player_race() != RACE_DARK_ELF || ctx.player_class_id() != row.1 {
                        return Some(event.to_string());
                    }
                    return Some(self.apply(ctx, row));
                }
            }
        }
        if event.ends_with(".htm") || event.ends_with(".html") {
            return Some(event.to_string());
        }
        None
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        match self.0 {
            Branch::Orc => {
                let in_orc =
                    ctx.is_in_category("ORC_MALL_CLASS") || ctx.is_in_category("ORC_FALL_CLASS");
                if ctx.is_in_category("FOURTH_CLASS_GROUP") && in_orc {
                    return Some(self.page(1));
                }
                if !in_orc {
                    return Some(self.page(18));
                }
                Some(match ctx.player_class_id() {
                    45 | 46 => self.page(2),       // Orc Raider / Destroyer
                    47 | 48 => self.page(6),       // Orc Monk / Tyrant
                    50 | 51 | 52 => self.page(10), // Shaman / Overlord / Warcryer
                    _ => self.page(17),
                })
            }
            Branch::DarkElf => {
                // `if (player.isSubClassActive()) return getNoQuestMsg(player);`
                if ctx.is_subclass_active() {
                    return Some(ctx.no_quest_html());
                }
                if ctx.player_race() != RACE_DARK_ELF {
                    return Some(self.page(56));
                }
                Some(match ctx.player_class_id() {
                    32 => self.page(1),  // Palus Knight
                    42 => self.page(8),  // Shillien Oracle
                    35 => self.page(12), // Assassin
                    39 => self.page(19), // Dark Wizard
                    _ if ctx.is_in_category("SECOND_CLASS_GROUP")
                        || ctx.is_in_category("THIRD_CLASS_GROUP") =>
                    {
                        self.page(54)
                    }
                    // No first occupation yet.
                    _ => self.page(55),
                })
            }
        }
    }
}
