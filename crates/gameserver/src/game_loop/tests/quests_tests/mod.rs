//! Quest and quest-adjacent tests, split by the dist's own quest-number
//! blocks; the helpers every block needs live here.

mod class_quests;
mod class_transfer;
mod collection;
mod engine;
mod fishing;
mod high_level;
mod hunting;
mod sagas;
mod shops;
mod story;
mod trials;
mod tutorial;

use super::*;
use crate::game_loop::character::inventory;
use crate::game_loop::commerce::shop;
use crate::game_loop::items::ground_items;
use crate::game_loop::{npc, quests};

/// Put `item_id` straight into the RHand paperdoll. Bypasses `equip_item`,
/// which would need full weapon templates for these quest items.
fn equip_weapon_row(world: &mut World, player: i32, item_id: i32) {
    let row = crate::db::ItemRow {
        object_id: 90000,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: model::inventory::PaperdollSlot::RHand as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    world
        .objects
        .add_components(&player, Inventory::from_rows(&[row]));
}

/// Object ids of every live NPC with `npc_id`.
fn npcs_of(world: &mut World, npc_id: i32) -> Vec<i32> {
    let mut out = Vec::new();
    world.objects.for_each_mut::<&model::npc::Npc>(|n| {
        if n.npc_id == npc_id {
            out.push(n.object_id);
        }
    });
    out
}

fn quest_memo(world: &World, player: i32, quest: &str) -> i32 {
    world
        .objects
        .get_component::<model::components::social::Quests>(&player)
        .and_then(|q| q.0.get(quest))
        .and_then(|qs| qs.vars.get("memoState"))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}

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
