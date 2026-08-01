//! Skill conditions — G34 S1 (`skills::conditions`, PLAN_G34_SKILL_PARITY.md).
//!
//! These run against the **real dist skills**, not fixtures, because the point
//! of the slice is that the datapack's own `<conditions>` blocks are now
//! honoured. Before S1 every one of these casts succeeded.

use super::*;
use crate::game_loop::skills::cast::handle_request_magic_skill_use;
use crate::model::components::{SkillBook, Vitals};
use crate::model::inventory::Inventory;
use crate::network::server_packets::sm_ids;

const CID: u32 = 1;
const CASTER: i32 = 5901;

/// Long Sword (SWORD), Bone Dagger (DAGGER) — two weapon types the dist's
/// `EquipWeapon` conditions discriminate between.
const LONG_SWORD: i32 = 2;
const BONE_DAGGER: i32 = 11;

/// Sonic Focus: `EquipWeapon` DUAL/DUALBLUNT/SWORD/BLUNT + `OpEnergyMax`.
const SONIC_FOCUS: i32 = 8;
/// Revival: `RemainHpPer` `LESS 10` on the **caster**.
const REVIVAL: i32 = 181;

fn dist_world() -> (World, impl Sized, impl Sized, impl Sized) {
    let (mut world, a, b, c) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    (world, a, b, c)
}

fn teach(world: &mut World, skill_id: i32) {
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(skill_id, 1);
}

/// Put `item_id` in the right hand. The object id is derived from the item id
/// so two calls in one test don't collide in the container — swapping weapons
/// mid-test is the point of the `EquipWeapon` case.
fn arm(world: &mut World, item_id: i32) {
    let mut inv = world
        .objects
        .get_component::<Inventory>(&CASTER)
        .expect("inventory")
        .clone();
    let oid = inv.add_item(&world.data.item_data, 0x5900_0000 + item_id, item_id, 1);
    inv.equip_item(&world.data.item_data, oid);
    world.objects.add_components(&CASTER, inv);
}

fn cast(world: &mut World, skill_id: i32) {
    handle_request_magic_skill_use(world, CID, &magic_skill_use_body(skill_id, false));
    advance_world(world, 30);
}

/// `EquipWeapon` (88 learnable skills — the largest single condition on this
/// dist): Sonic Focus refuses bare-handed and with the wrong weapon class, and
/// lands with a sword. Java `EquipWeaponSkillCondition` tests the *equipped*
/// weapon's type mask, so a dagger is not "a weapon" for a sword skill.
#[test]
fn equip_weapon_refuses_the_wrong_weapon_and_bare_hands() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 0);
    teach(&mut world, SONIC_FOCUS);

    // Bare-handed: refused, and told why.
    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    let refused = drain(&mut rx);
    assert!(
        has_system_message(&refused, sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS),
        "bare-handed Sonic Focus is refused with the generic condition message"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        0,
        "…and builds no Force"
    );

    // A dagger is a weapon, but not one of DUAL/DUALBLUNT/SWORD/BLUNT.
    arm(&mut world, BONE_DAGGER);
    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        0,
        "a dagger does not satisfy a SWORD/BLUNT/DUAL condition"
    );

    // The listed type does.
    arm(&mut world, LONG_SWORD);
    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        1,
        "with a sword equipped the skill lands (level-1 cap is 1 charge)"
    );
}

/// `OpEnergyMax` is the *inverse* of `EnergySaved`: it refuses once the caster
/// is **at** the cap, and sends its own "force has reached maximum capacity"
/// ahead of the generic refusal — Java sends both.
#[test]
fn op_energy_max_refuses_at_the_cap_with_its_own_message() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 0);
    teach(&mut world, SONIC_FOCUS);
    arm(&mut world, LONG_SWORD);
    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .charges = 1; // the level-1 cap

    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    let refused = drain(&mut rx);
    assert!(
        has_system_message(&refused, sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY),
        "the condition's own message"
    );
    assert!(
        has_system_message(&refused, sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS),
        "…then the generic one — Java's `checkCondition` sends both"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        1,
        "no further Force gained"
    );
}

/// `RemainHpPer` with `percentType LESS` / `affectType CASTER`: Revival is the
/// emergency self-heal and Java refuses it above 10 % HP. The port used to cast
/// it at any HP, which is what made it an unconditional full heal.
#[test]
fn remain_hp_per_gates_revival_on_the_casters_own_hp() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 0);
    teach(&mut world, REVIVAL);
    let max_hp = pvit(&world, CASTER).max_hp as f64;

    // 20 % — above the threshold, so Java refuses and no healing happens.
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_hp = max_hp * 0.2;
    drain(&mut rx);
    cast(&mut world, REVIVAL);
    assert!(
        (pvit(&world, CASTER).cur_hp - max_hp * 0.2).abs() < 1.0,
        "above 10 % the cast is refused, HP unchanged: {}",
        pvit(&world, CASTER).cur_hp
    );

    // 5 % — inside the band, the heal lands.
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_hp = max_hp * 0.05;
    drain(&mut rx);
    cast(&mut world, REVIVAL);
    assert!(
        pvit(&world, CASTER).cur_hp > max_hp * 0.5,
        "at 5 % the emergency heal is allowed: {}",
        pvit(&world, CASTER).cur_hp
    );
}

/// The GM bypass: Java's `checkCondition` returns `true` outright for a
/// character that can override `PlayerCondOverride.SKILL_CONDITIONS`, so a GM
/// casts a sword skill bare-handed. Guards against the engine being wired in
/// ahead of that check.
#[test]
fn a_gm_skips_every_skill_condition() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 100);
    teach(&mut world, SONIC_FOCUS);

    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        1,
        "a GM builds Force with no weapon equipped"
    );
}

// `<passiveConditions>` has **no observable test on this dist**, and that is a
// finding rather than an omission.
//
// Only two learnable skills declare the block:
//
// * **Sword/Blunt Weapon Mastery (205)** — `EquipWeapon` SWORD/BLUNT. Its
//   `PAtk` effect *also* carries its own `<weaponType>SWORD,BLUNT`, which the
//   effect-level `weapon_condition` filter has honoured since G14. The skill
//   condition is therefore redundant here: disabling `passive_stat_gate`
//   entirely changes no stat on this datapack (verified by sabotage — the
//   dagger delta stayed 0 either way). An earlier version of this test claimed
//   the block was "the only thing tying the bonus to a sword"; that came from a
//   grep truncated before the `<weaponType>` at the end of the effect.
// * **Inner Rhythm (428)** — `TargetMyParty` in a passive block, which Java's
//   own handler answers `false` to (no target), disabling the passive outright.
//   Deliberately not reproduced; see `passive_stat_gate`'s note.
//
// So the gate is wired and correct, and inert on this content. Anything that
// makes it observable — a datapack effect losing its own `<weaponType>`, or a
// new passive condition kind — needs a test here at that point.
