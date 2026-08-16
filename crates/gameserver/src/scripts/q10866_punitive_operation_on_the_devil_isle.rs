//! Punitive Operation on the Devil's Isle (10866) —
//! `quests/Q10866_PunitiveOperationOnTheDevilIsle`.
//!
//! A level 70+ courier run with no kills and no items: Rodemai in Giran sends
//! you to Ein, who sends you to Fethin at the harbour, who sends you to Nikia
//! on the Devil's Isle. Nikia pays 150 000 XP, 4 500 SP and 13 136 adena.
//!
//! Each step is a plain talk, so the whole quest is four html pages and three
//! `setCond` calls. The reward branch checks `isStarted()` again before
//! paying, which is Java's guard against a forged `34020-02.html` bypass from
//! someone who never took the quest.
//!
//! SKIP(census): Java overrides `checkPartyMember` (a started member is a
//! valid party target for `getRandomPartyMember`), but nothing in this quest
//! ever calls a party-member helper — it has no `onKill` at all. The override
//! is dead weight there and has nothing to port.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const RODEMAI: i32 = 30756;
const EIN: i32 = 34017;
const FETHIN: i32 = 34019;
const NIKIA: i32 = 34020;

const MIN_LEVEL: i32 = 70;

pub struct Q10866PunitiveOperationOnTheDevilIsle;

impl QuestScript for Q10866PunitiveOperationOnTheDevilIsle {
    fn id(&self) -> i32 {
        10866
    }
    fn name(&self) -> &'static str {
        "Q10866_PunitiveOperationOnTheDevilIsle"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q10866_PunitiveOperationOnTheDevilIsle"
    }
    fn start_npcs(&self) -> &[i32] {
        &[RODEMAI]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[RODEMAI, EIN, FETHIN, NIKIA]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30756-02.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "34017-02.html" => {
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "34019-02.html" => {
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            "34020-02.html" => {
                // The second `isStarted()` — a forged bypass from a player who
                // never started the quest gets nothing rather than the reward.
                if ctx.is_started() {
                    ctx.add_exp_and_sp(150_000, 4_500);
                    ctx.give_adena(13_136, true);
                    ctx.exit_quest(false, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // Java's `getQuestState(player, true)` — talking creates the state, so
        // `isCreated()` below is reached on the very first click.
        ctx.ensure_qs();
        let html = if ctx.is_created() {
            // The level gate is inside `onTalk`, not an `addCond*`: an
            // under-level player still gets a page, just a different one.
            if ctx.player_level() >= MIN_LEVEL {
                "30756-01.htm"
            } else {
                "no_lvl.html"
            }
        } else if ctx.is_started() {
            match ctx.npc_id {
                RODEMAI if ctx.is_cond(1) => "30756-02.html",
                EIN if ctx.is_cond(1) => "34017-01.html",
                EIN if ctx.is_cond(2) => "34017-02.html",
                FETHIN if ctx.is_cond(2) => "34019-01.html",
                FETHIN if ctx.is_cond(3) => "34019-02.html",
                NIKIA if ctx.is_cond(3) => "34020-01.html",
                _ => return Some(ctx.no_quest_html()),
            }
        } else if ctx.is_completed() && ctx.npc_id == RODEMAI {
            return Some(ctx.already_completed_html());
        } else {
            return Some(ctx.no_quest_html());
        };
        Some(html.to_string())
    }
}
