//! Newbie Guide AI — port of
//! `dist/game/data/scripts/ai/others/NewbieGuide/NewbieGuide.java`. The five
//! starter-village guides (one per race) own their chat window through
//! `addFirstTalkId`: without it the client falls back to
//! `data/html/default/<id>.htm`, which does not exist for these NPCs, so the
//! window degrades to `npcdefault.htm`'s lone "Quest" button. The real menu
//! is four entries — advice, NPC locations, support magic, quests.
//!
//! The `Quest NewbieGuide <n>` buttons walk the advice pages, suffixed `m`
//! for mage classes and `f` for everyone else.

use crate::game_loop::quests::{QuestCtx, QuestScript};

/// One guide per starter village, in Java's `NEWBIE_GUIDES` order: Talking
/// Island (Human), Elven, Dark Elf, Dwarven, Orc. Each only advises its own
/// race — the `<race>` on the template, not the id, decides.
const NEWBIE_GUIDES: &[i32] = &[30598, 30599, 30600, 30601, 30602];

/// Java `Config.MAX_NEWBIE_BUFF_LEVEL` — `Character.ini`'s
/// `MaxNewbieBuffLevel = 40` on this dist. It is `> 0`, so the guide always
/// serves the htm as-is; the `else` branch that strips the support-magic
/// button (for servers that disable newbie buffs) is unreachable here and
/// is not ported.
const MAX_NEWBIE_BUFF_LEVEL: i32 = 40;

pub struct NewbieGuide;

impl QuestScript for NewbieGuide {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "NewbieGuide"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/NewbieGuide"
    }
    fn start_npcs(&self) -> &[i32] {
        NEWBIE_GUIDES
    }
    fn talk_npcs(&self) -> &[i32] {
        NEWBIE_GUIDES
    }
    fn first_talk_npcs(&self) -> &[i32] {
        NEWBIE_GUIDES
    }

    /// `onFirstTalk`: a guide only speaks to its own race; otherwise the
    /// "go find your own people" page. A tutorial graduate (Q255 memoState 5)
    /// gets the second batch of newbie shots on their first visit.
    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.npc_race() != Some(ctx.player_race()) {
            return Some(format!("{}-no.htm", ctx.npc_id));
        }
        // Java: `Q00255_Tutorial` state exists, tutorial enabled, memoState 5
        // → memoState 6 + 200 Soulshots (or 100 Spiritshots for a non-Orc
        // mage) with the matching voice line.
        if !ctx.world.cfg.character.disable_tutorial
            && ctx.other_quest_memo_state(crate::scripts::tutorial::QUEST_NAME) == 5
        {
            ctx.set_other_quest_var(
                crate::scripts::tutorial::QUEST_NAME,
                crate::model::quest::MEMO_VAR,
                "6",
            );
            if ctx.is_in_category("MAGE_GROUP") && ctx.player_race() != 3 {
                ctx.give_items(5790, 100);
                ctx.play_tutorial_voice("tutorial_voice_027");
            } else {
                ctx.give_items(5789, 200);
                ctx.play_tutorial_voice("tutorial_voice_026");
            }
        }
        debug_assert!(MAX_NEWBIE_BUFF_LEVEL > 0);
        Some(format!("{}.htm", ctx.npc_id))
    }

    /// The guide has no quest of its own; talking to it always lands on the
    /// first-talk window.
    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onEvent`: `"0"` returns to the menu, every other event opens advice
    /// page `<npcId>-<event><m|f>.htm`.
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if event == "0" {
            return Some(format!("{}.htm", ctx.npc_id));
        }
        // Java `Player.isMageClass()` (a `ClassId` flag); `MAGE_GROUP` is
        // this port's established stand-in for it.
        let suffix = if ctx.is_in_category("MAGE_GROUP") {
            "m"
        } else {
            "f"
        };
        Some(format!("{}-{event}{suffix}.htm", ctx.npc_id))
    }
}
