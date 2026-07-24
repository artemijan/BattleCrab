//! A Game of Cards (662) — `quests/Q00662_AGameOfCards`. Klump (30845) runs a
//! five-card gambling game: hunt the high-level undead/orc fields for **Red Gems**
//! (chips), stake 50 to draw a hand, flip the cards one by one, and score any
//! pairs for **Ziggo's Gemstones** and crafting materials.
//!
//! Faithful port of the Java's packed-integer state machine: the four hidden
//! cards live in the `v1` variable (`i4·10⁶ + i3·10⁴ + i2·10² + i1`) and the
//! fifth card + the reveal bitmask live in `ExMemoState` (`i9·100 + i5`, where
//! `i9`'s low 5 bits mark which cards are face-up). The pair-scoring and the
//! card-cell HTML templating are translated verbatim.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const KLUMP: i32 = 30845;
const RED_GEM: i32 = 8765;
const ZIGGOS_GEMSTONE: i32 = 8868;
const MIN_LEVEL: i32 = 61;
const REQUIRED_CHIP_COUNT: i64 = 50;

/// Chip-drop mobs: `(npc_id, value)`. Java gates the drop on
/// `value < getRandom(1000)` (so a *higher* value drops *less* often), then a
/// second always-passing `giveItemRandomly` hands over the gem.
const MONSTERS: [(i32, i32); 24] = [
    (20672, 357),
    (20673, 357),
    (20674, 583),
    (20677, 435),
    (20955, 358),
    (20958, 283),
    (20959, 455),
    (20961, 365),
    (20962, 348),
    (20965, 457),
    (20966, 493),
    (20968, 418),
    (20972, 350),
    (20973, 453),
    (21002, 315),
    (21004, 320),
    (21006, 335),
    (21008, 462),
    (21010, 397),
    (21109, 507),
    (21112, 552),
    (21114, 587),
    (21116, 812),
    (20142, 232),
];

/// Java `strip suit`: fold a 1–70 draw down to a card value 1–14.
fn strip_suit(mut v: i32) -> i32 {
    if v >= 57 {
        v -= 56;
    } else if v >= 43 {
        v -= 42;
    } else if v >= 29 {
        v -= 28;
    } else if v >= 15 {
        v -= 14;
    }
    v
}

/// Java `setHtml`: a card value's display glyph.
fn card_glyph(v: i32) -> &'static str {
    match v {
        1 => "!",
        2 => "=",
        3 => "T",
        4 => "V",
        5 => "O",
        6 => "P",
        7 => "S",
        8 => "E",
        9 => "H",
        10 => "A",
        11 => "R",
        12 => "D",
        13 => "I",
        14 => "N",
        _ => "ERROR",
    }
}

pub struct Q00662AGameOfCards;

impl Q00662AGameOfCards {
    fn chips(&self, ctx: &QuestCtx) -> i64 {
        ctx.quest_items_count(RED_GEM)
    }

    /// The five cards + reveal mask from the packed vars: `(i1..i5, i9)`.
    fn hand(&self, ctx: &QuestCtx) -> (i32, i32, i32, i32, i32, i32) {
        let v1 = ctx.get_int("v1");
        let ex = ctx.get_int("ExMemoState");
        let i5 = ex % 100;
        let i9 = ex / 100;
        let i1 = v1 % 100;
        let i2 = (v1 % 10000) / 100;
        let i3 = (v1 % 1000000) / 10000;
        let i4 = (v1 % 100000000) / 1000000;
        (i1, i2, i3, i4, i5, i9)
    }

    /// Java's per-cell `FontColor`/`Cell` templating: a face-down card is a
    /// yellow `?`, a face-up one its glyph in red. `i9`'s bit `2^(n-1)` marks
    /// card `n` face-up.
    fn render_cards(&self, base: String, cards: [i32; 5], i9: i32) -> String {
        let mut html = base;
        for (idx, &card) in cards.iter().enumerate() {
            let n = idx + 1;
            let face_up = (i9 % (1 << n)) >= (1 << idx);
            let (color, cell) = (format!("FontColor{n}"), format!("Cell{n}"));
            if face_up {
                html = html
                    .replace(&color, "FF6F6F")
                    .replace(&cell, card_glyph(card));
            } else {
                html = html.replace(&color, "FFFF00").replace(&cell, "?");
            }
        }
        html
    }

