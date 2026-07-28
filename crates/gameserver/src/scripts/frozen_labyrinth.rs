//! `ai/areas/FrozenLabyrinth` — striking a Pronghorn or Frost Buffalo with
//! a physical *skill* shatters it into six spirit/lost copies that swarm
//! the attacker.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const PRONGHORN_SPIRIT: i32 = 22087;
const PRONGHORN: i32 = 22088;
const LOST_BUFFALO: i32 = 22093;
const FROST_BUFFALO: i32 = 22094;

pub struct FrozenLabyrinth;

impl QuestScript for FrozenLabyrinth {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FrozenLabyrinth"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/FrozenLabyrinth"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[PRONGHORN, FROST_BUFFALO]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.npc_script_value() != 0 {
            return;
        }
        // Java: `(skill != null) && !skill.isMagic()` — a physical skill blow.
        let Some(skill_id) = ctx.attack_skill_id() else {
            return;
        };
        let physical = ctx
            .world
            .data
            .skill_data
            .get(skill_id, 1)
            .is_some_and(|s| s.magic_type != 1);
        if !physical {
            return;
        }
        let npc_id = ctx.npc_id;
        let Some(pos) = ctx
            .world
            .objects
            .get_component::<crate::model::components::Position>(&ctx.npc)
            .map(|p| (p.x, p.y, p.z, p.heading))
        else {
            return;
        };
        let spawn_id = if npc_id == PRONGHORN {
            PRONGHORN_SPIRIT
        } else {
            LOST_BUFFALO
        };
        // Java's `diff` walk: six copies staggered east then north.
        let mut diff = 0;
        for _ in 0..6 {
            let x = if diff < 60 { pos.0 + diff } else { pos.0 };
            let y = if diff >= 60 {
                pos.1 + (diff - 40)
            } else {
                pos.1
            };
            ctx.spawn_attacker_at(spawn_id, x, y, pos.2);
            diff += 20;
        }
        ctx.set_npc_script_value(1);
        ctx.delete_npc();
    }
}
