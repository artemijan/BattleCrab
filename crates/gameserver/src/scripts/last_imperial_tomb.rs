//! The Frintezza instance's player-facing hooks — the Guide (32011) who admits
//! a scroll-holder and the Teleport Cube (29061) that sends winners out — plus
//! the crawl kill notifications, all wired to [`crate::game_loop::frintezza`].
//! Port of `ai/bosses/Frintezza/LastImperialTomb`'s `onTalk`/`onKill`.

use crate::game_loop::frintezza::{self, CUBE, GUIDE, SCARLET1, SCARLET2};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::{self, sm_ids};

/// Frintezza's Magic Force Field Removal Scroll — the entry ticket.
const FRINTEZZA_SCROLL: i32 = 8073;

/// The portrait pillars (their Dewdrop suicide + demon bookkeeping) and the
/// demons they emit.
const PORTRAITS: [i32; 2] = [29048, 29049];
const DEMONS: [i32; 2] = [29050, 29051];

/// Every NPC whose death this script reacts to: the crawl monsters (advance the
/// rooms), Scarlet's final form (end the fight), and the fight's demons +
/// portraits (spawn-cap + demon-source bookkeeping).
const KILL_NPCS: &[i32] = &[
    18328, 18333, // HALL_ALARM, HALL_KEEPER_SUICIDAL_SOLDIER
    18329, 18330, 18331, 18334, 18335, 18336, 18337, 18338, 18339, // room trash
    SCARLET2, 29050, 29051, // demons
    29048, 29049, // portraits
];

/// Registered for `on_attack`: Scarlet (morphs) and the portraits (Dewdrop
/// suicide).
const ATTACK_NPCS: &[i32] = &[SCARLET1, 29048, 29049];

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
        KILL_NPCS
    }
    fn attack_npcs(&self) -> &[i32] {
        ATTACK_NPCS
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

    fn on_attack(&self, ctx: &mut QuestCtx) {
        let (npc, npc_id) = (ctx.npc, ctx.npc_id);
        if npc_id == SCARLET1 {
            frintezza::on_scarlet_attack(ctx.world, npc, npc_id);
        } else if PORTRAITS.contains(&npc_id) {
            let (attacker, skill) = (ctx.player, ctx.attack_skill_id());
            frintezza::on_portrait_attacked(ctx.world, npc, attacker, skill);
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let (killer, npc, npc_id) = (ctx.player, ctx.npc, ctx.npc_id);
        if npc_id == SCARLET2 {
            frintezza::on_scarlet_killed(ctx.world, killer);
        } else if DEMONS.contains(&npc_id) {
            frintezza::on_demon_killed(ctx.world, killer);
        } else if PORTRAITS.contains(&npc_id) {
            frintezza::on_portrait_killed(ctx.world, killer, npc);
        } else {
            frintezza::on_monster_killed(ctx.world, killer, npc_id);
        }
    }
}
