//! `ai/areas/BeastFarm/FeedableBeasts` — the classic Beast Farm: sprinkle
//! Golden (6643, skill 2188) or Crystal Spice (6644, skill 2189) on an
//! Alpen Kookaburra / Buffalo / Cougar and it may grow; at the top of the
//! chain it either becomes a tamed beast that follows its feeder, a plain
//! top-stage animal — or a "mad cow" that turns on the hand that fed it.
//!
//! The sibling `BeastFarm.java` (Gracia spice revamp, NPCs 18874+) never
//! spawns on this dist — dead content, not ported.

use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::components::RegionCell;

const GOLDEN_SPICE: i32 = 6643;
const CRYSTAL_SPICE: i32 = 6644;
const SKILL_GOLDEN_SPICE: i32 = 2188;
const SKILL_CRYSTAL_SPICE: i32 = 2189;

pub const TAMED_BEASTS: [i32; 6] = [16013, 16014, 16015, 16016, 16017, 16018];

/// Everything that can eat, all three species plus the six mad cows —
/// and the tamed beasts, which keep eating to stay (Java feeds them through
/// the auto-feed trigger cast; the port routes both through skill-see).
const FEEDABLE_BEASTS: [i32; 69] = [
    16013, 16014, 16015, 16016, 16017, 16018, // tamed beasts
    21451, 21452, 21453, 21454, 21455, 21456, 21457, 21458, 21459, 21460, 21461, 21462, 21463,
    21464, 21465, 21466, 21467, 21468, 21469, // Alpen Kookaburra
    21470, 21471, 21472, 21473, 21474, 21475, 21476, 21477, 21478, 21479, 21480, 21481, 21482,
    21483, 21484, 21485, 21486, 21487, 21488, // Alpen Buffalo
    21489, 21490, 21491, 21492, 21493, 21494, 21495, 21496, 21497, 21498, 21499, 21500, 21501,
    21502, 21503, 21504, 21505, 21506, 21507, // Alpen Cougar
    21824, 21825, 21826, 21827, 21828, 21829, // mad cows
];

/// Mad cow → the plain top-stage animal it reverts to after 10 s.
pub(crate) fn mad_cow_reverts_to(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        21824 => 21468,
        21825 => 21469,
        21826 => 21487,
        21827 => 21488,
        21828 => 21506,
        21829 => 21507,
        _ => return None,
    })
}

/// The chatter tables (client `NpcStringId`s). `TEXT[growth]`.
const TEXT_0: [i32; 10] = [2114, 2005, 2006, 2007, 2008, 2009, 2010, 2011, 2012, 2013];
const TEXT_1: [i32; 5] = [2014, 2015, 2016, 2017, 2018];
const TEXT_2: [i32; 5] = [2019, 2020, 2021, 2022, 2023];
/// `TAMED_TEXT` — 2024–2028 carry the feeder's name as `$s1`.
const TAMED_TEXT: [i32; 8] = [2024, 2025, 2026, 2027, 2028, 2029, 2030, 2031];

/// One growth stage's outcome table for one spice: `rows[0]` = untamed
/// {normal, mad cow}, `rows[1]` = tamed {fighter's beast, mage's beast}
/// (level-2 tables only; earlier levels have a single row of next stages).
struct Table {
    row0: &'static [i32],
    row1: &'static [i32],
}

struct Growth {
    level: i32,
    chance: i32,
    gold: Option<Table>,
    crystal: Option<Table>,
}

impl Growth {
    fn table(&self, food: i32) -> Option<&Table> {
        match food {
            GOLDEN_SPICE => self.gold.as_ref(),
            CRYSTAL_SPICE => self.crystal.as_ref(),
            _ => None,
        }
    }
}

