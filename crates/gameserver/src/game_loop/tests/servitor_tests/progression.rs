//! What a pet earns: the experience split with its owner, levelling and the
//! stats that come with it, and evolution.

use super::*;

/// `lifeTime <= 0` is Java's "no expiry" case (`Integer.MAX_VALUE`, commented
/// "Classic hack. Resummon upon entering game."), and a positive one is stored
/// as an absolute deadline.
#[test]
fn life_time_zero_means_no_expiry() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let forever = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    assert_eq!(
        world
            .objects
            .get_component::<ServitorOf>(&forever)
            .unwrap()
            .expires_at_tick,
        u64::MAX
    );

    let timed = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    let link = world.objects.get_component::<ServitorOf>(&timed).unwrap();
    assert_eq!(
        link.expires_at_tick,
        world.tick + 12_000,
        "1200 s at 10 ticks/s"
    );
    assert_eq!(link.life_time_secs, 1200);
}

/// The upkeep tick ends a servitor whose lifetime has run out.
#[test]
fn a_servitor_passes_away_when_its_lifetime_expires() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 60, 0, 0).unwrap();

    // Just before the deadline it survives.
    world.tick = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .unwrap()
        .expires_at_tick
        - 1;
    handle_life_tick(&mut world, oid);
    assert_eq!(
        servitor_of(&world, OWNER),
        Some(oid),
        "still here a tick early"
    );

    world.tick += 1;
    handle_life_tick(&mut world, oid);
    assert_eq!(
        servitor_of(&world, OWNER),
        None,
        "gone once the lifetime ran out"
    );
}

/// Java's "avoiding pet delevels due to exp per level values changed": a stored
/// exp below what the pet's level now costs is raised to that level's floor,
/// rather than the pet silently dropping a level when the curve is retuned.
#[test]
fn restored_exp_is_floored_at_the_level_cost() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    add_wolf_level_2(&mut world);
    // Level 2 now costs 5000 exp; this row predates that and holds only 100.
    put_saved(&mut world, saved_row(collar, 2, 100, 90, 42.0));
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "the pet keeps its level");
    assert_eq!(pet.exp, 5_000, "exp is raised to the level's floor instead");
}

/// The pet's cut comes **out of** the owner's award, not on top of it.
#[test]
fn a_nearby_pet_takes_its_cut_from_the_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summoned_pet(&mut world);

    let (owner_ratio, pet_exp, pet_sp) = split_exp_with_pet(&world, OWNER, 1000.0, 100.0);
    assert_eq!(owner_ratio, 0.73, "the owner keeps get_exp_type percent");
    assert!(
        (pet_exp - 270.0).abs() < 0.001,
        "the pet takes the remaining 27% ({pet_exp})"
    );
    assert!((pet_sp - 27.0).abs() < 0.001);
}

/// Out of range, the pet earns nothing and the owner keeps the lot.
#[test]
fn a_distant_pet_earns_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    world
        .objects
        .get_component_mut::<Position>(&pet_oid)
        .unwrap()
        .x += 10_000;

    let (owner_ratio, pet_exp, _) = split_exp_with_pet(&world, OWNER, 1000.0, 100.0);
    assert_eq!(owner_ratio, 1.0, "the owner keeps everything");
    assert_eq!(pet_exp, 0.0);
}

/// With no pet at all the owner's award is untouched — the guard that keeps
/// this change invisible to every player without one.
#[test]
fn no_pet_means_no_split() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let (owner_ratio, pet_exp, pet_sp) = split_exp_with_pet(&world, OWNER, 1000.0, 100.0);
    assert_eq!((owner_ratio, pet_exp, pet_sp), (1.0, 0.0, 0.0));
}

/// **A starving pet earns nothing** — Java's `isUncontrollable()` guard in
/// `PetStat.addExp`. This is the link between the feeding loop and
/// progression: let the food bar hit zero and the pet stops growing.
#[test]
fn a_starving_pet_earns_no_exp() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 0;

    add_pet_exp(&mut world, OWNER, 1000.0, 100.0);
    assert_eq!(
        world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp,
        0,
        "starving pets do not learn"
    );
}