    /// Deal five distinct raw cards, fold to values, and pack the state.
    fn deal(&self, ctx: &mut QuestCtx) {
        let (mut r1, mut r2, mut r3, mut r4, mut r5) = (0, 0, 0, 0, 0);
        // Re-draw until all five raw values differ (folded values may collide —
        // that is how pairs form).
        while r1 == r2
            || r1 == r3
            || r1 == r4
            || r1 == r5
            || r2 == r3
            || r2 == r4
            || r2 == r5
            || r3 == r4
            || r3 == r5
            || r4 == r5
        {
            r1 = ctx.roll(70) + 1;
            r2 = ctx.roll(70) + 1;
            r3 = ctx.roll(70) + 1;
            r4 = ctx.roll(70) + 1;
            r5 = ctx.roll(70) + 1;
        }
        let (i1, i2, i3, i4, i5) = (
            strip_suit(r1),
            strip_suit(r2),
            strip_suit(r3),
            strip_suit(r4),
            strip_suit(r5),
        );
        ctx.set_var(
            "v1",
            (i4 * 1000000 + i3 * 10000 + i2 * 100 + i1).to_string(),
        );
        ctx.set_var("ExMemoState", i5.to_string());
        ctx.take_items(RED_GEM, REQUIRED_CHIP_COUNT);
    }

    /// Score a fully-revealed hand and pay out. Java's `i6` encodes the hand:
    /// two decimal digits count the two largest match-groups (so 40 = four of a
    /// kind, 30 = trips, 21/12 = two pair, 20 = ..., 10 = one pair, 0 = high card).
    /// Returns the result html filename.
    // `i8` (the match bookkeeping) is written on the final comparisons but not
    // read again — kept as-is to mirror the Java line-for-line.
    #[allow(unused_assignments)]
    fn score(
        &self,
        ctx: &mut QuestCtx,
        i1: i32,
        i2: i32,
        i3: i32,
        i4: i32,
        i5: i32,
    ) -> &'static str {
        let mut i6 = 0;
        let mut i8 = 0;
        if (1..=14).contains(&i1)
            && (1..=14).contains(&i2)
            && (1..=14).contains(&i3)
            && (1..=14).contains(&i4)
            && (1..=14).contains(&i5)
        {
            if i1 == i2 {
                i6 += 10;
                i8 += 8;
            }
            if i1 == i3 {
                i6 += 10;
                i8 += 4;
            }
            if i1 == i4 {
                i6 += 10;
                i8 += 2;
            }
            if i1 == i5 {
                i6 += 10;
                i8 += 1;
            }
            if (i6 % 100) < 10 {
                if (i8 % 16) < 8 {
                    if (i8 % 8) < 4 && i2 == i3 {
                        i6 += 10;
                        i8 += 4;
                    }
                    if (i8 % 4) < 2 && i2 == i4 {
                        i6 += 10;
                        i8 += 2;
                    }
                    if (i8 % 2) < 1 && i2 == i5 {
                        i6 += 10;
                        i8 += 1;
                    }
                }
            } else if (i6 % 10) == 0 && (i8 % 16) < 8 {
                if (i8 % 8) < 4 && i2 == i3 {
                    i6 += 1;
                    i8 += 4;
                }
                if (i8 % 4) < 2 && i2 == i4 {
                    i6 += 1;
                    i8 += 2;
                }
                if (i8 % 2) < 1 && i2 == i5 {
                    i6 += 1;
                    i8 += 1;
                }
            }
            if (i6 % 100) < 10 {
                if (i8 % 8) < 4 {
                    if (i8 % 4) < 2 && i3 == i4 {
                        i6 += 10;
                        i8 += 2;
                    }
                    if (i8 % 2) < 1 && i3 == i5 {
                        i6 += 10;
                        i8 += 1;
                    }
                }
            } else if (i6 % 10) == 0 && (i8 % 8) < 4 {
                if (i8 % 4) < 2 && i3 == i4 {
                    i6 += 1;
                    i8 += 2;
                }
                if (i8 % 2) < 1 && i3 == i5 {
                    i6 += 1;
                    i8 += 1;
                }
            }
            if (i6 % 100) < 10 {
                if (i8 % 4) < 2 && (i8 % 2) < 1 && i4 == i5 {
                    i6 += 10;
                    i8 += 1;
                }
            } else if (i6 % 10) == 0 && (i8 % 4) < 2 && (i8 % 2) < 1 && i4 == i5 {
                i6 += 1;
                i8 += 1;
            }
        }
        ctx.set_var("ExMemoState", "0");
        ctx.set_var("v1", "0");
        match i6 {
            40 => {
                ctx.reward_items(ZIGGOS_GEMSTONE, 43);
                ctx.reward_items(959, 3);
                ctx.reward_items(729, 1);
                "30845-13.html"
            }
            30 => {
                ctx.reward_items(959, 2);
                ctx.reward_items(951, 2);
                "30845-14.html"
            }
            21 | 12 => {
                ctx.reward_items(729, 1);
                ctx.reward_items(947, 2);
                ctx.reward_items(955, 1);
                "30845-15.html"
            }
            20 => {
                ctx.reward_items(951, 2);
                "30845-16.html"
            }
            11 => {
                ctx.reward_items(951, 1);
                "30845-17.html"
            }
            10 => {
                ctx.reward_items(956, 2);
                "30845-18.html"
            }
            _ => "30845-19.html",
        }
    }
}