/// The Java `GROWTH_CAPABLE_MOBS` map as a lookup. Bases: Kookaburra 21451,
/// Buffalo 21470, Cougar 21489; tamed pairs (fighter, mage): Buffalo
/// 16013/16014, Cougar 16015/16016, Kookaburra 16017/16018. Note the
/// verbatim Buffalo quirk: `Buffalo_1_Gold_2` is `{21481, 21482}` (21481
/// appears in both gold tables) — Java ships it that way.
fn growth_of(npc_id: i32) -> Option<Growth> {
    let g = |level, chance, gold, crystal| {
        Some(Growth {
            level,
            chance,
            gold,
            crystal,
        })
    };
    let t = |row0: &'static [i32]| Some(Table { row0, row1: &[] });
    let t2 = |row0: &'static [i32], row1: &'static [i32]| Some(Table { row0, row1 });
    match npc_id {
        // --- Alpen Kookaburra ---
        21451 => g(
            0,
            100,
            t(&[21452, 21453, 21454, 21455]),
            t(&[21456, 21457, 21458, 21459]),
        ),
        21452 | 21454 => g(1, 40, t(&[21460, 21462]), None),
        21453 | 21455 => g(1, 40, t(&[21461, 21463]), None),
        21456 | 21458 => g(1, 40, None, t(&[21464, 21466])),
        21457 | 21459 => g(1, 40, None, t(&[21465, 21467])),
        21460 | 21462 => g(2, 25, t2(&[21468, 21824], &[16017, 16018]), None),
        21461 | 21463 => g(2, 25, t2(&[21469, 21825], &[16017, 16018]), None),
        21464 | 21466 => g(2, 25, None, t2(&[21468, 21824], &[16017, 16018])),
        21465 | 21467 => g(2, 25, None, t2(&[21469, 21825], &[16017, 16018])),
        // --- Alpen Buffalo ---
        21470 => g(
            0,
            100,
            t(&[21471, 21472, 21473, 21474]),
            t(&[21475, 21476, 21477, 21478]),
        ),
        21471 | 21473 => g(1, 40, t(&[21479, 21481]), None),
        21472 | 21474 => g(1, 40, t(&[21481, 21482]), None), // Java's asymmetric table
        21475 | 21477 => g(1, 40, None, t(&[21483, 21485])),
        21476 | 21478 => g(1, 40, None, t(&[21484, 21486])),
        21479 | 21481 => g(2, 25, t2(&[21487, 21826], &[16013, 16014]), None),
        21480 | 21482 => g(2, 25, t2(&[21488, 21827], &[16013, 16014]), None),
        21483 | 21485 => g(2, 25, None, t2(&[21487, 21826], &[16013, 16014])),
        21484 | 21486 => g(2, 25, None, t2(&[21488, 21827], &[16013, 16014])),
        // --- Alpen Cougar ---
        21489 => g(
            0,
            100,
            t(&[21490, 21491, 21492, 21493]),
            t(&[21494, 21495, 21496, 21497]),
        ),
        21490 | 21492 => g(1, 40, t(&[21498, 21500]), None),
        21491 | 21493 => g(1, 40, t(&[21499, 21501]), None),
        21494 | 21496 => g(1, 40, None, t(&[21502, 21504])),
        21495 | 21497 => g(1, 40, None, t(&[21503, 21505])),
        21498 | 21500 => g(2, 25, t2(&[21506, 21828], &[16015, 16016]), None),
        21499 | 21501 => g(2, 25, t2(&[21507, 21829], &[16015, 16016]), None),
        21502 | 21504 => g(2, 25, None, t2(&[21506, 21828], &[16015, 16016])),
        21503 | 21505 => g(2, 25, None, t2(&[21507, 21829], &[16015, 16016])),
        _ => None,
    }
}

pub struct FeedableBeasts;

