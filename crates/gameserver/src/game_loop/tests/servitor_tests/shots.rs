//! Soulshots and spiritshots: charging from the owner, spending on a hit or
//! cast, retiring an exhausted stack, and the owner's weapon bonus.

use super::*;

/// A servitor with no upkeep item is never charged.
#[test]
fn a_servitor_without_upkeep_is_never_charged() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    assert_eq!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .next_consume_tick,
        u64::MAX,
        "no upkeep clock at all"
    );
    world.tick += 100_000;
    handle_life_tick(&mut world, oid);
    assert_eq!(servitor_of(&world, OWNER), Some(oid));
}

/// A pet charges from its **owner's** Beast shots, spending the count its
/// level demands.
#[test]
fn a_pet_charges_shots_from_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 10);

    assert!(
        crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true),
        "charged"
    );
    assert_eq!(
        owner_shot_count(&world),
        8,
        "the level-1 row costs 2 shots per hit"
    );
    assert!(
        world
            .objects
            .get_component::<model::components::combat::ChargedShots>(&pet_oid)
            .unwrap()
            .soulshot
    );
}

/// The cost follows the pet's level, so a levelled pet is more expensive to
/// keep shotted — the mechanic, not an incidental detail.
#[test]
fn the_shot_cost_follows_the_pets_level() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 20);

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2
    );

    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);
    assert_eq!(
        owner_shot_count(&world),
        17,
        "level 2 costs 3 per hit, not 2"
    );
}

/// Already charged, no second charge — and no second cost.
#[test]
fn a_charged_pet_does_not_recharge() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 10);

    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);
    let after_first = owner_shot_count(&world);
    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);
    assert_eq!(
        owner_shot_count(&world),
        after_first,
        "no double spend while charged"
    );
}

/// Whether `item_id` is still armed for auto-use on the owner.
fn toggle_is_on(world: &World, item_id: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&OWNER)
        .is_some_and(|p| p.auto_shots.contains(&item_id))
}

/// The `(item_id, enable, shot_type)` of every `ExAutoSoulShot` in `packets`.
fn auto_shot_echoes(packets: &[Vec<u8>]) -> Vec<(i32, i32, i32)> {
    packets
        .iter()
        .filter(|p| {
            p.len() >= 15
                && p[0] == server_packets::opcodes::EX
                && i16::from_le_bytes(p[1..3].try_into().unwrap())
                    == server_packets::opcodes::EX_AUTO_SOUL_SHOT
        })
        .map(|p| {
            (
                i32::from_le_bytes(p[3..7].try_into().unwrap()),
                i32::from_le_bytes(p[7..11].try_into().unwrap()),
                i32::from_le_bytes(p[11..15].try_into().unwrap()),
            )
        })
        .collect()
}

/// **A stack too thin for one swing retires the toggle.** Java's Beast handler
/// fails `destroyItemWithoutTrace` and falls into `disableAutoShot`; without it
/// the shot bar stays lit over a pet that silently stopped using shots, and
/// every swing re-walks a list it can never satisfy.
#[test]
fn a_partial_soulshot_stack_retires_the_toggle() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 1); // level 1 costs 2
    drain(&mut rx);

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert_eq!(owner_shot_count(&world), 1, "the odd shot is not consumed");
    assert!(
        !toggle_is_on(&world, BEAST_SOULSHOT),
        "auto-use is switched off"
    );

    let packets = drain(&mut rx);
    assert_eq!(
        auto_shot_echoes(&packets),
        vec![(BEAST_SOULSHOT, 0, model::ShotType::Soulshots as i32)],
        "the client is told the toggle went dark"
    );
    assert!(
        has_sm(
            &packets,
            server_packets::sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_DEACTIVATED
        ),
        "and why"
    );
}

/// The stack running out entirely is the same retirement, and it happens once:
/// a second swing over an already-cleared list says nothing.
#[test]
fn an_exhausted_soulshot_stack_retires_the_toggle_once() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    // Toggled on, bag empty — the state a player lands in after the last swing.
    world
        .objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .auto_shots
        .push(BEAST_SOULSHOT);
    drain(&mut rx);

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert!(!toggle_is_on(&world, BEAST_SOULSHOT));
    assert_eq!(auto_shot_echoes(&drain(&mut rx)).len(), 1, "told once");

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert!(
        auto_shot_echoes(&drain(&mut rx)).is_empty(),
        "and not again on the next swing"
    );
}

/// Spending the charge is a one-shot: the second swing is unshotted.
#[test]
fn the_charge_is_spent_once() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 10);
    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);

    assert!(
        crate::game_loop::servitor::uncharge_soulshot(&mut world, pet_oid),
        "first swing is shotted"
    );
    assert!(
        !crate::game_loop::servitor::uncharge_soulshot(&mut world, pet_oid),
        "the second is not"
    );
}