/// Crossing the level threshold levels the pet, and the food capacity moves
/// with it.
#[test]
fn a_pet_levels_when_it_earns_enough() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        1
    );

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "crossed the 5000-exp threshold");
    assert_eq!(pet.max_fed, 300, "food capacity follows the level");
}

/// A pet cannot pass the top level its species table defines — every per-level
/// lookup would fall off the end.
#[test]
fn a_pet_stops_at_its_species_max_level() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    add_pet_exp(&mut world, OWNER, 10_000_000.0, 0.0);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        3,
        "capped at the highest level the table defines"
    );
}

/// Java `getControlItem().setEnchantLevel(getLevel())` — the collar's enchant
/// level *is* the pet's level, which is how a collar advertises its pet
/// without being summoned.
#[test]
fn levelling_stamps_the_pets_level_onto_its_collar() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let enchant = world
        .objects
        .get_component::<Inventory>(&OWNER)
        .unwrap()
        .by_object_id(collar)
        .unwrap()
        .enchant_level;
    assert_eq!(enchant, 2, "the collar reads +2 once the pet hits level 2");
}

/// End-to-end through the real reward path: the helper being right is not
/// enough if `add_exp_and_sp` never calls it. A pet out of range and a pet
/// beside its owner must produce *different* owner awards from the same kill.
#[test]
fn the_reward_path_actually_splits_with_the_pet() {
    let owner_exp_after = |pet_nearby: bool| {
        let (mut world, _db, _l) = servitor_world();
        let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
        let pet_oid = summoned_pet(&mut world);
        if !pet_nearby {
            world
                .objects
                .get_component_mut::<Position>(&pet_oid)
                .unwrap()
                .x += 10_000;
        }
        world
            .objects
            .get_component_mut::<Player>(&OWNER)
            .unwrap()
            .exp = 0;
        crate::game_loop::death::add_exp_and_sp(&mut world, OWNER, 1000.0, 100.0, false);
        (
            world.objects.get_component::<Player>(&OWNER).unwrap().exp,
            world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp,
        )
    };

    let (owner_alone, pet_idle) = owner_exp_after(false);
    let (owner_shared, pet_fed) = owner_exp_after(true);

    assert_eq!(pet_idle, 0, "a distant pet learns nothing");
    assert_eq!(pet_fed, 270, "a nearby pet takes 27% of the kill");
    assert_eq!(
        owner_alone, 1000,
        "without a pet in range the owner keeps it all"
    );
    assert_eq!(
        owner_shared, 730,
        "with a pet in range the owner keeps only 73%"
    );
}

// ---------------------------------------------------------------------------
// Pet stats (slice 13)
// ---------------------------------------------------------------------------

/// A pet's stats come from its **per-level pet row**, not its NPC template.
/// The Wolf's NPC fixture is level 1 with 300 HP; its pet row says 100.
#[test]
fn a_pets_stats_come_from_the_pet_table_not_the_npc_template() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    let max_hp = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
    let template_hp = world.data.npc_data.get(WOLF_NPC).unwrap().base_hp_max;
    assert_ne!(
        max_hp as f64, template_hp,
        "the NPC template's HP ({template_hp}) must not be what the pet uses"
    );
    assert!(max_hp > 0, "and the pet has real HP ({max_hp})");
}

/// The point of the whole slice: levelling has to make the pet *stronger*.
/// Before this, the level number moved and every combat stat stayed put.
#[test]
fn levelling_makes_the_pet_stronger() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    let before = combat(&world, pet_oid);
    let hp_before = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2,
        "it levelled"
    );

    let after = combat(&world, pet_oid);
    let hp_after = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
    assert!(
        after.p_atk > before.p_atk,
        "p.atk grew ({} → {})",
        before.p_atk,
        after.p_atk
    );
    assert!(
        after.m_atk > before.m_atk,
        "m.atk grew ({} → {})",
        before.m_atk,
        after.m_atk
    );
    assert!(
        after.p_def > before.p_def,
        "p.def grew ({} → {})",
        before.p_def,
        after.p_def
    );
    assert!(
        hp_after > hp_before,
        "max HP grew ({hp_before} → {hp_after})"
    );
}

