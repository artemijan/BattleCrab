//! Storing and restoring a summon: the saved row, syncing live state back,
//! unsummon round trips, coming back after a logout, and regeneration.

use super::*;

/// With no saved row the pet is brand new: template level, a full food bar and
/// full vitals — Java's two-arg `Pet` constructor.
#[test]
fn a_pet_with_no_saved_row_is_fresh() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    add_wolf_level_2(&mut world);
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 1, "fresh pet takes the template level");
    assert_eq!(pet.fed, pet.max_fed, "fresh pet starts fed");
    assert_eq!(pet.max_fed, 248, "max_meal for level 1");
    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "fresh pet spawns at full HP");
}

/// A saved row is what the pet comes back as — the whole point of the table.
#[test]
fn a_saved_pet_is_restored_from_its_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    add_wolf_level_2(&mut world);
    put_saved(&mut world, saved_row(collar, 2, 6_000, 90, 42.0));
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(
        pet.level, 2,
        "restored at the saved level, not the template's"
    );
    assert_eq!(pet.exp, 6_000);
    assert_eq!(pet.sp, 7);
    assert_eq!(
        pet.fed, 90,
        "the food bar carries over — it does not refill on summon"
    );
    assert_eq!(pet.max_fed, 300, "max_meal follows the restored level");
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        42.0,
        "wounded pet stays wounded"
    );
}

/// `sync_pet_row` is what makes any of this reach the DB: it folds the live
/// pet's state back into `PlayerPets`, which the character flush reads.
#[test]
fn syncing_writes_live_pet_state_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();

    // The pet takes a beating and burns some food.
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = 33.0;
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 12;

    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    let row = world
        .objects
        .get_component::<PlayerPets>(&OWNER)
        .unwrap()
        .0
        .get(&collar)
        .unwrap()
        .clone();
    assert_eq!(row.cur_hp, 33.0, "the wound is what gets saved");
    assert_eq!(row.fed, 12);
    assert_eq!(
        row.collar_object_id, collar,
        "keyed by the collar, as the table is"
    );
}

/// The round trip the gate actually asks for: summon, take damage, log out,
/// summon again — the pet comes back as it was left.
#[test]
fn a_pet_survives_an_unsummon_round_trip() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = 25.0;
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 60;

    // Owner logs out: state is captured, then the pet leaves the world.
    on_owner_leave_world(&mut world, OWNER);
    assert!(
        pet_of(&world, OWNER).is_none(),
        "the pet is gone with its owner"
    );

    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.fed, 60, "it comes back as hungry as it was left");
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        25.0,
        "and as wounded"
    );
}

/// Destroying the collar destroys the pet bound to it — Java unsummons it and
/// deletes the row. Object ids are recycled, so a surviving row would
/// eventually hand a stale pet to an unrelated item.
#[test]
fn destroying_the_collar_drops_the_saved_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let _ = summon_pet(&mut world, OWNER).unwrap();
    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    assert!(
        world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .contains_key(&collar)
    );

    let mut body = Vec::new();
    body.extend_from_slice(&collar.to_le_bytes());
    body.extend_from_slice(&1i64.to_le_bytes());
    items::handle_request_destroy_item(&mut world, CID, &body);

    assert!(
        pet_of(&world, OWNER).is_none(),
        "the summoned pet is unsummoned with its collar"
    );
    assert!(
        !world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .contains_key(&collar),
        "and its saved row goes with it"
    );
}

// ---------------------------------------------------------------------------
// Pet feeding (slice 8)
// ---------------------------------------------------------------------------

/// A pet regenerates from its **per-level pet row**, not the NPC template.
/// The fixture's pet row says 2.0 HP/tick; the Wolf NPC template says nothing,
/// so a template-driven pet would not heal at all.
#[test]
fn a_pet_regenerates_from_its_pet_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    {
        let v = world.objects.get_component_mut::<Vitals>(&pet_oid).unwrap();
        v.cur_hp = 10.0;
        v.cur_mp = 1.0;
    }

    crate::game_loop::stats::regen::run_npc_regen_tick(&mut world);

    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert_eq!(v.cur_hp, 12.0, "regen_hp 2.0 from the pet row");
    assert!(
        (v.cur_mp - 1.9).abs() < 1e-6,
        "regen_mp 0.9 from the pet row ({})",
        v.cur_mp
    );
}