/// A pet with no owner shots toggled on charges nothing — the auto-use switch
/// is what arms it.
#[test]
fn without_the_toggle_a_pet_charges_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    // Shots in the bag, but never toggled on.
    let World { data, objects, .. } = &mut world;
    objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_500_002, BEAST_SOULSHOT, 10);

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert_eq!(owner_shot_count(&world), 10, "untouched");
}

// ---------------------------------------------------------------------------
// SUMMON target type (slice 19)
// ---------------------------------------------------------------------------

const BEAST_SPIRITSHOT: i32 = 6646;

fn register_beast_spiritshot(world: &mut World) {
    let mut t = crate::data::item_data::template::ItemTemplate::default();
    t.item_id = BEAST_SPIRITSHOT;
    t.name = "Beast Spiritshot".into();
    t.is_stackable = true;
    t.handler = crate::data::item_data::kinds::ItemHandler::BeastSpiritShot;
    t.default_action = crate::data::item_data::kinds::ActionType::SummonSpiritshot;
    world.data.item_data.insert_for_test(t);
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_700_001, BEAST_SPIRITSHOT, 10);
    objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .auto_shots
        .push(BEAST_SPIRITSHOT);
}

fn owner_spiritshots(world: &World) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&OWNER)
        .map(|inv| inv.count_of(BEAST_SPIRITSHOT))
        .unwrap_or(0)
}

/// A summon charges its Beast Spiritshot from the owner, at the pet level's
/// `spiritshot_count`.
#[test]
fn a_pet_charges_spiritshots_from_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world);

    assert!(crate::game_loop::servitor::recharge_spiritshots(
        &mut world, pet_oid
    ));
    assert_eq!(owner_spiritshots(&world), 8, "level 1 costs 2 per cast");
    assert!(
        world
            .objects
            .get_component::<model::components::combat::ChargedShots>(&pet_oid)
            .unwrap()
            .spiritshot
    );
}

/// The charge is spent by the **cast**, not a swing — and it doubles the
/// summon's magic damage while it lasts.
#[test]
fn a_spiritshot_doubles_a_summons_magic_damage() {
    let damage_with = |charged: bool| {
        let (mut world, _db, _l) = servitor_world();
        let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
        let pet_oid = summoned_pet(&mut world);
        add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
        {
            let v = world.objects.get_component_mut::<Vitals>(&FOE).unwrap();
            v.max_hp = 100_000;
            v.cur_hp = 100_000.0;
        }
        let skill = Skill {
            self_continuous: false,
            id: 4079,
            level: 1,
            magic_type: 1,
            effects: vec![SkillEffect::MagicalAttack { power: 50.0 }],
            ..Default::default()
        };
        if charged {
            world.objects.add_components(
                &pet_oid,
                model::components::combat::ChargedShots {
                    soulshot: false,
                    spiritshot: true,
                },
            );
        }
        let before = world.objects.get_component::<Vitals>(&FOE).unwrap().cur_hp;
        effects::apply_skill_effects(&mut world, pet_oid, FOE, &skill);
        before - world.objects.get_component::<Vitals>(&FOE).unwrap().cur_hp
    };

    let plain = damage_with(false);
    let shotted = damage_with(true);
    assert!(plain > 0.0, "the summon's spell hit ({plain})");
    assert!(
        shotted > plain * 1.5,
        "a charged spiritshot roughly doubles it ({plain} → {shotted})"
    );
}

/// One cast, one shot: the charge does not carry to the next spell.
#[test]
fn a_summon_spiritshot_is_spent_by_one_cast() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world);
    crate::game_loop::servitor::recharge_spiritshots(&mut world, pet_oid);

    assert!(
        crate::game_loop::servitor::uncharge_spiritshot(&mut world, pet_oid),
        "spent by the first cast"
    );
    assert!(
        !crate::game_loop::servitor::uncharge_spiritshot(&mut world, pet_oid),
        "and not the second"
    );
}

/// A physical skill does not burn a magic shot.
#[test]
fn a_physical_skill_does_not_spend_a_spiritshot() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world);
    crate::game_loop::servitor::recharge_spiritshots(&mut world, pet_oid);
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);

    let physical = Skill {
        self_continuous: false,
        id: 4080,
        level: 1,
        magic_type: 0,
        effects: vec![SkillEffect::MagicalAttack { power: 10.0 }],
        ..Default::default()
    };
    effects::apply_skill_effects(&mut world, pet_oid, FOE, &physical);

    assert!(
        world
            .objects
            .get_component::<model::components::combat::ChargedShots>(&pet_oid)
            .unwrap()
            .spiritshot,
        "the magic shot is still charged"
    );
}