/// Levelling must neither heal nor wound the pet — Java's stat recompute keeps
/// the bar where it was, and a level-up that silently full-heals would be a
/// free heal on demand.
#[test]
fn levelling_preserves_the_hp_fraction() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    {
        let v = world.objects.get_component_mut::<Vitals>(&pet_oid).unwrap();
        v.cur_hp = v.max_hp as f64 / 2.0;
    }

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    let frac = v.cur_hp / v.max_hp as f64;
    assert!(
        (frac - 0.5).abs() < 0.01,
        "still at half health after levelling ({frac})"
    );
}

/// A row missing a stat falls back to the NPC template rather than zeroing it.
/// Without this a single datapack gap gives the pet 0 max HP — which is how
/// this guard was found, when the shared fixture (no `org_hp`) produced a pet
/// that restored at 0 HP.
#[test]
fn a_missing_stat_row_falls_back_to_the_npc_template() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    // `give_collar`'s fixture carries no combat stats at all.
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();

    let max_hp = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
    assert!(
        max_hp > 0,
        "fell back to the template instead of zeroing the pet"
    );
}

// ---------------------------------------------------------------------------
// Pet death (slice 14)
// ---------------------------------------------------------------------------

/// Dying costs the pet experience — `percentLost = -0.07 × level + 6.5` of the
/// current level band.
#[test]
fn a_dying_pet_loses_experience() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before = pet_exp(&world, pet_oid);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2
    );

    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    let after = pet_exp(&world, pet_oid);
    assert!(after < before, "exp was lost on death ({before} → {after})");
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .exp_before_death,
        before,
        "the pre-death total is recorded for a later resurrection"
    );
}

/// A duel death costs nothing — Java skips the penalty entirely there.
#[test]
fn a_duel_death_costs_the_pet_no_experience() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before = pet_exp(&world, pet_oid);

    // `is_in_duel` is presence of `DuelRef`, so marking the owner is the whole
    // condition Java tests.
    world
        .objects
        .add_components(&OWNER, model::components::social::DuelRef(1));
    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);

    assert_eq!(
        pet_exp(&world, pet_oid),
        before,
        "no exp lost to a duel death"
    );
}

/// Resurrection hands back a share of what death took, and consumes the
/// record so a second revive restores nothing.
#[test]
fn resurrection_restores_a_share_of_the_lost_experience() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before_death = pet_exp(&world, pet_oid);

    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    let after_death = pet_exp(&world, pet_oid);
    let lost = before_death - after_death;
    assert!(lost > 0);

    pet_restore_exp(&mut world, pet_oid, 100.0);
    assert_eq!(
        pet_exp(&world, pet_oid),
        before_death,
        "a full-power revive restores all of it"
    );

    // The record is spent.
    pet_restore_exp(&mut world, pet_oid, 100.0);
    assert_eq!(
        pet_exp(&world, pet_oid),
        before_death,
        "a second revive restores nothing more"
    );
}

/// Accepting revives the pet and restores its lost experience.
#[test]
fn accepting_revives_the_pet_and_restores_its_exp() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before_death = world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp;
    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    let after_death = world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp;
    assert!(after_death < before_death, "the penalty applied");

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    assert!(crate::game_loop::death::handle_revive_answer(
        &mut world, OWNER, true
    ));

    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert!(!v.dead, "the pet is alive again");
    assert!(v.cur_hp > 0.0, "with HP on the bar");
    let exp_now = world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp;
    assert!(
        exp_now > after_death,
        "some of the lost exp came back ({after_death} → {exp_now})"
    );
}

/// An expired buff is not carried across — otherwise relogging would resurrect
/// buffs that had already run out.
#[test]
fn an_expired_servitor_buff_is_not_saved() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1111, 1200, 0, 0).unwrap();
    let buff = Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        abnormal_time: 10,
        effects: vec![SkillEffect::StatModifier(
            model::skill::effects::StatModifierEffect {
                stat: Stat::RunSpeed,
                mode: model::stats::StatModifierType::Diff,
                amount: 50.0,
                ..Default::default()
            },
        )],
        ..Default::default()
    };
    effects::apply_continuous_effects(&mut world, OWNER, servitor, &buff, None);

    world.tick += 20 * 10; // past its 10 s
    crate::game_loop::servitor::sync_summon_row(&mut world, OWNER);

    let saved = world
        .objects
        .get_component::<model::components::summons::PlayerSummons>(&OWNER)
        .unwrap()
        .0[0]
        .clone();
    assert!(
        saved.buffs.is_empty(),
        "an expired buff is not carried across a relog"
    );
}

