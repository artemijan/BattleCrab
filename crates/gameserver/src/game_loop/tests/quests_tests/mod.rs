//! Quest and quest-adjacent tests, split by the dist's own quest-number
//! blocks; the helpers every block needs live here.

mod alliance_master;
mod class_quests;
mod collection;
mod engine;
mod fishing;
mod high_level;
mod hunting;
mod shops;
mod story;
mod trials;
mod tutorial;

use super::*;
use crate::game_loop::character::inventory;
use crate::game_loop::commerce::shop;
use crate::game_loop::items::ground_items;
use crate::game_loop::{npc, quests};

/// Force a quest's cond directly — used to jump into a mid-quest stage without
/// replaying the whole chain.
fn set_quest_cond(world: &mut World, player: i32, quest: &str, cond: i32) {
    if let Some(q) = world
        .objects
        .get_component_mut::<model::components::social::Quests>(&player)
        && let Some(qs) = q.0.get_mut(quest)
    {
        qs.vars.insert("cond".to_string(), cond.to_string());
    }
}

fn inject(world: &mut World, oid: i32, obj: i32, item: i32, count: i64) {
    let World { objects, data, .. } = world;
    objects
        .get_component_mut::<Inventory>(&oid)
        .unwrap()
        .add_item(&data.item_data, obj, item, count);
}
