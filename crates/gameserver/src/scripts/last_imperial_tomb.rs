//! The Frintezza instance's player-facing hooks — the Guide (32011) who admits
//! a scroll-holder and the Teleport Cube (29061) that sends winners out — plus
//! the crawl kill notifications, all wired to [`crate::game_loop::frintezza`].
//! Port of `ai/bosses/Frintezza/LastImperialTomb`'s `onTalk`/`onKill`.

use crate::game_loop::frintezza::{self, CUBE, GUIDE, ON_KILL_MONSTERS};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::{self, sm_ids};

/// Frintezza's Magic Force Field Removal Scroll — the entry ticket.
const FRINTEZZA_SCROLL: i32 = 8073;

pub struct LastImperialTomb;

impl QuestScript for LastImperialTomb {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "LastImperialTomb"
    }
    fn html_dir(&self) -> &'static str {
        "ai/bosses/Frintezza"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GUIDE, CUBE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[GUIDE, CUBE]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[GUIDE, CUBE]
    }
    fn kill_npcs(&self) -> &[i32] {
        ON_KILL_MONSTERS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        // Clicking is handled by `on_first_talk`; a bare `Quest` talk shows
        // nothing.
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.npc_id == GUIDE {
            // Java: enter only while carrying the removal scroll, else the
            // "not enough required items" message.
            if ctx.quest_items_count(FRINTEZZA_SCROLL) > 0 {
                let player = ctx.player;
                frintezza::try_enter(ctx.world, player);
            } else if let Some(cs) = ctx.world.clients.get(&ctx.client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::YOU_DO_NOT_HAVE_ENOUGH_REQUIRED_ITEMS,
                    &[],
                ));
            }
        } else if ctx.npc_id == CUBE {
            let player = ctx.player;
            frintezza::exit(ctx.world, player);
        }
        // Both actions teleport; no chat window is shown.
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let (killer, npc_id) = (ctx.player, ctx.npc_id);
        frintezza::on_monster_killed(ctx.world, killer, npc_id);
    }
}
