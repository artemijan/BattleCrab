//! Destroy Plague Carriers (316) — port of
//! `dist/game/data/scripts/quests/Q00316_DestroyPlagueCarriers/`. Elf only:
//! Ellenia buys wererat fangs (5a) and Varool Foulclaw's fang (1000a, one
//! only, +5000 for 10+ total). The first hit on Varool Foulclaw makes him
//! shout ("Why do you oppress us so?") — the `on_attack` hook's first
//! consumer.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const ELLENIA: i32 = 30155;
const WERERAT_FANG: i32 = 1042;
const VAROOL_FOULCLAW_FANG: i32 = 1043;
const VAROOL_FOULCLAW: i32 = 27020;
const SUKAR_WERERATS: [i32; 2] = [20040, 20047];
const MIN_LEVEL: i32 = 18;
const RACE_ELF: i32 = 1;
/// NpcStringId "Why do you oppress us so?".
const WHY_DO_YOU_OPPRESS_US_SO: i32 = 31603;

pub struct Q00316DestroyPlagueCarriers;

impl QuestScript for Q00316DestroyPlagueCarriers {
    fn id(&self) -> i32 {
        316
    }
    fn name(&self) -> &'static str {
        "Q00316_DestroyPlagueCarriers"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00316_DestroyPlagueCarriers"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ELLENIA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ELLENIA]
    }
    fn kill_npcs(&self) -> &[i32] {
        const IDS: [i32; 3] = [20040, 20047, VAROOL_FOULCLAW];
        &IDS
    }
    fn attack_npcs(&self) -> &[i32] {
        &[VAROOL_FOULCLAW]
    }
    fn quest_items(&self) -> &[i32] {
        &[WERERAT_FANG, VAROOL_FOULCLAW_FANG]
    }

    /// `addCondMaxLevel(24, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 24).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30155-04.htm" if ctx.is_created() => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30155-08.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30155-09.html" => Some(event.to_string()),
            _ => None,
        }
    }

    /// Varool Foulclaw complains on the first hit (`npc.isScriptValue(0)` →
    /// NpcSay + `setScriptValue(1)`; a respawn is a fresh instance, so the
    /// shout re-arms).
    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.npc_script_value() == 0 {
            ctx.npc_say(WHY_DO_YOU_OPPRESS_US_SO);
            ctx.set_npc_script_value(1);
        }
    }

    /// `getRandomPartyMemberState(killer, -1, 3, npc)` + the
    /// `checkPartyMember` override — killer-only: started state, and
    /// Varool never drops a second fang.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        if ctx.npc_id == VAROOL_FOULCLAW {
            if ctx.quest_items_count(VAROOL_FOULCLAW_FANG) == 0 {
                ctx.give_item_randomly(VAROOL_FOULCLAW_FANG, 1, 1, 10.0 / 7.0, true);
            }
        } else if SUKAR_WERERATS.contains(&ctx.npc_id) {
            ctx.give_item_randomly(WERERAT_FANG, 1, 0, 10.0 / 5.0, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ELF {
                    "30155-00.htm"
                } else if ctx.player_level() < MIN_LEVEL {
                    "30155-02.htm"
                } else {
                    "30155-03.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let wererats = ctx.quest_items_count(WERERAT_FANG);
            let foulclaws = ctx.quest_items_count(VAROOL_FOULCLAW_FANG);
            return Some(if wererats + foulclaws > 0 {
                ctx.give_adena(
                    (wererats * 5) + (foulclaws * 1000) + if wererats + foulclaws >= 10 { 5000 } else { 0 },
                    true,
                );
                ctx.take_items(WERERAT_FANG, -1);
                ctx.take_items(VAROOL_FOULCLAW_FANG, -1);
                "30155-07.html".to_string()
            } else {
                "30155-05.html".to_string()
            });
        }
        Some(ctx.no_quest_html())
    }
}
