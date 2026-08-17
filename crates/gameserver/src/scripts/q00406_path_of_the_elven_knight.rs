//! Path Of The Elven Knight (406) — port of
//! `dist/game/data/scripts/quests/Q00406_PathOfTheElvenKnight/`.
//!
//! The first-occupation quest that awards the **Elven Knight Brooch** (1204),
//! which `ElfHumanFighterChange1` consumes to make an Elven Fighter an Elven
//! Knight. Until this landed the brooch had no source in the port, so that
//! transfer was unreachable outside `//setclass`.
//!
//! Master Sorius sends you for 20 topaz pieces, then to Blacksmith Kluto, who
//! wants 20 emerald pieces from Ol Mahum Novices before he hands over the box
//! the brooch comes in.
//!
//! **The drop is hand-rolled, and that is deliberate.** Most collect quests
//! call `giveItemRandomly`, which multiplies both chance and amount by
//! `RateQuestDrop`. This one does its own `getRandom(100) < chance` plus a
//! plain `giveItems`, so it is **not** rate-multiplied — on a server with
//! `RateQuestDrop != 1` the two paths diverge. Reaching for the convenience
//! helper here would have silently changed the drop rate, so the roll is
//! written out to match Java.
//!
//! Note the page extensions are **mixed within one quest**: the pre-accept
//! dialog is `.htm` and everything after it is `.html`. Copied exactly.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const MASTER_SORIUS: i32 = 30327;
const BLACKSMITH_KLUTO: i32 = 30317;

const SORIUS_LETTER: i32 = 1202;
const KLUTO_BOX: i32 = 1203;
const ELVEN_KNIGHT_BROOCH: i32 = 1204;
const TOPAZ_PIECE: i32 = 1205;
const EMERALD_PIECE: i32 = 1206;
const KLUTO_MEMO: i32 = 1276;

const ELVEN_FIGHTER: i32 = 18;
const ELVEN_KNIGHT: i32 = 19;
const MIN_LEVEL: i32 = 19;
const REQUIRED: i64 = 20;

const OL_MAHUM_NOVICE: i32 = 20782;
/// Tracker Skeleton, its leader, Skeleton Scout/Bowman, Ruin Spartoi,
/// Salamander Noble — all 70% topaz. Ol Mahum Novice drops emerald at 50%.
const TOPAZ_MOBS: [i32; 6] = [20035, 20042, 20045, 20051, 20054, 20060];
const KILL_NPCS: [i32; 7] = [20035, 20042, 20045, 20051, 20054, 20060, OL_MAHUM_NOVICE];

const QUEST_ITEMS: [i32; 5] = [
    SORIUS_LETTER,
    KLUTO_BOX,
    TOPAZ_PIECE,
    EMERALD_PIECE,
    KLUTO_MEMO,
];

pub struct Q00406PathOfTheElvenKnight;