impl QuestScript for FeedableBeasts {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FeedableBeasts"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/BeastFarm"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn skill_see_npcs(&self) -> &[i32] {
        &FEEDABLE_BEASTS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_skill_see(&self, ctx: &mut QuestCtx, skill_id: i32) {
        if skill_id != SKILL_GOLDEN_SPICE && skill_id != SKILL_CRYSTAL_SPICE {
            return;
        }
        let npc_id = ctx.npc_id;
        // Tamed beasts eat too — the spice extends their stay.
        if let Some(t) = ctx
            .world
            .objects
            .get_component::<crate::model::components::TamedBeastOf>(&ctx.npc)
            .copied()
        {
            if skill_id == t.food_skill {
                let beast = ctx.npc;
                crate::game_loop::tamed_beast::on_receive_food(ctx.world, beast);
                let msg = TAMED_TEXT[ctx.roll(TAMED_TEXT.len() as i32) as usize];
                bark(ctx, beast, msg);
            }
            return;
        }

        let growth = growth_of(npc_id);
        // Feedables not in the growth map are max growth (3): they eat and
        // nothing more.
        let growth_level = growth.as_ref().map(|x| x.level).unwrap_or(3);

        // Lock a 0-growth beast to its first feeder (Java's dual-feeder
        // exploit guard: while locked it will not grow for anyone).
        if growth_level == 0 && ctx.npc_var_int("feeder") != 0 {
            return;
        }
        ctx.set_npc_var_int("feeder", ctx.player);

        let food = if skill_id == SKILL_GOLDEN_SPICE {
            GOLDEN_SPICE
        } else {
            CRYSTAL_SPICE
        };

        // The beast visibly eats.
        let eater = ctx.npc;
        social(ctx, eater, 2);

        let Some(growth) = growth else {
            return;
        };
        // Wrong spice for this stage: consumed, no effect.
        if growth.table(food).is_none() {
            return;
        }
        // Rare random talk.
        if ctx.roll(20) == 0 {
            let table: &[i32] = match growth_level {
                0 => &TEXT_0,
                1 => &TEXT_1,
                _ => &TEXT_2,
            };
            let msg = table[ctx.roll(table.len() as i32) as usize];
            bark(ctx, eater, msg);
        }
        if ctx.roll(100) < growth.chance {
            spawn_next(ctx, &growth, food);
        }
    }
}

/// `spawnNext`: pick the next stage, replace the beast, and either hand the
/// player a tamed beast or set the newcomer on them.
fn spawn_next(ctx: &mut QuestCtx, growth: &Growth, food: i32) {
    if ctx.npc_script_value() == 1 {
        return;
    }
    ctx.set_npc_script_value(1);
    let Some(table) = growth.table(food) else {
        return;
    };

    let next_id = if growth.level == 2 {
        if ctx.roll(2) == 0 {
            // Tamed — which of the pair depends on the feeder's calling.
            let mage = ctx.is_in_category("MAGE_GROUP");
            table.row1[usize::from(mage)]
        } else if ctx.roll(5) == 0 {
            table.row0[1] // the mad cow
        } else {
            table.row0[0]
        }
    } else {
        table.row0[ctx.roll(table.row0.len() as i32) as usize]
    };

    let Some((x, y, z)) = ctx
        .world
        .objects
        .get_component::<crate::model::components::Position>(&ctx.npc)
        .map(|p| (p.x, p.y, p.z))
    else {
        return;
    };
    ctx.delete_npc();

    if TAMED_BEASTS.contains(&next_id) {
        let food_skill = food - (GOLDEN_SPICE - SKILL_GOLDEN_SPICE);
        let player = ctx.player;
        let beast = crate::game_loop::tamed_beast::spawn_tamed_beast(
            ctx.world, next_id, player, food_skill, x, y, z,
        );
        // SKIP(off-chronicle): Java's quest hooks here — 20 (Bring Up With
        // Love) and 655 (A Grand Plan for Taming Wild Beasts) — are both
        // commented out in the reference (the `Q00020…` / `Q00655…` hooks),
        // and neither quest exists anywhere in this datapack. Dead code even
        // in Java; revive only if those quests ever ship here.
        if let (Some(beast), 0) = (beast, ctx.roll(20)) {
            // A rare word from the newly tamed friend ($s1 = the tamer).
            let msg = TAMED_TEXT[ctx.roll(5) as usize];
            bark_at(ctx, beast, msg);
        }
    } else {
        let spawned = ctx.spawn_attacker_at(next_id, x, y, z);
        if let Some(cow) = spawned {
            if let Some(n) = ctx
                .world
                .objects
                .get_component_mut::<crate::model::npc::Npc>(&cow)
            {
                n.vars.insert("feeder".into(), ctx.player);
            }
            if mad_cow_reverts_to(next_id).is_some() {
                let feeder = ctx.player;
                ctx.world.scheduler.schedule(
                    ctx.world.tick + 100,
                    crate::scheduler::ScheduledTask::MadCowPolymorph {
                        cow_oid: cow,
                        feeder_oid: feeder,
                    },
                );
            }
        }
    }
}

/// A no-param npc-string bark from `npc_oid`.
fn bark(ctx: &mut QuestCtx, npc_oid: i32, npc_string_id: i32) {
    say_impl(ctx, npc_oid, npc_string_id, false);
}

/// A bark carrying the player's name as `$s1`.
fn bark_at(ctx: &mut QuestCtx, npc_oid: i32, npc_string_id: i32) {
    say_impl(ctx, npc_oid, npc_string_id, true);
}

fn say_impl(ctx: &mut QuestCtx, npc_oid: i32, npc_string_id: i32, with_name: bool) {
    let Some(npc_id) = npc_id_of(ctx.world, npc_oid) else {
        return;
    };
    let pkt = if with_name {
        let name = ctx
            .world
            .objects
            .get_component::<crate::model::Player>(&ctx.player)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        crate::network::server_packets::npc_say_param(npc_oid, npc_id, npc_string_id, &name)
    } else {
        crate::network::server_packets::npc_say(npc_oid, npc_id, npc_string_id)
    };
    broadcast_near(ctx, npc_oid, &pkt);
}

fn social(ctx: &mut QuestCtx, npc_oid: i32, action: i32) {
    let pkt = crate::network::server_packets::social_action(npc_oid, action);
    broadcast_near(ctx, npc_oid, &pkt);
}

fn broadcast_near(ctx: &mut QuestCtx, npc_oid: i32, pkt: &[u8]) {
    if let Some(region) = ctx
        .world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    {
        crate::game_loop::helpers::broadcast_near_region(ctx.world, region, pkt);
    }
}