/// Regen is capped at the maximum like any other — a nearly-full pet does not
/// overshoot.
#[test]
fn pet_regen_stops_at_full() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    let max_hp = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = max_hp as f64 - 0.5;

    crate::game_loop::stats::regen::run_npc_regen_tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        max_hp as f64,
        "clamped, not overshot"
    );
}

/// The pet multipliers are separate from the NPC ones — a server that retunes
/// monster regen must not accidentally retune pets, and vice versa.
#[test]
fn pet_regen_uses_the_pet_multiplier() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = 10.0;
    // Double pets, and set the *monster* multiplier to something absurd that
    // must not apply.
    world.cfg.npc.pet_hp_regen_multiplier = 2.0;
    world.cfg.npc.hp_regen_multiplier = 100.0;

    crate::game_loop::stats::regen::run_npc_regen_tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        14.0,
        "2.0 regen × the pet multiplier, untouched by the monster one"
    );
}

/// A dead pet does not regenerate back to life while its corpse waits to decay.
#[test]
fn a_dead_pet_does_not_regenerate() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);

    crate::game_loop::stats::regen::run_npc_regen_tick(&mut world);
    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert_eq!(v.cur_hp, 0.0, "a corpse stays a corpse");
    assert!(v.dead);
}

// ---------------------------------------------------------------------------
// Summon shots (slice 18)
// ---------------------------------------------------------------------------

/// A pet that was out at logout comes back on the next login —
/// `RestorePetOnReconnect` is True on this dist, so this is the normal path.
#[test]
fn a_pet_that_was_out_at_logout_comes_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 42;

    // Log out with the pet out: the sync marks the row restorable.
    on_owner_leave_world(&mut world, OWNER);
    assert!(
        pet_of(&world, OWNER).is_none(),
        "the pet left with its owner"
    );
    assert!(
        world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .get(&collar)
            .unwrap()
            .restore,
        "the row is marked as 'was out'"
    );

    // Log back in.
    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    let back = pet_of(&world, OWNER).expect("the pet came back");
    assert_eq!(
        world.objects.get_component::<PetOf>(&back).unwrap().fed,
        42,
        "and it came back in the state it left in"
    );
}

/// A pet deliberately put away before logging out stays in its collar — only
/// a pet that was *out* is restored.
#[test]
fn a_pet_put_away_before_logout_stays_away() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    // Put it away by hand first, *then* log out.
    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    unsummon_servitor(&mut world, OWNER);
    world
        .objects
        .get_component_mut::<PlayerPets>(&OWNER)
        .unwrap()
        .0
        .get_mut(&collar)
        .unwrap()
        .restore = false;
    on_owner_leave_world(&mut world, OWNER);

    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    assert!(pet_of(&world, OWNER).is_none(), "it stayed in its collar");
}

/// A collar traded away or destroyed between sessions leaves nothing to
/// restore — and must not leave a dangling holder behind.
#[test]
fn a_missing_collar_restores_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();
    on_owner_leave_world(&mut world, OWNER);

    // The collar is gone by the time they log back in.
    world
        .objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .remove_by_object_id(collar, 1);

    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    assert!(pet_of(&world, OWNER).is_none(), "nothing to restore");
    assert!(
        world
            .objects
            .get_component::<Player>(&OWNER)
            .unwrap()
            .pending_pet_collar
            .is_none(),
        "and no dangling collar holder was left set"
    );
}

/// With the config off, nothing is restored — the flag is honoured, not
/// assumed.
#[test]
fn the_reconnect_config_is_honoured() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();
    on_owner_leave_world(&mut world, OWNER);

    world.cfg.character.restore_pet_on_reconnect = false;
    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    assert!(pet_of(&world, OWNER).is_none(), "config off, no restore");
}

