//! The Heart of Warding (13001) and the Teleportation Cubic (31859) — the
//! player-facing half of Antharas (`ai/bosses/Antharas`), wired to
//! `game_loop::antharas`'s entry ladder. The dist htmls already point their
//! buttons at `Quest Antharas enter` / `Quest Antharas teleportOut`, so this
//! script's **name is load-bearing**.
//!
//! The cubic has no first-talk: it speaks through `html/default/31859.htm`
//! (the ordinary default-chat path), whose button carries the named bypass.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const HEART: i32 = 13001;
const CUBE: i32 = 31859;

pub struct AntharasHeart;

impl QuestScript for AntharasHeart {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "Antharas"
    }
    fn html_dir(&self) -> &'static str {
        "ai/bosses/Antharas"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HEART, CUBE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[HEART, CUBE]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[HEART]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // The Heart's window comes from `on_first_talk`; the cubic talks
        // through `html/default/31859.htm`. Direct `Quest Antharas` talks
        // just re-show the Heart's window.
        (ctx.npc_id == HEART).then(|| "13001.html".to_string())
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.npc_id == HEART).then(|| "13001.html".to_string())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            "enter" => {
                let player = ctx.player;
                crate::game_loop::antharas::heart_enter(ctx.world, player).map(str::to_string)
            }
            "teleportOut" => {
                let player = ctx.player;
                crate::game_loop::antharas::teleport_out(ctx.world, player);
                None
            }
            _ => None,
        }
    }
}
