//! `ai/areas/PaganTemple/PaganKeys` — the temple key drops: Zombie Workers
//! drop the Anteroom Key, Triol's Laypersons the Chapel Key, Triol's
//! Priests the Key of Darkness — 10% each, honoring `AutoLoot`.

use crate::game_loop::items::ground_items::{LOOT_PROTECTION_TICKS, reserve_for};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::game_loop::space::position::pos_of;

const ANTEROOM_KEY: i32 = 8273;
const CHAPEL_KEY: i32 = 8274;
const KEY_OF_DARKNESS: i32 = 8275;

const ZOMBIE_WORKER: i32 = 22140;
const TRIOLS_LAYPERSON: i32 = 22142;
const TRIOLS_PRIEST: i32 = 22168;

const KEY_CHANCE: i32 = 10;

pub struct PaganKeys;

impl QuestScript for PaganKeys {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "PaganKeys"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/PaganTemple"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[ZOMBIE_WORKER, TRIOLS_LAYPERSON, TRIOLS_PRIEST]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let key = match ctx.npc_id {
            ZOMBIE_WORKER => ANTEROOM_KEY,
            TRIOLS_LAYPERSON => CHAPEL_KEY,
            TRIOLS_PRIEST => KEY_OF_DARKNESS,
            _ => return,
        };
        if ctx.roll(100) >= KEY_CHANCE {
            return;
        }
        if ctx.world.cfg.character.auto_loot {
            ctx.give_items(key, 1);
            return;
        }
        // Java `npc.dropItem(killer, key, 1)` — toss it on the corpse with
        // the killer's 15 s pickup protection (the death-drop rules).
        let Some((x, y, z)) = pos_of(ctx.world, ctx.npc) else {
            return;
        };
        let npc_oid = ctx.npc;
        let player = ctx.player;
        let ground_oid = crate::game_loop::items::ground_items::spawn_ground_item(
            ctx.world,
            key,
            1,
            0,
            x,
            y,
            z,
            npc_oid,
            crate::game_loop::items::ground_items::DropSource::Npc,
        );
        reserve_for(ctx.world, ground_oid, player, LOOT_PROTECTION_TICKS);
    }
}
