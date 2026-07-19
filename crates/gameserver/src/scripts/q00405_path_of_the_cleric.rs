//! Path Of The Cleric (405) — port of
//! `dist/game/data/scripts/quests/Q00405_PathOfTheCleric/`.
//!
//! Awards the **Mark of Faith** (1201), the second of the four proofs
//! `ElfHumanWizardChange1` consumes.
//!
//! Two errands run back-to-back out of Zigaunt's letters. The first collects
//! three books (Vivyan, Simplon, Praga); the second is a courier loop —
//! Lionel's book → Gallint's certificate → back to Lionel for Lemoniell's
//! covenant → back to Zigaunt.
//!
//! ## Two things that would be wrong if normalised
//!
//! **Simplon hands over a stack of three.** `giveItems(player, BOOK_OF_SIMPLON,
//! 3)` where Vivyan and Praga give one each, and the completion accordingly
//! does `takeItems(BOOK_OF_SIMPLON, -1)` (all) but `takeItems(..., 1)` for the
//! other two. Treating the three books uniformly would either leave two of
//! Simplon's behind or make the count check unsatisfiable.
//!
//! **The cond-2 checks contain a no-op term.** Each of the three book-givers
//! re-checks all three counts after giving its own, but writes its *own* slot
//! as `>= 0` — trivially true, a placeholder for "the one I just handed over":
//!
//! ```java
//! giveItems(player, BOOK_OF_VIVYAN, 1);
//! if ((count(BOOK_OF_SIMPLON) >= 3) && (count(BOOK_OF_VIVYAN) >= 0) && (count(BOOK_OF_PRAGA) >= 1))
//! ```
//!
//! So all three sites reduce to the same predicate — *hold all three books,
//! Simplon's counting three* — which is what the port checks once. Read
//! literally as written, the `>= 0` looks like a bug; it isn't, it's just
//! redundant.
//!
//! Praga's book is the slow one: he lends a necklace, and the pendant to match
//! it drops from Ruin Zombies with **no chance roll** — the first kill after
//! taking the necklace pays.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const GALLINT: i32 = 30017;
const ZIGAUNT: i32 = 30022;
const VIVYAN: i32 = 30030;
const TRADER_SIMPLON: i32 = 30253;
const GUARD_PRAGA: i32 = 30333;
const LIONEL: i32 = 30408;

const LETTER_OF_ORDER_1ST: i32 = 1191;
const LETTER_OF_ORDER_2ND: i32 = 1192;
const LIONELS_BOOK: i32 = 1193;
const BOOK_OF_VIVYAN: i32 = 1194;
const BOOK_OF_SIMPLON: i32 = 1195;
const BOOK_OF_PRAGA: i32 = 1196;
const CERTIFICATE_OF_GALLINT: i32 = 1197;
const PENDANT_OF_MOTHER: i32 = 1198;
const NECKLACE_OF_MOTHER: i32 = 1199;
const LEMONIELLS_COVENANT: i32 = 1200;
const MARK_OF_FAITH: i32 = 1201;

const RUIN_ZOMBIE: i32 = 20026;
const RUIN_ZOMBIE_LEADER: i32 = 20029;

const MAGE: i32 = 10;
const CLERIC: i32 = 15;
const MIN_LEVEL: i32 = 19;

/// Simplon's stack size — the reason the three books can't be treated alike.
const SIMPLON_BOOKS: i64 = 3;

const QUEST_ITEMS: [i32; 10] = [
    LETTER_OF_ORDER_1ST, LETTER_OF_ORDER_2ND, LIONELS_BOOK, BOOK_OF_VIVYAN, BOOK_OF_SIMPLON,
    BOOK_OF_PRAGA, CERTIFICATE_OF_GALLINT, PENDANT_OF_MOTHER, NECKLACE_OF_MOTHER,
    LEMONIELLS_COVENANT,
];

pub struct Q00405PathOfTheCleric;

impl Q00405PathOfTheCleric {
    /// The predicate all three cond-2 sites reduce to.
    fn has_all_books(&self, ctx: &QuestCtx) -> bool {
        ctx.quest_items_count(BOOK_OF_SIMPLON) >= SIMPLON_BOOKS
            && ctx.quest_items_count(BOOK_OF_VIVYAN) >= 1
            && ctx.quest_items_count(BOOK_OF_PRAGA) >= 1
    }
}

