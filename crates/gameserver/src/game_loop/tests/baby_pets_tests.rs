//! `ai/areas/BeastFarm/BabyPets` and `ai/others/OlyBuffer/OlyBuffer` — the two
//! AI scripts row 13 of the measured-gaps audit found unported.
//!
//! Both were invisible from inside the suite: the baby pets summon and feed and
//! level correctly, they simply never healed anyone, and the Olympiad buffer
//! spawns in every arena instance and did nothing when talked to.

use super::*;

use crate::model::components::{PetOf, Vitals};
use crate::scripts::baby_pets;

const OWNER: i32 = 9901;
const CID: u32 = 1;
/// Baby Buffalo — one of the three `BABY_PETS`.
const BABY_BUFFALO: i32 = 12780;
const BUFFALO_COLLAR: i32 = 6648;

fn baby_pet_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data.skill_data = dist::skills_owned();
    world.id_pool = 0x5000_0000..0x5000_0200;
    let mut t = crate::data::npc_data::default_template(BABY_BUFFALO);
    t.type_name = "Pet".into();
    t.name = "Baby Buffalo".into();
    t.level = 55;
    t.base_hp_max = 800.0;
    t.base_mp_max = 800.0;
    world.data.npc_data.insert_for_test(t);
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: BABY_BUFFALO,
            item_id: BUFFALO_COLLAR,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(
                55,
                crate::data::pet_data::PetLevel {
                    max_meal: 300,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            skills: vec![
                crate::data::pet_data::PetSkillLearn {
                    skill_id: 4717,
                    skill_level: 0,
                    min_level: 1,
                },
                crate::data::pet_data::PetSkillLearn {
                    skill_id: 4718,
                    skill_level: 0,
                    min_level: 1,
                },
            ],
        });
    let rx = ingame_player(&mut world, CID, OWNER, 0, 0, 0);
    (world, rx)
}

/// Put a baby buffalo out beside its owner and return its object id.
fn summon_baby(world: &mut World) -> i32 {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_100_050, BUFFALO_COLLAR, 1);
    let collar = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.item_id == BUFFALO_COLLAR)
        .unwrap()
        .object_id;
    world
        .objects
        .get_component_mut::<crate::model::Player>(&OWNER)
        .unwrap()
        .pending_pet_collar = Some(collar);
    crate::game_loop::servitor::summon_pet(world, OWNER).expect("summoned")
}

/// The heal amount is `sqrt(2 · mAtk)` plus the effect's `power` — and every
/// `Heal` skill on this dist declares `<item>power</item>` with no value, which
/// Java parses into a *list* rather than a parameter, so `getDouble("power")`
/// is 0 there too. The caster's M.Atk is therefore the whole heal, and a
/// fixture pet with none heals nothing.
fn give_pet_magic_attack(world: &mut World, pet: i32) {
    if let Some(cs) = world
        .objects
        .get_component_mut::<crate::model::components::CombatStats>(&pet)
    {
        cs.m_atk = 200.0;
        cs.m_atk_spd = 333;
    }
}

fn set_owner_hp(world: &mut World, hp: f64) {
    let v = world.objects.get_component_mut::<Vitals>(&OWNER).unwrap();
    v.max_hp = 1000;
    v.cur_hp = hp;
    v.dead = false;
}

fn owner_hp(world: &World) -> f64 {
    world
        .objects
        .get_component::<Vitals>(&OWNER)
        .unwrap()
        .cur_hp
}

/// **The auto-heal.** A wounded owner gets a heal ordered at them without
/// asking. The assertion is on the *cast the script starts* — its skill, its
/// level and its target — rather than on the HP that lands, because the timer
/// reschedules and a later tick's random rolls would mask the gate under test.
#[test]
fn a_baby_pet_heals_its_wounded_owner() {
    let (mut world, _rx) = baby_pet_world();
    let pet = summon_baby(&mut world);
    give_pet_magic_attack(&mut world, pet);
    set_owner_hp(&mut world, 100.0); // 10 % — under both thresholds
    // Both rolls succeed (`<= 25` and `<= 75`).
    world.force_rolls([0, 0]);

    baby_pets::handle_heal_tick(&mut world, pet);

    let cast = world
        .objects
        .get_component::<crate::model::components::Casting>(&pet)
        .expect("the pet started a heal");
    // At 10 % both fire; the second overwrites the first, so the live cast is
    // Greater Heal Trick — the emergency one.
    assert_eq!(cast.0.skill_id, 4718, "Greater Heal Trick");
    assert_eq!(cast.0.target_object_id, OWNER, "aimed at the owner");
    // `getHealLv(55)` = 5.
    assert_eq!(cast.0.skill_level, 5, "scaled off the pet's level");
}

/// And the cast really does restore HP once it lands — the script's half is
/// ordering it, but a heal that heals nothing would be no fix at all.
#[test]
fn the_ordered_heal_restores_hp_when_it_lands() {
    let (mut world, _rx) = baby_pet_world();
    let pet = summon_baby(&mut world);
    give_pet_magic_attack(&mut world, pet);
    set_owner_hp(&mut world, 100.0);
    world.force_rolls([0, 0]);

    baby_pets::handle_heal_tick(&mut world, pet);
    advance_world(&mut world, 400);

    assert!(
        owner_hp(&world) > 100.0,
        "the owner was healed, got {}",
        owner_hp(&world)
    );
}

