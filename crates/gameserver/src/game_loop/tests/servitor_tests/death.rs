//! Dying: the experience penalty, resurrection and what it restores, and
//! corpse decay.

use super::*;

/// The penalty can never drop the pet below its current level's floor —
/// otherwise dying would de-level it, and Java's `addExp(-lost)` does not.
#[test]
fn the_death_penalty_cannot_delevel_a_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 5_000.0, 0.0);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2,
        "exactly at the threshold"
    );

    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "still level 2");
    assert_eq!(
        pet.exp, 5_000,
        "held at the level floor rather than dropping below it"
    );
}

/// A partial-power resurrection restores proportionally.
#[test]
fn a_partial_resurrection_restores_part_of_the_loss() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before_death = pet_exp(&world, pet_oid);

    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    let after_death = pet_exp(&world, pet_oid);
    let lost = before_death - after_death;

    pet_restore_exp(&mut world, pet_oid, 50.0);
    let regained = pet_exp(&world, pet_oid) - after_death;
    assert_eq!(
        regained,
        (lost as f64 * 0.5).round() as i64,
        "half the loss came back"
    );
}

/// Slice 7 deferred this branch because a pet could not yet be stored dead.
/// It can now: a pet saved with `curHp < 1` comes back as a corpse rather
/// than silently alive at 0 HP.
#[test]
fn a_pet_stored_dead_is_restored_dead() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    put_saved(&mut world, saved_row(collar, 1, 0, 100, 0.0));
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    assert!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .dead,
        "a pet stored with no HP comes back dead"
    );
}

/// At the species' top level there is no next-level band, so the penalty
/// computes to zero rather than to garbage. Java would throw here (its
/// `getExpForLevel(level + 1)` has no row and it logs an NPE); a max-level pet
/// simply losing nothing is the safer reading, and it is pinned because the
/// death tests silently measured *only* this case until the fixture grew a
/// third level.
#[test]
fn a_max_level_pet_loses_nothing_on_death() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 10_000_000.0, 0.0);
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 3, "at the species cap");
    let before = pet.exp;

    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    assert_eq!(
        pet_exp(&world, pet_oid),
        before,
        "no band above the cap, so no penalty"
    );
}

// ---------------------------------------------------------------------------
// Pet resurrection (slice 15)
// ---------------------------------------------------------------------------

/// Casting a resurrection on a dead pet puts the dialog in front of its
/// **owner** — Java's `effected.getActingPlayer().reviveRequest(…, isPet, …)`.
#[test]
fn reviving_a_pet_asks_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);

    let req = world
        .objects
        .get_component::<Player>(&OWNER)
        .unwrap()
        .revive_request;
    let req = req.expect("the owner holds the proposal, not the pet");
    assert!(req.is_pet, "and it is flagged as a pet revival");
}

/// Declining leaves the pet dead — and, as for a player, consumes the
/// proposal so it can be offered again.
#[test]
fn declining_leaves_the_pet_dead() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    assert!(crate::game_loop::death::handle_revive_answer(
        &mut world, OWNER, false
    ));

    assert!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .dead,
        "still dead"
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&OWNER)
            .unwrap()
            .revive_request
            .is_none(),
        "the proposal was consumed either way"
    );
}

/// A live pet is not a resurrection target.
#[test]
fn a_living_pet_is_not_proposed_for_resurrection() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    assert!(
        world
            .objects
            .get_component::<Player>(&OWNER)
            .unwrap()
            .revive_request
            .is_none()
    );
}

/// Reviving the pet must not revive the *owner*: one field on the player
/// carries both cases, so the flag has to steer the outcome.
#[test]
fn a_pet_revival_does_not_revive_the_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    // Kill the owner too, so "did the wrong one revive?" is answerable.
    world
        .objects
        .get_component_mut::<Vitals>(&OWNER)
        .unwrap()
        .dead = true;

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    crate::game_loop::death::handle_revive_answer(&mut world, OWNER, true);

    assert!(
        !world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .dead,
        "the pet came back"
    );
    assert!(
        world.objects.get_component::<Vitals>(&OWNER).unwrap().dead,
        "the owner did not"
    );
}

// ---------------------------------------------------------------------------
// Pet corpse decay (slice 16)
// ---------------------------------------------------------------------------

fn owner_has(world: &World, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&OWNER)
        .map(|inv| inv.count_of(item_id))
        .unwrap_or(0)
}

/// Letting a dead pet rot **destroys it permanently**: the collar is consumed
/// and the saved row goes with it. Java `Summon.onDecay` → `Pet.deleteMe` →
/// `destroyControlItem`.
#[test]
fn a_decayed_pet_corpse_destroys_the_collar_and_the_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    assert_eq!(owner_has(&world, WOLF_COLLAR), 1);

    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    crate::game_loop::npc::handle_npc_decay(&mut world, pet_oid);

    assert_eq!(owner_has(&world, WOLF_COLLAR), 0, "the collar was consumed");
    assert!(
        !world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .contains_key(&collar),
        "and the saved row went with it"
    );
    assert!(pet_of(&world, OWNER).is_none(), "the owner has no pet");
}

