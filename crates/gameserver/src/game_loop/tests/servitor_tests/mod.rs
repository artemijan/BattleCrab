//! Servitor summoning — the first G29 slice.
//!
//! `Summon` is the single biggest unported effect on the whole ranking (24
//! learnable skills). This slice covers summoning, ownership, unsummon and the
//! owner's `PetInfo` view; follow/attack AI and the `SummonInfo` packet that
//! shows a servitor to *other* players are separate slices.

mod death;
mod equipment;
mod feeding;
mod persistence;
mod pet_summon;
mod progression;
mod pvp;
mod servitor;
mod shots;
mod skills;

use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::character::inventory;
use crate::game_loop::servitor::evolve;
use crate::game_loop::servitor::pet_restore_exp;
use crate::game_loop::servitor::{add_pet_exp, split_exp_with_pet};
use crate::game_loop::servitor::{
    handle_life_tick, on_owner_leave_world, pet_of, servitor_attack, servitor_follow_tick,
    servitor_of, servitor_stop, servitor_toggle_follow, summon_pet, summon_servitor,
    unsummon_servitor,
};
use crate::game_loop::servitor::{handle_pet_action, handle_servitor_action};
use crate::game_loop::skills::skill_by_id;
use crate::model::components::summons::ServitorOf;
use crate::model::components::summons::{PetOf, PlayerPets};
use crate::model::inventory::PetInventory;
use crate::model::skill::effects::SkillEffect;

const OWNER: i32 = 9901;

const CID: u32 = 1;

const PANTHER: i32 = 14799;

/// A distinct object id for the sparring dummy.
///
/// **Not `NPC_OID`.** A servitor is spawned through the runtime allocator,
/// which starts at `FIRST_NPC_OBJECT_ID` — the very id `NPC_OID` is — so a
/// fixture NPC placed there silently *replaces* the servitor. Three tests
/// failed on exactly that before this constant existed.
const FOE: i32 = NPC_OID + 10;

const DIST: &str = crate::data::DIST_GAME;

fn servitor_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for id in [PANTHER, PANTHER + 1] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Servitor".into();
        t.name = format!("Panther {id}");
        t.level = 20;
        t.base_hp_max = 400.0;
        t.base_mp_max = 200.0;
        t.collision_radius = 10.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db, l)
}

// ---------------------------------------------------------------------------
// Summon / unsummon
// ---------------------------------------------------------------------------

const WOLF_NPC: i32 = 12077;

const WOLF_COLLAR: i32 = 2375;

/// Register the Wolf's pet template + NPC template, and give the owner a
/// collar. Returns the collar's **object id**, which is the pet's identity.
fn give_collar(world: &mut World) -> i32 {
    let mut t = crate::data::npc_data::default_template(WOLF_NPC);
    t.type_name = "Pet".into();
    t.name = "Wolf".into();
    t.level = 1;
    t.base_hp_max = 300.0;
    t.base_mp_max = 100.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: WOLF_NPC,
            item_id: WOLF_COLLAR,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(
                1,
                crate::data::pet_data::PetLevel {
                    max_meal: 248,
                    consume_meal_in_normal: 10,
                    consume_meal_in_battle: 15,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            skills: Vec::new(),
        });
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_100_001, WOLF_COLLAR, 1);
    world
        .objects
        .get_component::<Inventory>(&OWNER)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.item_id == WOLF_COLLAR)
        .unwrap()
        .object_id
}

fn park_collar(world: &mut World, collar_oid: i32) {
    world
        .objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .pending_pet_collar = Some(collar_oid);
}

/// Give the Wolf template a level-2 row so level-dependent lookups have
/// somewhere to move to — with a single level every "restored at level N"
/// assertion would pass vacuously.
fn add_wolf_level_2(world: &mut World) {
    let mut t = world.data.pet_data.get(WOLF_NPC).unwrap().clone();
    t.levels.insert(
        2,
        crate::data::pet_data::PetLevel {
            max_meal: 300,
            exp: 5_000,
            ..Default::default()
        },
    );
    world.data.pet_data.insert_for_test(t);
}

fn saved_row(collar_oid: i32, level: i32, exp: i64, fed: i32, cur_hp: f64) -> db::PetRow {
    db::PetRow {
        collar_object_id: collar_oid,
        name: "Wolf".into(),
        level,
        cur_hp,
        cur_mp: 10.0,
        exp,
        sp: 7,
        fed,
        restore: false,
    }
}

fn put_saved(world: &mut World, row: db::PetRow) {
    world
        .objects
        .get_component_mut::<PlayerPets>(&OWNER)
        .unwrap()
        .0
        .insert(row.collar_object_id, row);
}

const WOLF_FOOD: i32 = 2515;

/// The Wolf Food skill (2048) — a single `Feed` effect restoring 100.
const WOLF_FOOD_SKILL: i32 = 2048;

