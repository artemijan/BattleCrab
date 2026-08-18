//! The player side of Baium (`ai/bosses/Baium`): the **Angelic Vortex** (31862)
//! that ferries a Blooded-Fabric bearer into the lair, the **stone statue**
//! (29025) they wake once inside, and the **teleport cube** (31842) that scatters
//! survivors back to the surface. Every button routes through `Quest Baium
//! <event>`, so this script's **name is load-bearing**.
//!
//! The boss-side machinery (spawn/awakening/combat) lives in
//! [`baium`].

use crate::game_loop::baium::{self, ANG_VORTEX, BAIUM_STONE, FABRIC, TELE_CUBE};
use crate::game_loop::quests::{QuestCtx, QuestScript};

pub struct Baium;

impl QuestScript for Baium {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "Baium"
    }
    fn html_dir(&self) -> &'static str {
        "ai/bosses/Baium"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ANG_VORTEX, TELE_CUBE, BAIUM_STONE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ANG_VORTEX, TELE_CUBE, BAIUM_STONE]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        // Only the vortex has a script-dir landing page. The cube and the statue
        // use their `data/html/default/` htmls, served by the core.
        &[ANG_VORTEX]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.npc_id == ANG_VORTEX {
            return Some("31862.html".to_string());
        }
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            // The statue's "Wake Baium" button.
            "wakeUp" => {
                baium::wake_up(ctx.world, ctx.npc, ctx.player);
                None
            }
            // The vortex: cross into the lair if the fight allows it and the
            // entrant carries a Blooded Fabric.
            "enter" => match baium::entry_outcome(ctx.world, ctx.quest_items_count(FABRIC) > 0) {
                baium::EntryOutcome::Dead => Some("31862-03.html".to_string()),
                baium::EntryOutcome::InFight => Some("31862-02.html".to_string()),
                baium::EntryOutcome::NoFabric => Some("31862-01.html".to_string()),
                baium::EntryOutcome::Admitted => {
                    ctx.take_items(FABRIC, 1);
                    let (x, y, z) = baium::teleport_in_loc();
                    ctx.teleport_to(x, y, z);
                    None
                }
            },
            // The teleport cube: back to the surface, scattered.
            "teleportOut" => {
                let (x, y, z) = baium::random_exit(ctx.world);
                ctx.teleport_to(x, y, z);
                None
            }
            // The vortex's static lore page.
            e if e.ends_with(".html") => Some(e.to_string()),
            _ => None,
        }
    }
}