// ---------------------------------------------------------------------------
// ServitorSkillUse (slice 29)
// ---------------------------------------------------------------------------

/// Wolf → Great Wolf is `evolve 1`, and it wants level 55.
const EVOLVE_MIN_LEVEL: i32 = 55;

/// Put a summoned wolf at `level`/`exp` so the evolve gates can be exercised.
fn set_pet_level(world: &mut World, pet: i32, level: i32, exp: i64) {
    if let Some(p) = world.objects.get_component_mut::<PetOf>(&pet) {
        p.level = level;
        p.exp = exp;
    }
}

/// **The evolve button works, and carries the pet across.** A qualifying wolf
/// becomes a Great Wolf: the old collar and its saved row are gone, the new
/// collar is in the inventory with the new pet's level stamped on it, and the
/// pet is out again carrying its experience.
#[test]
fn a_qualifying_pet_evolves_and_keeps_its_experience() {
    let (mut world, mut db_rx, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_test_npc(&mut world, NPC_OID + 30, 30827, "Lundy", 5, 60, 0, 0);
    add_great_wolf(&mut world);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL, 1_250_000);
    // A name to carry across.
    if let Some(pets) = world.objects.get_component_mut::<PlayerPets>(&OWNER) {
        pets.0.insert(
            collar,
            db::PetRow {
                collar_object_id: collar,
                name: "Rex".into(),
                level: EVOLVE_MIN_LEVEL,
                cur_hp: 1.0,
                cur_mp: 1.0,
                exp: 1_250_000,
                sp: 0,
                fed: 10,
                restore: true,
            },
        );
    }
    drain(&mut rx);
    drain_db(&mut db_rx);

    handle_request_bypass_to_server(
        &mut world,
        CID,
        &bypass_body(&format!("npc_{}_evolve 1", NPC_OID + 30)),
    );

    let inv = world.objects.get_component::<Inventory>(&OWNER).unwrap();
    assert_eq!(inv.count_of(WOLF_COLLAR), 0, "the old collar is destroyed");
    let new_collar = inv
        .items()
        .iter()
        .find(|i| i.item_id == GREAT_WOLF_COLLAR)
        .expect("the evolved collar");
    assert_eq!(
        new_collar.enchant_level, 56,
        "the collar records the pet's level — which is what a later restore reads"
    );
    let new_pet = pet_of(&world, OWNER).expect("the evolved pet is out");
    let link = world.objects.get_component::<PetOf>(&new_pet).unwrap();
    assert_eq!(link.exp, 1_250_000, "the experience came across");
    assert_eq!(
        link.level, 56,
        "…and the level is re-derived from it on the new curve"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::npc::Npc>(&new_pet)
            .unwrap()
            .npc_id,
        GREAT_WOLF_NPC
    );
    assert_eq!(
        world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .get(&new_collar.object_id)
            .map(|r| r.name.as_str()),
        Some("Rex"),
        "the pet keeps its name (the html promises it)"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::DeletePetRow { collar_object_id } if *collar_object_id == collar)),
        "the old collar's saved row is deleted, not left to haunt the new pet"
    );
}