impl QuestScript for Q00662AGameOfCards {
    fn id(&self) -> i32 {
        662
    }
    fn name(&self) -> &'static str {
        "Q00662_AGameOfCards"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00662_AGameOfCards"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KLUMP]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KLUMP]
    }
    fn kill_npcs(&self) -> &[i32] {
        // Materialised once; the drop table is looked up per-mob below.
        &MOB_IDS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30845-03.htm" => {
                if ctx.player_level() >= MIN_LEVEL {
                    if ctx.is_created() {
                        ctx.start_quest();
                    }
                    return Some(event.to_string());
                }
                None
            }
            "30845-06.html" | "30845-08.html" | "30845-09.html" | "30845-09a.html"
            | "30845-09b.html" | "30845-10.html" => Some(event.to_string()),
            "30845-07.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "return" => Some(
                if self.chips(ctx) < REQUIRED_CHIP_COUNT {
                    "30845-04.html"
                } else {
                    "30845-05.html"
                }
                .to_string(),
            ),
            "30845-11.html" => {
                if self.chips(ctx) >= REQUIRED_CHIP_COUNT {
                    self.deal(ctx);
                    return Some(event.to_string());
                }
                None
            }
            "turncard1" | "turncard2" | "turncard3" | "turncard4" | "turncard5" => {
                let (i1, i2, i3, i4, i5, mut i9) = self.hand(ctx);
                // Flip this card's bit if it is still face-down.
                let n: i32 = event["turncard".len()..].parse().unwrap_or(1);
                let bit = 1 << (n - 1);
                if (i9 % (bit * 2)) < bit {
                    i9 += bit;
                }
                if (i9 % 32) < 31 {
                    ctx.set_var("ExMemoState", (i9 * 100 + i5).to_string());
                }
                let result = if (i9 % 32) < 31 {
                    Some(ctx.get_htm("30845-12.html"))
                } else {
                    // All five up: score, pay out, and show the result.
                    let file = self.score(ctx, i1, i2, i3, i4, i5);
                    Some(ctx.get_htm(file))
                };
                result.map(|html| self.render_cards(html, [i1, i2, i3, i4, i5], i9))
            }
            "playagain" => Some(
                if self.chips(ctx) < REQUIRED_CHIP_COUNT {
                    "30845-21.html"
                } else {
                    "30845-20.html"
                }
                .to_string(),
            ),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Party sharing collapses to the killer (documented onKill deviation).
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let Some(&(_, value)) = MONSTERS.iter().find(|(id, _)| *id == ctx.npc_id) else {
            return;
        };
        // Java: `if (value < getRandom(1000)) giveItemRandomly(…, value, …)` —
        // the second roll always passes (value ≥ 1), so this is the whole gate.
        if value < ctx.roll(1000) {
            ctx.give_item_randomly(RED_GEM, 1, 0, value as f64, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() < MIN_LEVEL {
                    "30845-02.html"
                } else {
                    "30845-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        // A game in progress (ExMemoState set) resumes its board; otherwise the
        // chip-count prompt.
        if ctx.get_int("ExMemoState") != 0 {
            let (i1, i2, i3, i4, i5, i9) = self.hand(ctx);
            let html = ctx.get_htm("30845-11a.html");
            return Some(self.render_cards(html, [i1, i2, i3, i4, i5], i9));
        }
        Some(
            if self.chips(ctx) < REQUIRED_CHIP_COUNT {
                "30845-04.html"
            } else {
                "30845-05.html"
            }
            .to_string(),
        )
    }
}

/// The chip-drop mob ids (`addKillId`), materialised for the registry.
static MOB_IDS: [i32; 24] = [
    20672, 20673, 20674, 20677, 20955, 20958, 20959, 20961, 20962, 20965, 20966, 20968, 20972,
    20973, 21002, 21004, 21006, 21008, 21010, 21109, 21112, 21114, 21116, 20142,
];