/// Register the food item + its `Feed` skill so the eat path has something
/// real to run. Without the skill the item would be consumed for nothing,
/// which is exactly the bug the `Feed` parse arm fixes.
fn register_food(world: &mut World, restores: i32) {
    let mut item = crate::data::item_data::template::ItemTemplate::default();
    item.item_id = WOLF_FOOD;
    item.name = "Wolf Food".into();
    // 2515 ships `for_npc="true"`, which is Java's first gate on the pet
    // window — without it `RequestPetUseItem` refuses the item outright.
    item.for_npc = true;
    item.is_stackable = true;
    item.item_skills = vec![(WOLF_FOOD_SKILL, 1)];
    world.data.item_data.insert_for_test(item);

    let skill = Skill {
        self_continuous: false,
        id: WOLF_FOOD_SKILL,
        level: 1,
        effects: vec![SkillEffect::Feed {
            normal: restores,
            ride: 0,
            wyvern: 0,
        }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill);
}

fn put_food_in_pet(world: &mut World, count: i64) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .add_item(&data.item_data, 7_200_001, WOLF_FOOD, count);
}

fn fed(world: &World, pet_oid: i32) -> i32 {
    world.objects.get_component::<PetOf>(&pet_oid).unwrap().fed
}

/// Extend the Wolf with a level-2 row that costs 5000 exp and a real
/// `get_exp_type`, so the split and the level-up both have somewhere to go.
fn wolf_with_exp_curve(world: &mut World) {
    let mut t = world.data.pet_data.get(WOLF_NPC).unwrap().clone();
    // Three levels, not two: with only two, a level-2 pet sits at the species
    // cap and the death penalty's level *band* is empty — which made the first
    // draft of the death tests measure nothing.
    for (lvl, exp, meal) in [(1, 0i64, 248), (2, 5_000, 300), (3, 20_000, 340)] {
        t.levels.insert(
            lvl,
            crate::data::pet_data::PetLevel {
                max_meal: meal,
                consume_meal_in_normal: 10,
                consume_meal_in_battle: 15,
                exp,
                // The owner keeps 73%, so the pet takes 27% — the real value
                // on this species.
                owner_exp_taken: 73,
                // Level 2 is strictly stronger, so "did levelling do anything?"
                // is answerable rather than vacuous.
                p_atk: 10.0 * lvl as f64,
                m_atk: 8.0 * lvl as f64,
                p_def: 20.0 * lvl as f64,
                m_def: 15.0 * lvl as f64,
                max_hp: 100.0 * lvl as f64,
                max_mp: 50.0 * lvl as f64,
                regen_hp: 2.0,
                regen_mp: 0.9,
                // Cost rises with level, so "does the cost follow the level?"
                // is answerable rather than vacuous.
                soulshot_count: 1 + lvl,
                spiritshot_count: 1 + lvl,
                ..Default::default()
            },
        );
    }
    world.data.pet_data.insert_for_test(t);
}

fn summoned_pet(world: &mut World) -> i32 {
    let collar = give_collar(world);
    wolf_with_exp_curve(world);
    park_collar(world, collar);
    summon_pet(world, OWNER).unwrap()
}

fn combat(world: &World, oid: i32) -> CombatStats {
    *world.objects.get_component::<CombatStats>(&oid).unwrap()
}

fn pet_exp(world: &World, pet_oid: i32) -> i64 {
    world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp
}

const BEAST_SOULSHOT: i32 = 6645;

fn register_beast_soulshot(world: &mut World) {
    let mut t = crate::data::item_data::template::ItemTemplate::default();
    t.item_id = BEAST_SOULSHOT;
    t.name = "Beast Soulshot".into();
    t.is_stackable = true;
    t.handler = crate::data::item_data::kinds::ItemHandler::BeastSoulShot;
    t.default_action = crate::data::item_data::kinds::ActionType::SummonSoulshot;
    world.data.item_data.insert_for_test(t);
}

fn give_owner_shots(world: &mut World, count: i64) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_500_001, BEAST_SOULSHOT, count);
    objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .auto_shots
        .push(BEAST_SOULSHOT);
}

fn owner_shot_count(world: &World) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&OWNER)
        .map(|inv| inv.count_of(BEAST_SOULSHOT))
        .unwrap_or(0)
}

const GREAT_WOLF_NPC: i32 = 16025;

const GREAT_WOLF_COLLAR: i32 = 9882;

/// Register the Great Wolf so the evolution has somewhere to land, with a
/// two-entry level table (min level 55, then 56) so a level can be *read back*
/// from carried exp.
fn add_great_wolf(world: &mut World) {
    let mut t = crate::data::npc_data::default_template(GREAT_WOLF_NPC);
    t.type_name = "Pet".into();
    t.name = "Great Wolf".into();
    t.level = 55;
    t.base_hp_max = 900.0;
    t.base_mp_max = 300.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    let lvl = |exp: i64| crate::data::pet_data::PetLevel {
        max_meal: 300,
        consume_meal_in_normal: 10,
        consume_meal_in_battle: 15,
        exp,
        ..Default::default()
    };
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: GREAT_WOLF_NPC,
            item_id: GREAT_WOLF_COLLAR,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(55, lvl(1_000_000)), (56, lvl(1_200_000))]
                .into_iter()
                .collect(),
            skills: Vec::new(),
        });
}

/// Hit `summon_oid` for 50, as an auto-attack from `attacker`.
fn hit(world: &mut World, summon_oid: i32, attacker: i32) {
    combat::npc_receive_damage(world, summon_oid, attacker, 50.0, true);
}

fn hate_for(world: &World, summon_oid: i32, foe: i32) -> f64 {
    world
        .objects
        .get_component::<AggroList>(&summon_oid)
        .and_then(|a| a.0.get(&foe))
        .map(|i| i.hate)
        .unwrap_or(0.0)
}