impl QuestScript for Q00405PathOfTheCleric {
    fn id(&self) -> i32 {
        405
    }
    fn name(&self) -> &'static str {
        "Q00405_PathOfTheCleric"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00405_PathOfTheCleric"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ZIGAUNT]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ZIGAUNT, GALLINT, VIVYAN, TRADER_SIMPLON, GUARD_PRAGA, LIONEL]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[RUIN_ZOMBIE, RUIN_ZOMBIE_LEADER]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() || event != "ACCEPT" {
            return None;
        }
        Some(match ctx.player_class_id() {
            MAGE if ctx.player_level() < MIN_LEVEL => "30022-03.htm".to_string(),
            MAGE if ctx.quest_items_count(MARK_OF_FAITH) > 0 => "30022-04.htm".to_string(),
            MAGE => {
                // `ACCEPT` starts and issues the first letter in one step.
                ctx.start_quest();
                ctx.give_items(LETTER_OF_ORDER_1ST, 1);
                "30022-05.htm".to_string()
            }
            CLERIC => "30022-02a.htm".to_string(),
            _ => "30022-02.htm".to_string(),
        })
    }

    /// Ruin Zombies drop the pendant with **no roll** — but only while the
    /// necklace is out on loan and the pendant is not already held.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs()
            && ctx.is_started()
            && ctx.quest_items_count(NECKLACE_OF_MOTHER) > 0
            && ctx.quest_items_count(PENDANT_OF_MOTHER) == 0
        {
            ctx.give_items(PENDANT_OF_MOTHER, 1);
            ctx.play_sound(quest_sounds::MIDDLE);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == ZIGAUNT {
                return Some(
                    if ctx.quest_items_count(MARK_OF_FAITH) == 0 { "30022-01.htm" } else { "30022-04.htm" }
                        .to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            ZIGAUNT => self.talk_zigaunt(ctx),
            GALLINT => self.talk_gallint(ctx),
            VIVYAN => self.talk_book_giver(ctx, BOOK_OF_VIVYAN, 1, "30030-01.html", "30030-02.html"),
            TRADER_SIMPLON => {
                self.talk_book_giver(ctx, BOOK_OF_SIMPLON, SIMPLON_BOOKS, "30253-01.html", "30253-02.html")
            }
            GUARD_PRAGA => self.talk_praga(ctx),
            LIONEL => self.talk_lionel(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00405PathOfTheCleric {
    fn talk_zigaunt(&self, ctx: &mut QuestCtx) -> Option<String> {
        let has_2nd = ctx.quest_items_count(LETTER_OF_ORDER_2ND) > 0;
        let has_covenant = ctx.quest_items_count(LEMONIELLS_COVENANT) > 0;
        if has_2nd && !has_covenant {
            return Some("30022-07.html".to_string());
        }
        if has_2nd && has_covenant {
            ctx.take_items(LETTER_OF_ORDER_2ND, 1);
            ctx.take_items(LEMONIELLS_COVENANT, 1);
            ctx.give_items(MARK_OF_FAITH, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30022-09.html".to_string());
        }
        if ctx.quest_items_count(LETTER_OF_ORDER_1ST) > 0 {
            if !self.has_all_books(ctx) {
                return Some("30022-06.html".to_string());
            }
            ctx.take_items(LETTER_OF_ORDER_1ST, 1);
            ctx.give_items(LETTER_OF_ORDER_2ND, 1);
            ctx.take_items(BOOK_OF_VIVYAN, 1);
            // All three of Simplon's, one each of the others.
            ctx.take_items(BOOK_OF_SIMPLON, -1);
            ctx.take_items(BOOK_OF_PRAGA, 1);
            ctx.set_cond(3, true);
            return Some("30022-08.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    /// Vivyan and Simplon are the same shape; only the stack size differs.
    fn talk_book_giver(
        &self,
        ctx: &mut QuestCtx,
        book: i32,
        count: i64,
        given: &str,
        already: &str,
    ) -> Option<String> {
        if ctx.quest_items_count(LETTER_OF_ORDER_1ST) == 0 {
            return Some(ctx.no_quest_html());
        }
        if ctx.quest_items_count(book) > 0 {
            return Some(already.to_string());
        }
        ctx.give_items(book, count);
        if self.has_all_books(ctx) {
            ctx.set_cond(2, true);
        }
        Some(given.to_string())
    }

    fn talk_praga(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(LETTER_OF_ORDER_1ST) == 0 {
            return Some(ctx.no_quest_html());
        }
        let has_book = ctx.quest_items_count(BOOK_OF_PRAGA) > 0;
        let has_necklace = ctx.quest_items_count(NECKLACE_OF_MOTHER) > 0;
        let has_pendant = ctx.quest_items_count(PENDANT_OF_MOTHER) > 0;
        if has_book {
            return Some("30333-04.html".to_string());
        }
        if !has_necklace {
            ctx.give_items(NECKLACE_OF_MOTHER, 1);
            return Some("30333-01.html".to_string());
        }
        if !has_pendant {
            return Some("30333-02.html".to_string());
        }
        ctx.give_items(BOOK_OF_PRAGA, 1);
        ctx.take_items(PENDANT_OF_MOTHER, 1);
        ctx.take_items(NECKLACE_OF_MOTHER, 1);
        if self.has_all_books(ctx) {
            ctx.set_cond(2, true);
        }
        Some("30333-03.html".to_string())
    }

    fn talk_gallint(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(LETTER_OF_ORDER_2ND) == 0
            || ctx.quest_items_count(LEMONIELLS_COVENANT) > 0
        {
            return Some(ctx.no_quest_html());
        }
        if ctx.quest_items_count(CERTIFICATE_OF_GALLINT) == 0
            && ctx.quest_items_count(LIONELS_BOOK) > 0
        {
            ctx.take_items(LIONELS_BOOK, 1);
            ctx.give_items(CERTIFICATE_OF_GALLINT, 1);
            ctx.set_cond(5, true);
            return Some("30017-01.html".to_string());
        }
        Some("30017-02.html".to_string())
    }

    fn talk_lionel(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(LETTER_OF_ORDER_2ND) == 0 {
            return Some("30408-02.html".to_string());
        }
        let has_book = ctx.quest_items_count(LIONELS_BOOK) > 0;
        let has_cert = ctx.quest_items_count(CERTIFICATE_OF_GALLINT) > 0;
        let has_covenant = ctx.quest_items_count(LEMONIELLS_COVENANT) > 0;
        if !has_book && !has_cert && !has_covenant {
            ctx.give_items(LIONELS_BOOK, 1);
            ctx.set_cond(4, true);
            return Some("30408-01.html".to_string());
        }
        if has_book && !has_cert && !has_covenant {
            return Some("30408-03.html".to_string());
        }
        if has_cert && !has_book && !has_covenant {
            ctx.take_items(CERTIFICATE_OF_GALLINT, 1);
            ctx.give_items(LEMONIELLS_COVENANT, 1);
            ctx.set_cond(6, true);
            return Some("30408-04.html".to_string());
        }
        if has_covenant && !has_book && !has_cert {
            return Some("30408-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
