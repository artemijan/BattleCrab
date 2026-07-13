//! Charm teleport AI — port of
//! `dist/game/data/scripts/ai/others/TeleportWithCharm/`. Whirpy (Dwarven
//! village) and Tamil (Orc village) teleport a player holding the matching
//! gatekeeper token/charm, consuming it; without one they show the "come
//! back with a token" page. The first `ai/others`-shaped script: no quest
//! state, pure NPC behavior registered through the same `QuestRegistry`
//! (id ≤ 0 talk scripts run from the quest-window route).

use crate::game_loop::quests::{QuestCtx, QuestScript};

const WHIRPY: i32 = 30540;
const TAMIL: i32 = 30576;
const ORC_GATEKEEPER_CHARM: i32 = 1658;
const DWARF_GATEKEEPER_TOKEN: i32 = 1659;
/// Both charms teleport to Gludin (the Java script declares identical
/// locations for the two).
const TELEPORT: (i32, i32, i32) = (-80826, 149775, -3043);

pub struct TeleportWithCharm;

impl QuestScript for TeleportWithCharm {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "TeleportWithCharm"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/TeleportWithCharm"
    }
    fn start_npcs(&self) -> &[i32] {
        &[WHIRPY, TAMIL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[WHIRPY, TAMIL]
    }
    /// The dist htmls point "Use Gatekeeper Token" at the bare `Quest`
    /// bypass.
    fn bare_talk(&self) -> bool {
        true
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let token = match ctx.npc_id {
            WHIRPY => DWARF_GATEKEEPER_TOKEN,
            TAMIL => ORC_GATEKEEPER_CHARM,
            _ => return None,
        };
        if ctx.quest_items_count(token) > 0 {
            ctx.take_items(token, 1);
            ctx.teleport_to(TELEPORT.0, TELEPORT.1, TELEPORT.2);
            None
        } else {
            Some(format!("{}-01.htm", ctx.npc_id))
        }
    }
}
