//! Path Of The Palus Knight (410) — port of
//! `dist/game/data/scripts/quests/Q00410_PathOfThePalusKnight/`.
//!
//! Awards the **Gaze of Abyss** (1244), one of `DarkElfChange1`'s four proofs.
//! Opens the Dark Elf tier: those four `*Change1` targets are ported but
//! proof-starved, exactly as the Elf/Human ones were before quests 401–409.
//!
//! Virgil → 13 lycanthrope skulls → Kalinta → a carapace and 5 silks → the
//! coffin → back to Virgil.
//!
//! **All three drops are unrolled.** No `getRandom` anywhere in `onKill`: every
//! kill of a gated mob pays until the cap. That is worth stating because the
//! three sibling quests in this tier *do* roll, and a reader porting 411–413
//! by analogy would add a chance that isn't here.
//!
//! ## Two redundant terms in the Java, kept readable rather than copied
//!
//! The silk branch reaches its cap and then re-tests
//! `getQuestItemsCount(SILK) >= 4` inside `== 5` — trivially true. And
//! Kalinta's second talk branch (`!has(SILK) && has(CARAPACE)`) is **dead**:
//! the branch above it is `!hasQuestItems(SILK, CARAPACE)`, i.e. *not both*,
//! which already catches carapace-only. The port collapses both, and the
//! observable page for every reachable state is unchanged — see the table in
//! `talk_kalinta`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const MASTER_VIRGIL: i32 = 30329;
const KALINTA: i32 = 30422;

const PALLUS_TALISMAN: i32 = 1237;
const LYCANTHROPE_SKULL: i32 = 1238;
const VIRGILS_LETTER: i32 = 1239;
const MORTE_TALISMAN: i32 = 1240;
const VENOMOUS_SPIDERS_CARAPACE: i32 = 1241;
const ARACHNID_TRACKER_SILK: i32 = 1242;
const COFFIN_OF_ETERNAL_REST: i32 = 1243;
const GAZE_OF_ABYSS: i32 = 1244;

const VENOMOUS_SPIDER: i32 = 20038;
const ARACHNID_TRACKER: i32 = 20043;
const LYCANTHROPE: i32 = 20049;

const DARK_FIGHTER: i32 = 31;
const PALUS_KNIGHT: i32 = 32;
const MIN_LEVEL: i32 = 19;

const SKULLS_NEEDED: i64 = 13;
const SILK_NEEDED: i64 = 5;

const QUEST_ITEMS: [i32; 7] = [
    PALLUS_TALISMAN,
    LYCANTHROPE_SKULL,
    VIRGILS_LETTER,
    MORTE_TALISMAN,
    VENOMOUS_SPIDERS_CARAPACE,
    ARACHNID_TRACKER_SILK,
    COFFIN_OF_ETERNAL_REST,
];

pub struct Q00410PathOfThePalusKnight;

impl Q00410PathOfThePalusKnight {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    /// Both of Kalinta's collectables are in hand at full count.
    fn collection_done(&self, ctx: &QuestCtx) -> bool {
        ctx.quest_items_count(ARACHNID_TRACKER_SILK) >= SILK_NEEDED
            && self.has(ctx, VENOMOUS_SPIDERS_CARAPACE)
    }
}