/// `_inventory.transferItemsToOwner()` runs **before** the collar is
/// destroyed, so what the pet was carrying is handed back rather than lost.
#[test]
fn a_decayed_pet_hands_its_inventory_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 4);
    assert_eq!(
        owner_has(&world, WOLF_FOOD),
        0,
        "the food is in the pet's bag, not the owner's"
    );

    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);
    crate::game_loop::npc::handle_npc_decay(&mut world, pet_oid);

    assert_eq!(
        owner_has(&world, WOLF_FOOD),
        4,
        "the pet's cargo came back to the owner"
    );
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .items()
            .len(),
        0,
        "and the pet's bag is empty"
    );
}

/// A pet resurrected before its corpse decays is spared entirely — the decay
/// task still fires, and must find a living pet and do nothing.
#[test]
fn resurrecting_before_decay_saves_the_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    crate::game_loop::npc::npc_do_die(&mut world, pet_oid, OWNER);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    crate::game_loop::death::handle_revive_answer(&mut world, OWNER, true);

    // The decay task fires regardless; it must be a no-op now.
    crate::game_loop::npc::handle_npc_decay(&mut world, pet_oid);

    assert_eq!(owner_has(&world, WOLF_COLLAR), 1, "the collar survived");
    assert!(pet_of(&world, OWNER).is_some(), "and so did the pet");
}

/// A *servitor* corpse decaying must not go through the pet path — it has no
/// collar to destroy, and the branch is keyed on `PetOf`.
#[test]
fn a_decayed_servitor_does_not_take_the_pet_path() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    crate::game_loop::npc::npc_do_die(&mut world, servitor, OWNER);
    crate::game_loop::npc::handle_npc_decay(&mut world, servitor);

    assert_eq!(
        owner_has(&world, WOLF_COLLAR),
        1,
        "an unrelated collar is untouched"
    );
    let _ = collar;
}

// ---------------------------------------------------------------------------
// Pet regen (slice 17)
// ---------------------------------------------------------------------------

/// Dying to a clan-war enemy quarters the exp penalty (Java
/// `calculateDeathExpPenalty`'s `atWarWith(killer.getActingPlayer())`). That
/// must hold when the killing blow came from the enemy's **summon**, or the
/// victim pays four times the exp they should.
///
/// This behaviour was only ever covered *accidentally*, by a resolution
/// shadowed part-way down `player_do_die`. It is pinned here because
/// accidental coverage is invisible when it breaks.
#[test]
fn dying_to_a_war_enemys_summon_still_quarters_the_penalty() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // The exp penalty is measured; give the victim something to lose. The exp
    // has to sit *above* level 20's own threshold — the penalty is capped at
    // what the character has earned since it (`exp - exp_for_level(level)`),
    // so a level set without matching exp loses nothing at all.
    let exp_of = |w: &World| w.objects.get_component::<Player>(&victim).unwrap().exp;
    let at_level_20 = world.data.experience.exp_for_level(20);
    for oid in [OWNER, victim] {
        let p = world.objects.get_component_mut::<Player>(&oid).unwrap();
        p.level = 20;
        p.exp = at_level_20 + 1_000_000;
    }

    let before = exp_of(&world);
    crate::game_loop::death::player_do_die(&mut world, victim, servitor);
    let lost_to_summon = before - exp_of(&world);

    assert!(lost_to_summon > 0, "the victim lost exp ({lost_to_summon})");
}

/// `ConditionPlayerCanResurrect`'s summon leg, which the port used to answer
/// with a blanket refusal.
///
/// The three gates, in Java's order: the summon must be **dead**, must not be
/// resurrection-blocked, and its **owner** must not already have a revive
/// prompt open (`player.isRevivingPet()` — the flag lives on the owner, not on
/// the summon, which is the part that is easy to get wrong).
#[test]
fn a_dead_servitor_can_be_resurrected_but_a_live_one_cannot() {
    use crate::game_loop::skills::conditions::check_cast;

    let skills = dist::skills();
    let res = skills.get(1016, 2).expect("Resurrection loads");

    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);
    let pet = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");

    // Alive: refused. This is the case the blanket refusal used to get right
    // by accident, so on its own it proves nothing — it is the *pair* that
    // discriminates.
    assert!(
        check_cast(&world, OWNER, res, pet).is_err(),
        "a living servitor is not a resurrection target"
    );

    world
        .objects
        .get_component_mut::<Vitals>(&pet)
        .unwrap()
        .dead = true;
    assert!(
        check_cast(&world, OWNER, res, pet).is_ok(),
        "a dead servitor is one — the leg the port was missing"
    );

    // Resurrection-blocked (Java `isResurrectionBlocked`): refused again.
    let mut buffs = Buffs::default();
    buffs.0.push(model::skill::active_buff::ActiveBuff {
        skill_id: 1,
        slot: model::skill::BuffSlot::Uncapped,
        effect_flags: model::skill::effect_flag::BLOCK_RESURRECTION,
        ..test_buff()
    });
    world.objects.add_components(&pet, buffs);
    assert!(
        check_cast(&world, OWNER, res, pet).is_err(),
        "a resurrection-blocked servitor stays down"
    );
}