/// **The exp floor is why an evolution doesn't demote the pet.** A wolf that
/// only just made level 55 carries less exp than the Great Wolf curve wants for
/// 55, and Java floors it — otherwise the reward for evolving would be a
/// level-1 pet.
#[test]
fn evolving_floors_the_experience_at_the_new_species_curve() {
    let (mut world, ..) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_test_npc(&mut world, NPC_OID + 30, 30827, "Lundy", 5, 60, 0, 0);
    add_great_wolf(&mut world);
    // A table that starts at level 10, *below* the button's min level of 55.
    // Without Java's explicit floor the carried 4,000 exp would derive level 10
    // and the summon path would happily floor at level 10's own exp — the pet
    // would survive the evolution 45 levels down.
    if let Some(t) = world.data.pet_data.by_item_id(GREAT_WOLF_COLLAR).cloned() {
        let mut t = t;
        t.levels.insert(
            10,
            crate::data::pet_data::PetLevel {
                max_meal: 300,
                exp: 1_000,
                ..Default::default()
            },
        );
        world.data.pet_data.insert_for_test(t);
    }
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();
    // Far below the Great Wolf's 1,000,000 for level 55, but above level 10's.
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL, 4_000);

    evolve::handle_evolve(&mut world, CID, OWNER, NPC_OID + 30, "evolve 1");

    let new_pet = pet_of(&world, OWNER).expect("evolved");
    let link = world.objects.get_component::<PetOf>(&new_pet).unwrap();
    assert_eq!(link.exp, 1_000_000, "floored at the new curve's level 55");
    assert_eq!(link.level, 55, "so it lands at 55, not at level 1");
}

/// The gates: too low a level, the wrong species, no pet out, and a dead pet
/// are each refused — with Java's `evolve_no.htm`, no system message.
#[test]
fn the_evolve_gates_refuse_and_change_nothing() {
    let (mut world, ..) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_great_wolf(&mut world);

    let held = |w: &World| {
        w.objects
            .get_component::<Inventory>(&OWNER)
            .unwrap()
            .count_of(GREAT_WOLF_COLLAR)
    };

    // No pet out at all.
    evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 1");
    assert_eq!(held(&world), 0, "nothing handed out");

    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();

    // Level 54 — one short.
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL - 1, 900_000);
    evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 1");
    assert_eq!(held(&world), 0, "one level short is still short");
    assert!(pet_of(&world, OWNER).is_some(), "and the pet is still out");

    // Right level, wrong button: `evolve 3` is the Baby Buffalo line. The
    // buffalo has to *exist* in pet data, or the refusal would come from the
    // missing lookup rather than from the species check.
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: 12780,
            item_id: 6648,
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
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: 16034,
            item_id: 10311,
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
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL, 1_150_000);
    evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 3");
    assert_eq!(held(&world), 0, "a wolf is not a buffalo");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&OWNER)
            .unwrap()
            .count_of(10311),
        0,
        "…and no improved buffalo collar either"
    );

    // Dead pet — Java calls this an exploit attempt.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&pet) {
        v.dead = true;
    }
    drain(&mut rx);
    evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 1");
    assert_eq!(held(&world), 0, "a dead pet cannot evolve");
    // The exploit attempt punishes: the immediate warning line (S1_TEXT) is
    // the only system message, and the kick lands 5 s later.
    assert_eq!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::S1_TEXT]
    );
    advance_ticks(&mut world, 51);
    assert!(!world.clients.contains_key(&CID), "kicked for the exploit");
}

/// **`ImmobilePetBuff`** (Servitor Empowerment 1299) roots the servitor for the
/// duration — the trade for whatever else the buff grants. The root is the
/// `IMMOBILIZED` flag, the same one `BlockMove` uses, and it has to come *back
/// off* when the buff ends or the servitor is stuck for good.
#[test]
fn servitor_empowerment_roots_the_servitor_until_it_expires() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    let immobile = |world: &World| {
        abnormal::flags_of(world, servitor) & model::skill::effect_flag::IMMOBILIZED != 0
    };
    assert!(!immobile(&world), "free to move before the buff");

    let empower = Skill {
        self_continuous: false,
        id: 9422,
        level: 1,
        target_type: TargetType::Summon,
        abnormal_time: 1200,
        abnormal_type: "EMPOWER".into(),
        effects: vec![SkillEffect::ImmobilePetBuff],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(empower.clone());
    effects::apply_skill_effects(&mut world, OWNER, servitor, &empower);
    assert!(immobile(&world), "the buff roots it");

    effects::handle_buff_expire(&mut world, servitor, 9422);
    assert!(
        !immobile(&world),
        "and expiry frees it — otherwise the servitor is stuck for good"
    );
}

// ---------------------------------------------------------------------------
// Resurrecting a servitor
// ---------------------------------------------------------------------------
