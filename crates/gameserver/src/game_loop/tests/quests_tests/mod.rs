//! Quest and quest-adjacent tests, split by the dist's own quest-number
//! blocks; the helpers every block needs live here.

mod alliance_master;
mod class_quests;
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

fn inject(world: &mut World, oid: i32, obj: i32, item: i32, count: i64) {
    let World { objects, data, .. } = world;
    objects
        .get_component_mut::<Inventory>(&oid)
        .unwrap()
        .add_item(&data.item_data, obj, item, count);
}