/// The two skills have different HP gates: Heal Trick tops up under 80 %,
/// Greater Heal Trick is held for under 15 %. At 50 % only the first can fire,
/// so failing *its* roll means no heal at all even though the second's roll
/// succeeds.
#[test]
fn the_greater_heal_is_held_back_for_an_emergency() {
    let (mut world, _rx) = baby_pet_world();
    let pet = summon_baby(&mut world);
    give_pet_magic_attack(&mut world, pet);
    set_owner_hp(&mut world, 500.0); // 50 %
    // Heal Trick's roll fails (> 25), Greater Heal Trick's succeeds (<= 75).
    world.force_rolls([90, 0]);

    baby_pets::handle_heal_tick(&mut world, pet);

    // Observed on the tick itself: the timer reschedules, so waiting for HP
    // would let a *later* tick's random rolls heal and hide the gate.
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::Casting>(&pet),
        "no cast started — the emergency heal is held back at half health"
    );
}

/// `!summon.isHungry()` — a starving pet does not heal.
#[test]
fn a_hungry_baby_pet_does_not_heal() {
    let (mut world, _rx) = baby_pet_world();
    let pet = summon_baby(&mut world);
    give_pet_magic_attack(&mut world, pet);
    set_owner_hp(&mut world, 100.0);
    world.objects.get_component_mut::<PetOf>(&pet).unwrap().fed = 0;
    world.force_rolls([0, 0]);

    baby_pets::handle_heal_tick(&mut world, pet);

    assert!(
        !world
            .objects
            .has_component::<crate::model::components::Casting>(&pet),
        "a starving pet starts no cast"
    );
}

/// `getHealLv` — the auto-scaling curve, clamped to the skills' twelve levels.
#[test]
fn the_heal_level_follows_the_pets_level() {
    // Below 70: lvl/10, floored at 1.
    assert_eq!(baby_pets::heal_level_for_test(5), 1);
    assert_eq!(baby_pets::heal_level_for_test(40), 4);
    // From 70: 7 + (lvl-70)/5.
    assert_eq!(baby_pets::heal_level_for_test(70), 7);
    assert_eq!(baby_pets::heal_level_for_test(80), 9);
    // And clamped at the skills' top level.
    assert_eq!(baby_pets::heal_level_for_test(99), 12);
}

/// The timer only arms for the three baby species — every other pet summons
/// without one.
#[test]
fn only_baby_pets_get_the_heal_timer() {
    assert!(baby_pets::is_baby_pet(12780), "Baby Buffalo");
    assert!(baby_pets::is_baby_pet(12781), "Baby Kookaburra");
    assert!(baby_pets::is_baby_pet(12782), "Baby Cougar");
    assert!(!baby_pets::is_baby_pet(12077), "a Wolf is not a baby pet");
}

// ---------------------------------------------------------------------------
// `ai/others/OlyBuffer` — the arena's buff vendor
// ---------------------------------------------------------------------------

const BUFFER_NPC: i32 = 36402;
const BUFFER_OID: i32 = 5401;

fn buffer_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data.skill_data = dist::skills_owned();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut t = crate::data::npc_data::default_template(BUFFER_NPC);
    t.type_name = "Npc".into();
    t.name = "Olympiad Buffer".into();
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, BUFFER_OID, BUFFER_NPC, "Npc", 70, 0, 0, 0);
    let rx = ingame_player(&mut world, CID, OWNER, 0, 0, 0);
    (world, rx)
}

fn take_buff(world: &mut World, index: usize) {
    handle_request_bypass_to_server(
        world,
        CID,
        &bypass_body(&format!(
            "npc_{BUFFER_OID}_Quest OlyBuffer giveBuff;{index}"
        )),
    );
}

fn has_buff(world: &World, skill_id: i32) -> bool {
    crate::game_loop::abnormal::has_buff(world, OWNER, skill_id)
}

/// **The buffer's buffs.** Two of these stand in every Olympiad arena and
/// talking to one did nothing at all.
#[test]
fn the_olympiad_buffer_grants_its_buffs() {
    let (mut world, _rx) = buffer_world();

    take_buff(&mut world, 0); // Haste
    assert!(has_buff(&world, 1086), "Haste landed");

    take_buff(&mut world, 3); // Might
    assert!(has_buff(&world, 1068), "and Might");
}

/// `npc.getScriptValue() < 5` — five per buffer, and the counter is the
/// **NPC's**, which is what makes the allowance per arena-entry.
#[test]
fn the_buffer_stops_after_five() {
    let (mut world, _rx) = buffer_world();

    for i in 0..5 {
        take_buff(&mut world, i);
    }
    let script_value = world
        .objects
        .get_component::<crate::model::npc::Npc>(&BUFFER_OID)
        .map(|n| n.script_value);
    assert_eq!(script_value, Some(5), "five taken");

    // A sixth is refused: Magic Barrier (index 5) never lands.
    take_buff(&mut world, 5);
    assert!(!has_buff(&world, 1036), "the sixth buff is refused");
}

/// A forged index grants nothing rather than panicking on the parse — Java
/// throws here and the exception escapes its handler.
#[test]
fn a_forged_buff_index_grants_nothing() {
    let (mut world, _rx) = buffer_world();

    take_buff(&mut world, 99);
    handle_request_bypass_to_server(
        &mut world,
        CID,
        &bypass_body(&format!("npc_{BUFFER_OID}_Quest OlyBuffer giveBuff;xyz")),
    );

    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&BUFFER_OID)
            .map(|n| n.script_value),
        Some(0),
        "nothing was counted"
    );
}