impl QuestScript for Q00410PathOfThePalusKnight {
    fn id(&self) -> i32 {
        410
    }
    fn name(&self) -> &'static str {
        "Q00410_PathOfThePalusKnight"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00410_PathOfThePalusKnight"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MASTER_VIRGIL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MASTER_VIRGIL, KALINTA]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[VENOMOUS_SPIDER, ARACHNID_TRACKER, LYCANTHROPE]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(
                match ctx.player_class_id() {
                    DARK_FIGHTER if ctx.player_level() < MIN_LEVEL => "30329-02.htm",
                    DARK_FIGHTER if self.has(ctx, GAZE_OF_ABYSS) => "30329-04.htm",
                    DARK_FIGHTER => "30329-05.htm",
                    PALUS_KNIGHT => "30329-02a.htm",
                    _ => "30329-03.htm",
                }
                .to_string(),
            ),
            "30329-06.htm" => {
                ctx.start_quest();
                ctx.give_items(PALLUS_TALISMAN, 1);
                Some(event.to_string())
            }
            // Skulls in, Virgil's letter out.
            "30329-10.html" => {
                if self.has(ctx, PALLUS_TALISMAN) && self.has(ctx, LYCANTHROPE_SKULL) {
                    ctx.take_items(PALLUS_TALISMAN, 1);
                    ctx.take_items(LYCANTHROPE_SKULL, -1);
                    ctx.give_items(VIRGILS_LETTER, 1);
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            // Letter in, Morte talisman out.
            "30422-02.html" => {
                if self.has(ctx, VIRGILS_LETTER) {
                    ctx.take_items(VIRGILS_LETTER, 1);
                    ctx.give_items(MORTE_TALISMAN, 1);
                    ctx.set_cond(4, true);
                    return Some(event.to_string());
                }
                None
            }
            // The full collection in, the coffin out.
            "30422-06.html" => {
                if self.has(ctx, MORTE_TALISMAN)
                    && self.has(ctx, ARACHNID_TRACKER_SILK)
                    && self.has(ctx, VENOMOUS_SPIDERS_CARAPACE)
                {
                    ctx.take_items(MORTE_TALISMAN, 1);
                    ctx.take_items(VENOMOUS_SPIDERS_CARAPACE, 1);
                    ctx.take_items(ARACHNID_TRACKER_SILK, -1);
                    ctx.give_items(COFFIN_OF_ETERNAL_REST, 1);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    /// No chance roll on any of the three — see the module header.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            LYCANTHROPE => {
                if !self.has(ctx, PALLUS_TALISMAN)
                    || ctx.quest_items_count(LYCANTHROPE_SKULL) >= SKULLS_NEEDED
                {
                    return;
                }
                ctx.give_items(LYCANTHROPE_SKULL, 1);
                if ctx.quest_items_count(LYCANTHROPE_SKULL) == SKULLS_NEEDED {
                    ctx.set_cond(2, true);
                } else {
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            VENOMOUS_SPIDER => {
                if !self.has(ctx, MORTE_TALISMAN) || self.has(ctx, VENOMOUS_SPIDERS_CARAPACE) {
                    return;
                }
                ctx.give_items(VENOMOUS_SPIDERS_CARAPACE, 1);
                // Java plays no item sound on this one — only the cond when
                // the silks are already done.
                if self.collection_done(ctx) {
                    ctx.set_cond(5, true);
                }
            }
            ARACHNID_TRACKER => {
                if !self.has(ctx, MORTE_TALISMAN)
                    || ctx.quest_items_count(ARACHNID_TRACKER_SILK) >= SILK_NEEDED
                {
                    return;
                }
                ctx.give_items(ARACHNID_TRACKER_SILK, 1);
                if ctx.quest_items_count(ARACHNID_TRACKER_SILK) == SILK_NEEDED {
                    // Java re-tests `silk >= 4` here, trivially true at 5.
                    if self.has(ctx, VENOMOUS_SPIDERS_CARAPACE) {
                        ctx.set_cond(5, true);
                    }
                } else {
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == MASTER_VIRGIL {
                return Some("30329-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            MASTER_VIRGIL => self.talk_virgil(ctx),
            KALINTA => self.talk_kalinta(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00410PathOfThePalusKnight {
    fn talk_virgil(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, PALLUS_TALISMAN) {
            let skulls = ctx.quest_items_count(LYCANTHROPE_SKULL);
            return Some(
                match skulls {
                    0 => "30329-07.html",
                    n if n < SKULLS_NEEDED => "30329-08.html",
                    _ => "30329-09.html",
                }
                .to_string(),
            );
        }
        if self.has(ctx, COFFIN_OF_ETERNAL_REST) {
            ctx.give_items(GAZE_OF_ABYSS, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30329-11.html".to_string());
        }
        if self.has(ctx, VIRGILS_LETTER) || self.has(ctx, MORTE_TALISMAN) {
            return Some("30329-12.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    /// Java's chain, with its dead second branch removed. Reachable states:
    ///
    /// | Holding | Page |
    /// |---|---|
    /// | Virgil's letter | `30422-01` |
    /// | Morte + not both collectables | `30422-03` |
    /// | Morte + both, silk < 5 | `30422-04` |
    /// | Morte + both, silk ≥ 5 | `30422-05` |
    /// | the coffin | `30422-06` |
    fn talk_kalinta(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, VIRGILS_LETTER) {
            return Some("30422-01.html".to_string());
        }
        if self.has(ctx, MORTE_TALISMAN) {
            let both =
                self.has(ctx, ARACHNID_TRACKER_SILK) && self.has(ctx, VENOMOUS_SPIDERS_CARAPACE);
            if !both {
                return Some("30422-03.html".to_string());
            }
            return Some(
                if self.collection_done(ctx) {
                    "30422-05.html"
                } else {
                    "30422-04.html"
                }
                .to_string(),
            );
        }
        if self.has(ctx, COFFIN_OF_ETERNAL_REST) {
            return Some("30422-06.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
