//! `ai/areas/DragonValley/CaveMaiden` — killing a Cave Maiden or Cave
//! Keeper has a 20% chance to spring a Banshee on the killer.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const CAVE_MAIDEN: i32 = 20134;
const CAVE_KEEPER: i32 = 20246;
const BANSHEE: i32 = 20412;

/// Java `addSpawn(…, 300000)` — the banshee lingers 5 minutes.
const BANSHEE_LIFE_MS: u64 = 300_000;

pub struct CaveMaiden;

impl QuestScript for CaveMaiden {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "CaveMaiden"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/DragonValley"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[CAVE_MAIDEN, CAVE_KEEPER]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.roll(100) < 20 {
            // TODO(G22): Java sets the banshee on the killing *Playable* (the
            // pet/servitor if one landed the kill); this seeds on the owner.
            if let Some(banshee) = ctx.spawn_attacker(BANSHEE, false) {
                ctx.schedule_despawn(banshee, BANSHEE_LIFE_MS);
            }
            ctx.delete_npc();
        }
    }
}
