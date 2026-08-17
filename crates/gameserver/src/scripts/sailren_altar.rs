//! Shilen's Stone Statue (32109) and the Teleport Cube (32107) — the player
//! side of Sailren (`ai/bosses/Sailren`). The Statue admits a Gazkh-bearing
//! party leader into the wave fight ([`crate::game_loop::sailren`]); the cube
//! sends survivors home. The dist htmls point their buttons at `Quest Sailren
//! <event>`, so this script's **name is load-bearing**.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::game_loop::sailren::{self, STATUE};

const CUBIC: i32 = 32107;
/// Gazkh — the entry ticket, consumed on a successful start.
const GAZKH: i32 = 8784;
/// Java `TeleportWhereType.TOWN` for the nest → Rune Township, the nearest town.
const TOWN_EXIT: (i32, i32, i32) = (43835, -47749, -792);

pub struct SailrenAltar;

impl QuestScript for SailrenAltar {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "Sailren"
    }
    fn html_dir(&self) -> &'static str {
        "ai/bosses/Sailren"
    }
    fn start_npcs(&self) -> &[i32] {
        &[STATUE, CUBIC]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[STATUE, CUBIC]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[STATUE, CUBIC]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.npc_id == STATUE {
            return Some("32109.html".to_string());
        }
        // The teleport cube: talking to it sends you home.
        ctx.teleport_to(TOWN_EXIT.0, TOWN_EXIT.1, TOWN_EXIT.2);
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            "enter" => {
                // Refused? Show the reason. Otherwise take the Gazkh and let the
                // leader's nearby party in (the wave arms 60 s later).
                if let Some(refusal) = sailren::entry_refusal(ctx.world, ctx.player) {
                    return Some(refusal.to_string());
                }
                ctx.take_items(GAZKH, 1);
                let leader = ctx.player;
                sailren::enter_party(ctx.world, leader);
                None
            }
            "teleportOut" => {
                ctx.teleport_to(TOWN_EXIT.0, TOWN_EXIT.1, TOWN_EXIT.2);
                None
            }
            // The static requirement/method pages the Statue menu links to.
            e if e.ends_with(".html") => Some(e.to_string()),
            _ => None,
        }
    }
}