/// A servitor that was out at logout comes back — rebuilt by **re-casting its
/// summoning skill**, as Java does, with the saved vitals and remaining
/// lifetime stamped back on.
#[test]
fn a_servitor_that_was_out_at_logout_comes_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let summon_skill = 1111;
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id: summon_skill,
        level: 1,
        effects: vec![SkillEffect::Summon {
            npc_id: PANTHER,
            life_time: 1200,
            consume_item_id: 0,
            consume_item_count: 0,
        }],
        ..Default::default()
    });
    world
        .objects
        .get_component_mut::<SkillBook>(&OWNER)
        .unwrap()
        .0
        .insert(summon_skill, 1);

    let servitor = summon_servitor(&mut world, OWNER, PANTHER, summon_skill, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 77.0;
    world.tick += 200 * 10; // 200 s of its 1200 s spent

    on_owner_leave_world(&mut world, OWNER);
    assert!(
        servitor_of(&world, OWNER).is_none(),
        "it left with its owner"
    );

    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    let back = servitor_of(&world, OWNER).expect("the servitor came back");
    assert_eq!(
        world.objects.get_component::<Vitals>(&back).unwrap().cur_hp,
        77.0,
        "with the HP it had"
    );
    let remaining = (world
        .objects
        .get_component::<ServitorOf>(&back)
        .unwrap()
        .expires_at_tick
        - world.tick)
        / 10;
    assert!(
        (990..=1005).contains(&remaining),
        "and roughly its remaining lifetime, not a fresh 1200 s ({remaining})"
    );
}

/// A servitor dismissed before logout stays dismissed — the row is cleared
/// when nothing is out, or it would come back anyway.
#[test]
fn a_servitor_dismissed_before_logout_stays_away() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 1111, 1200, 0, 0).unwrap();
    crate::game_loop::servitor::sync_summon_row(&mut world, OWNER);
    unsummon_servitor(&mut world, OWNER);

    on_owner_leave_world(&mut world, OWNER);
    assert!(
        world
            .objects
            .get_component::<model::components::summons::PlayerSummons>(&OWNER)
            .unwrap()
            .0
            .is_empty(),
        "the stale row was cleared"
    );
    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    assert!(servitor_of(&world, OWNER).is_none());
}

/// Restore works off an **item**, not a live pet, and reads the pet's level out
/// of the collar's enchant — the one place it was recorded.
#[test]
fn restore_reads_the_level_off_the_collar_enchant() {
    let (mut world, ..) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_great_wolf(&mut world);
    // The Great Snow Wolf collar (10307) restores to the Great Wolf (9882).
    let mut t = crate::data::npc_data::default_template(GREAT_WOLF_NPC + 12);
    t.type_name = "Pet".into();
    t.name = "Great Snow Wolf".into();
    t.level = 55;
    t.base_hp_max = 900.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: GREAT_WOLF_NPC + 12,
            item_id: 10307,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(
                55,
                crate::data::pet_data::PetLevel {
                    max_meal: 300,
                    exp: 1_000_000,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            skills: Vec::new(),
        });
    let World { data, objects, .. } = &mut world;
    objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_100_055, 10307, 1);
    let snow = world
        .objects
        .get_component::<Inventory>(&OWNER)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.item_id == 10307)
        .unwrap()
        .object_id;
    // The collar remembers a level-56 pet.
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&OWNER) {
        inv.set_item_enchant(snow, 56);
    }

    evolve::handle_restore(&mut world, CID, OWNER, 0, "restore 1");

    let inv = world.objects.get_component::<Inventory>(&OWNER).unwrap();
    assert_eq!(inv.count_of(10307), 0, "the seasonal collar is consumed");
    assert_eq!(inv.count_of(GREAT_WOLF_COLLAR), 1, "the base one is given");
    let pet = pet_of(&world, OWNER).expect("and the pet is summoned");
    assert_eq!(
        world.objects.get_component::<PetOf>(&pet).unwrap().level,
        56,
        "at the level the collar's enchant recorded, not the minimum"
    );
}

// ---------------------------------------------------------------------------
// Buff sharing (`Skill.isSharedWithSummon`)
// ---------------------------------------------------------------------------
