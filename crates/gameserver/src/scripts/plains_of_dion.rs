//! `ai/areas/PlainsOfDion` — the Delu Lizardmen near Floran duel among
//! themselves; interrupt one and it calls every idle clansman in help range
//! onto you, with the appropriate indignation.

use crate::game_loop::helpers::maybe_position;
use crate::game_loop::npc::ai;
use crate::game_loop::npc::say::npc_say_param;
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::components::{Position, Vitals};
use crate::model::npc::AggroList;

const DELU_LIZARDMEN: [i32; 3] = [
    21104, // Delu Lizardman Supplier
    21105, // Delu Lizardman Special Agent
    21107, // Delu Lizardman Commander
];

/// `MONSTERS_MSG` — indices 0/1 carry the attacker's name as `$s1`.
const MONSTERS_MSG: [i32; 5] = [
    1000288, // $s1! How dare you interrupt our fight! Hey guys, help!
    1000388, // $s1! Hey! We're having a duel here!
    1000389, // The duel is over! Attack!
    1000390, // Foul! Kill the coward!
    1000391, // How dare you interrupt a sacred duel! ...
];

/// `MONSTERS_ASSIST_MSG`.
const ASSIST_MSG: [i32; 3] = [
    1000392, // Die, you coward!
    1000394, // Kill the coward!
    99702,   // What are you looking at?
];

pub struct PlainsOfDion;

impl QuestScript for PlainsOfDion {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "PlainsOfDion"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/PlainsOfDion"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn attack_npcs(&self) -> &[i32] {
        &DELU_LIZARDMEN
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.npc_script_value() != 0 {
            return;
        }
        let attacker_name = ctx
            .world
            .objects
            .get_component::<crate::model::Player>(&ctx.player)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let i = ctx.roll(5) as usize;
        if i < 2 {
            npc_say_param(ctx.world, ctx.npc, MONSTERS_MSG[i], Some(&attacker_name));
        } else {
            npc_say_param(ctx.world, ctx.npc, MONSTERS_MSG[i], None);
        }

        let help_range = ctx
            .world
            .data
            .npc_data
            .get(ctx.npc_id)
            .map(|t| t.clan_help_range)
            .unwrap_or(0);
        let Some(origin) = ctx
            .world
            .objects
            .get_component::<Position>(&ctx.npc)
            .copied()
        else {
            return;
        };
        // `forEachVisibleObjectInRange(npc, Monster, clanHelpRange)`: every
        // idle, living Delu in range and line of sight joins in.
        let self_oid = ctx.npc;
        let mut helpers: Vec<i32> = Vec::new();
        ctx.world
            .objects
            .for_each_mut::<(&crate::model::npc::Npc, &Position, &Vitals)>(|(n, p, v)| {
                if n.object_id != self_oid
                    && DELU_LIZARDMEN.contains(&n.npc_id)
                    && !v.dead
                    && origin.distance_2d(p) <= help_range as f64
                {
                    helpers.push(n.object_id);
                }
            });
        for helper in helpers {
            let idle = ctx
                .world
                .objects
                .get_component::<AggroList>(&helper)
                .is_none_or(|a| a.0.is_empty());
            if !idle {
                continue;
            }
            let sees = {
                let (hp, op) = (maybe_position(ctx.world, helper), origin);
                hp.is_some_and(|hp| {
                    ctx.world
                        .geo
                        .can_see_target(op.x, op.y, op.z, hp.x, hp.y, hp.z)
                })
            };
            if !sees {
                continue;
            }
            ai::seed_attack(ctx.world, helper, ctx.player);
            let assist = ASSIST_MSG[ctx.roll(3) as usize];
            npc_say_param(ctx.world, helper, assist, None);
        }
        ctx.set_npc_script_value(1);
    }
}