impl QuestScript for Q00406PathOfTheElvenKnight {
    fn id(&self) -> i32 {
        406
    }
    fn name(&self) -> &'static str {
        "Q00406_PathOfTheElvenKnight"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00406_PathOfTheElvenKnight"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MASTER_SORIUS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MASTER_SORIUS, BLACKSMITH_KLUTO]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == MASTER_SORIUS {
                return Some("30327-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            // Completed (or anything else) falls through to the no-quest msg.
            return Some(ctx.no_quest_html());
        }
        match npc {
            MASTER_SORIUS => self.talk_sorius(ctx),
            BLACKSMITH_KLUTO => self.talk_kluto(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(
                match ctx.player_class_id() {
                    ELVEN_FIGHTER if ctx.player_level() < MIN_LEVEL => "30327-03.htm",
                    // Already holding the brooch — nothing to earn.
                    ELVEN_FIGHTER if ctx.quest_items_count(ELVEN_KNIGHT_BROOCH) > 0 => {
                        "30327-04.htm"
                    }
                    ELVEN_FIGHTER => "30327-05.htm",
                    ELVEN_KNIGHT => "30327-02a.htm",
                    _ => "30327-02.htm",
                }
                .to_string(),
            ),
            "30327-06.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30317-02.html" => {
                ctx.take_items(SORIUS_LETTER, 1);
                if ctx.quest_items_count(KLUTO_MEMO) == 0 {
                    ctx.give_items(KLUTO_MEMO, 1);
                }
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    /// Java's `onKill`. The two halves share one body but differ in which item
    /// gates them: the topaz mobs pay out only *before* the Kluto box exists,
    /// the Ol Mahum Novice only *while* you carry Kluto's memo.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        let (reward, chance, cond, allowed) = if npc_id == OL_MAHUM_NOVICE {
            (EMERALD_PIECE, 50, 5, ctx.quest_items_count(KLUTO_MEMO) > 0)
        } else if TOPAZ_MOBS.contains(&npc_id) {
            (TOPAZ_PIECE, 70, 2, ctx.quest_items_count(KLUTO_BOX) == 0)
        } else {
            return;
        };
        if !allowed || ctx.quest_items_count(reward) >= REQUIRED {
            return;
        }
        // `getRandom(100) < chance` — deliberately not `give_item_randomly`,
        // which would apply RateQuestDrop (see the module header).
        if ctx.roll(100) >= chance {
            return;
        }
        ctx.give_items(reward, 1);
        if ctx.quest_items_count(reward) == REQUIRED {
            ctx.set_cond(cond, true);
        } else {
            ctx.play_sound(crate::network::server_packets::quest_sounds::ITEMGET);
        }
    }
}

impl Q00406PathOfTheElvenKnight {
    fn talk_sorius(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(KLUTO_BOX) > 0 {
            // The box is the finish line.
            if ctx.quest_items_count(ELVEN_KNIGHT_BROOCH) == 0 {
                ctx.give_items(ELVEN_KNIGHT_BROOCH, 1);
            }
            // Java branches on level 20 / 19 / below and awards the *same*
            // exp and sp in all three — collapsed, not a dropped case.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30327-10.html".to_string());
        }
        let topaz = ctx.quest_items_count(TOPAZ_PIECE);
        let has_letter = ctx.quest_items_count(SORIUS_LETTER) > 0;
        let has_memo = ctx.quest_items_count(KLUTO_MEMO) > 0;
        if topaz == 0 {
            return Some("30327-07.html".to_string());
        }
        if topaz < REQUIRED {
            return Some("30327-08.html".to_string());
        }
        // 20 topaz and neither of Kluto's papers yet: hand over the letter.
        if !has_letter && !has_memo {
            ctx.give_items(SORIUS_LETTER, 1);
            ctx.set_cond(3, true);
            return Some("30327-09.html".to_string());
        }
        Some("30327-11.html".to_string())
    }

    fn talk_kluto(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(KLUTO_BOX) > 0 {
            return Some("30317-06.html".to_string());
        }
        let topaz = ctx.quest_items_count(TOPAZ_PIECE);
        let emerald = ctx.quest_items_count(EMERALD_PIECE);
        let has_letter = ctx.quest_items_count(SORIUS_LETTER) > 0;
        let has_memo = ctx.quest_items_count(KLUTO_MEMO) > 0;
        if has_letter && topaz >= REQUIRED {
            return Some("30317-01.html".to_string());
        }
        if !has_memo || topaz < REQUIRED {
            return Some(ctx.no_quest_html());
        }
        if emerald == 0 {
            return Some("30317-03.html".to_string());
        }
        if emerald < REQUIRED {
            return Some("30317-04.html".to_string());
        }
        ctx.give_items(KLUTO_BOX, 1);
        ctx.take_items(TOPAZ_PIECE, -1);
        ctx.take_items(EMERALD_PIECE, -1);
        ctx.take_items(KLUTO_MEMO, 1);
        ctx.set_cond(6, true);
        Some("30317-05.html".to_string())
    }
}
