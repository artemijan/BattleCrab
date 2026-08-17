//! Noblesse Master (`custom/NoblessMaster/NoblessMaster.java`) — a custom NPC
//! that grants nobless outright to any character at the configured level,
//! optionally handing over the Noblesse Tiara with it.
//!
//! Gated on `Custom/NoblessMaster.ini`'s `Enabled` (**True** on this dist), and
//! **reachable only if spawned**: the npc template (1003000 "Kadmos") ships in
//! `stats/npcs/custom/`, but no spawn file places it, so an untouched dist can
//! only meet him through `//spawn 1003000`. Java is in exactly the same
//! position — the script registers against an id nothing spawns.

use crate::game_loop::quests::{QuestCtx, QuestScript};

pub struct NoblessMaster;

/// Java `NoblessMaster.NOBLESS_TIARA`.
const NOBLESS_TIARA: i32 = 7694;
/// `Config.NOBLESS_MASTER_NPCID`'s dist value. The script's registration list
/// has to be a compile-time slice, so the id is fixed here rather than read
/// from the config; the *behaviour* still honours every other key, and an
/// operator who repoints `NpcId` gets a documented no-op instead of a crash.
const NOBLESS_MASTER: i32 = 1003000;

impl QuestScript for NoblessMaster {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "NoblessMaster"
    }
    fn html_dir(&self) -> &'static str {
        "custom/NoblessMaster"
    }
    fn start_npcs(&self) -> &[i32] {
        &[NOBLESS_MASTER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[NOBLESS_MASTER]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[NOBLESS_MASTER]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("1003000.htm".to_string())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // Java's `onEvent` opens with the master gate, so a disabled master
        // answers nothing at all rather than falling through to a page.
        if !ctx.world.cfg.custom_npc.nobless_master_enabled {
            return None;
        }
        if event != "noblesse" {
            return None;
        }
        let (is_noble, level) = ctx
            .world
            .objects
            .get_component::<crate::model::Player>(&ctx.player)
            .map(|p| (p.is_noble, p.level))
            .unwrap_or((false, 0));
        if is_noble {
            return Some("1003000-3.htm".to_string());
        }
        if level < ctx.world.cfg.custom_npc.nobless_master_level {
            return Some("1003000-2.htm".to_string());
        }
        if ctx.world.cfg.custom_npc.nobless_master_tiara {
            ctx.give_items(NOBLESS_TIARA, 1);
        }
        // `setNoble(true)` — the shared setter, which also grants the noble
        // skill tree and refreshes the client (`//setnoble` uses it too).
        crate::game_loop::admin::hero::set_noble(ctx.world, ctx.player, true);
        ctx.play_sound("ItemSound.quest_finish");
        Some("1003000-1.htm".to_string())
    }
}