/// **The spiritshot path retires the toggle exactly like the soulshot one.**
/// It used to do neither prune, so a summoner who ran dry kept a lit Beast
/// Spiritshot toggle over a servitor that had quietly stopped using shots —
/// forever, since nothing else on the summon path ever cleared it.
#[test]
fn a_partial_spiritshot_stack_retires_the_toggle() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world); // arms the toggle and gives 10
    // Down to one, and a level-1 cast costs two.
    world
        .objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .remove_item(BEAST_SPIRITSHOT, 9);
    drain(&mut rx);

    assert!(!crate::game_loop::servitor::recharge_spiritshots(
        &mut world, pet_oid
    ));
    assert_eq!(owner_spiritshots(&world), 1, "the odd shot is not consumed");
    assert!(
        !toggle_is_on(&world, BEAST_SPIRITSHOT),
        "auto-use is switched off"
    );

    let packets = drain(&mut rx);
    assert_eq!(
        auto_shot_echoes(&packets),
        vec![(BEAST_SPIRITSHOT, 0, model::ShotType::Spiritshots as i32)],
        "echoed as a spiritshot toggle, not a soulshot one"
    );
    assert!(has_sm(
        &packets,
        server_packets::sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_DEACTIVATED
    ));
}

/// An empty bag retires the spiritshot toggle too — Java's
/// `Summon.rechargeShots` prunes any auto-shot entry whose item is gone.
#[test]
fn an_exhausted_spiritshot_stack_retires_the_toggle() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world);
    world
        .objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .remove_item(BEAST_SPIRITSHOT, 10);
    drain(&mut rx);

    assert!(!crate::game_loop::servitor::recharge_spiritshots(
        &mut world, pet_oid
    ));
    assert!(!toggle_is_on(&world, BEAST_SPIRITSHOT));
    assert_eq!(auto_shot_echoes(&drain(&mut rx)).len(), 1);
}

/// Charging one kind leaves the other kind's toggle alone: a swing that finds
/// no soulshots must not switch off a perfectly stocked spiritshot entry.
#[test]
fn retiring_one_shot_kind_leaves_the_other_armed() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world); // 10 spiritshots, armed
    // Soulshots armed but the bag is empty.
    world
        .objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .auto_shots
        .push(BEAST_SOULSHOT);
    drain(&mut rx);

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert!(
        !toggle_is_on(&world, BEAST_SOULSHOT),
        "the dry kind retires"
    );
    assert!(
        toggle_is_on(&world, BEAST_SPIRITSHOT),
        "the stocked kind stays armed"
    );
    assert_eq!(
        auto_shot_echoes(&drain(&mut rx)),
        vec![(BEAST_SOULSHOT, 0, model::ShotType::Soulshots as i32)],
        "only the dry kind is echoed off"
    );
}

// ---------------------------------------------------------------------------
// Community-board "Pet" buffer (applies a scheme to the summon)
// ---------------------------------------------------------------------------

/// `ShotsBonusFinalizer` resolves through **`getActingPlayer()`**, and
/// `Summon.getActingPlayer()` returns the *owner*:
///
/// ```java
/// final Player player = creature.getActingPlayer();
/// if (player != null) {
///     final Item weapon = player.getActiveWeaponInstance();
///     if ((weapon != null) && weapon.isEnchanted()) baseValue += (weapon.getEnchantLevel() * 0.3) / 100;
/// }
/// ```
///
/// So a servitor's soulshots ride its **master's** weapon enchant — the summon
/// has no weapon of its own, and Java re-reads the stat on every swing, so the
/// bonus follows the master swapping weapons with no recompute on the summon.
/// A plain monster's `getActingPlayer()` is null and stays at a flat 1.
#[test]
fn a_servitors_shot_bonus_comes_from_its_owners_weapon() {
    use crate::data::item_data::SLOT_R_HAND;
    use crate::data::item_data::kinds::ItemKind;
    use crate::data::item_data::template::ItemStats;
    use crate::game_loop::combat::shots_bonus_of;
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    const SWORD: i32 = 541;
    const SWORD_OID: i32 = 9411;

    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).expect("summoned");

    assert_eq!(
        shots_bonus_of(&world, servitor),
        1.0,
        "bare-handed master, flat 1"
    );

    world
        .data
        .item_data
        .insert_for_test(super::skill_shield_tests::gear(
            SWORD,
            ItemKind::Weapon,
            SLOT_R_HAND,
        ));
    world.data.item_data.set_item_stats_for_test(
        SWORD,
        ItemStats {
            bonuses: vec![(Stat::PhysicalAttack, 100.0)],
            ..Default::default()
        },
    );
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&OWNER).expect("inv");
        inv.add_item(&data.item_data, SWORD_OID, SWORD, 1);
        inv.equip_item(&data.item_data, SWORD_OID);
        inv.set_item_enchant(SWORD_OID, 10);
    }
    crate::game_loop::helpers::recalculate_player_stats(&mut world, OWNER);

    assert!(
        (shots_bonus_of(&world, OWNER) - 1.03).abs() < 1e-12,
        "the master's +10 weapon is worth 3 %"
    );
    assert!(
        (shots_bonus_of(&world, servitor) - 1.03).abs() < 1e-12,
        "…and the servitor reads the same number through its owner, got {}",
        shots_bonus_of(&world, servitor)
    );
}
