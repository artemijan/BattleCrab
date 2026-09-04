//! `NPC.ini`'s last unread keys (row 14).
//!
//! Thirteen keys were parsed and five of them wired to a consumer that already
//! existed. These tests hold the five to Java's behaviour, and the config test
//! at the bottom holds the parse itself to the shipped file — the eight inert
//! ones have no observable behaviour to pin at their shipped values (see the
//! field docs in [`crate::config::npc`]), so what matters for them is that the
//! numbers are read correctly.

use super::*;
use crate::config::npc::NpcConfig;
use crate::model::components::Vitals;
use crate::model::npc::Npc;

/// `DecayTaskManager.add`: `delay += SPOILED_CORPSE_EXTEND_TIME` when the
/// corpse is spoiled or seeded. A plain kill gets the template's `corpseTime`;
/// a spoiled one gets `corpseTime + 10 s`, so the sweeper has time to walk over.
#[test]
fn a_spoiled_corpse_outlives_a_plain_one_by_the_configured_extension() {
    let decay_tick = |spoiled: bool| {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut t = crate::data::npc_data::default_template(40901);
        t.type_name = "Monster".into();
        t.level = 5;
        t.base_hp_max = 100.0;
        t.corpse_time = Some(20);
        world.data.npc_data.insert_for_test(t);
        let npc_oid = NPC_OID + 91;
        add_test_npc(&mut world, npc_oid, 40901, "Monster", 5, 10, 0, 0);
        if spoiled {
            world
                .objects
                .get_component_mut::<Npc>(&npc_oid)
                .expect("npc")
                .spoiler_object_id = 3001;
        }
        crate::game_loop::npc::npc_do_die(&mut world, npc_oid, 0);
        world
            .objects
            .get_component::<Npc>(&npc_oid)
            .expect("corpse")
            .decay_at_tick
    };
    let plain = decay_tick(false);
    let spoiled = decay_tick(true);
    // 10 s of extension at 10 ticks per second.
    assert_eq!(
        spoiled - plain,
        NpcConfig::default().spoiled_corpse_extend_time as u64 * 10,
        "SpoiledCorpseExtendTime should extend the corpse by exactly its value"
    );
    assert_eq!(
        plain,
        20 * 10,
        "a plain corpse gets the template's corpseTime"
    );
}

/// A seeded corpse takes the same extension — Java's guard is
/// `isSpoiled() || isSeeded()`, not spoil alone.
#[test]
fn a_seeded_corpse_takes_the_same_extension_as_a_spoiled_one() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(40902);
    t.type_name = "Monster".into();
    t.level = 5;
    t.base_hp_max = 100.0;
    t.corpse_time = Some(20);
    world.data.npc_data.insert_for_test(t);
    let npc_oid = NPC_OID + 92;
    add_test_npc(&mut world, npc_oid, 40902, "Monster", 5, 10, 0, 0);
    world
        .objects
        .get_component_mut::<Npc>(&npc_oid)
        .expect("npc")
        .seeded = true;
    crate::game_loop::npc::npc_do_die(&mut world, npc_oid, 0);
    assert_eq!(
        world
            .objects
            .get_component::<Npc>(&npc_oid)
            .expect("corpse")
            .decay_at_tick,
        (20 + NpcConfig::default().spoiled_corpse_extend_time) as u64 * 10
    );
}

/// `ConditionPlayerCanSweep`'s third gate:
/// `!attackable.isOldCorpse(sweeper, CORPSE_CONSUME_SKILL_ALLOWED_TIME_BEFORE_DECAY)`.
/// A corpse with more than 2 s left sweeps; the same corpse with less than 2 s
/// left is refused with "the corpse is too old", *after* the spoiler-owner
/// check has already passed — so this is the gate under test and not one of
/// the two before it.
#[test]
fn a_corpse_about_to_decay_is_too_old_to_sweep() {
    use crate::model::skill::Skill;
    use crate::model::skill::effects::SkillEffect;
    use crate::model::skill::target::{AffectObject, AffectScope, TargetType};
    use crate::network::server_packets::sm_ids;

    // `None` = no decay scheduled at all, which Java answers with
    // `Long.MAX_VALUE` remaining.
    let attempt = |ticks_left: Option<u64>| {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = NPC_OID + 93;
        add_test_npc(&mut world, npc_oid, 40903, "Monster", 5, 10, 0, 0);
        {
            let npc = world
                .objects
                .get_component_mut::<Npc>(&npc_oid)
                .expect("npc");
            // Spoiled by the sweeper themself, so the owner gate passes.
            npc.spoiler_object_id = 3001;
            npc.decay_at_tick = ticks_left.map_or(0, |t| world.tick + t);
        }
        world
            .objects
            .get_component_mut::<Vitals>(&npc_oid)
            .expect("vitals")
            .dead = true;
        let sweeper = Skill {
            id: 42,
            level: 1,
            target_type: TargetType::NpcBody,
            affect_scope: AffectScope::Single,
            affect_object: AffectObject::All,
            effects: vec![SkillEffect::Sweeper],
            ..Default::default()
        };
        let caster = world
            .objects
            .get_component::<Player>(&3001)
            .expect("caster")
            .clone();
        let caster_pos = *world
            .objects
            .get_component::<Position>(&3001)
            .expect("caster pos");
        resolve_cast_target(
            &world,
            &caster,
            &caster_pos,
            Some(npc_oid),
            &sweeper,
            false,
            false,
        )
        .map(|_| ())
    };

    // 5 s left — comfortably above the 2 s floor.
    assert!(attempt(Some(50)).is_ok(), "a fresh corpse sweeps");
    // 1 s left — under the floor.
    assert_eq!(
        attempt(Some(10)),
        Err(sm_ids::THE_CORPSE_IS_TOO_OLD_THE_SKILL_CANNOT_BE_USED),
        "a corpse inside CorpseConsumeSkillAllowedTimeBeforeDecay is refused"
    );
    // No decay scheduled: `getRemainingTime` is `Long.MAX_VALUE`, so the gate
    // must let it through rather than reading the sentinel as "0 ms left".
    assert!(
        attempt(None).is_ok(),
        "a corpse with no decay scheduled is never too old"
    );
}

/// `MaximumSlotsForPet` is `Config.INVENTORY_MAXIMUM_PET`, the pet bag's slot
/// count — it used to be a `const` in `servitor::pet`. The bag must actually
/// stop taking *non-stackable* items at the configured number, and a stackable
/// the bag already holds must still go in past the limit (Java's
/// `Inventory.validateCapacity` counts slots, not items).
#[test]
fn the_pet_bag_stops_at_the_configured_slot_count() {
    use crate::game_loop::servitor::pet::pet_inventory_has_room;
    use crate::model::inventory::PetInventory;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let owner = 3001;
    let _rx = ingame_caster(&mut world, 1, owner, 0, 0);
    world
        .objects
        .add_components(&owner, (PetInventory(Default::default()),));

    // Three distinct non-stackable items, plus one stackable.
    for (i, id) in [9_101, 9_102, 9_103].into_iter().enumerate() {
        let mut item = crate::data::item_data::template::ItemTemplate::default();
        item.item_id = id;
        item.name = format!("Slot Filler {i}");
        world.data.item_data.insert_for_test(item);
    }
    let mut stack = crate::data::item_data::template::ItemTemplate::default();
    stack.item_id = 9_200;
    stack.name = "Stackable".into();
    stack.is_stackable = true;
    world.data.item_data.insert_for_test(stack);

    world.cfg.npc.inventory_maximum_pet = 3;
    {
        let World { data, objects, .. } = &mut world;
        let inv = objects
            .get_component_mut::<PetInventory>(&owner)
            .expect("pet bag");
        for (n, id) in [9_101, 9_102].into_iter().enumerate() {
            inv.0.add_item(&data.item_data, 8_000_000 + n as i32, id, 1);
        }
        inv.0.add_item(&data.item_data, 8_000_100, 9_200, 5);
    }
    // 3 slots used of 3 → full.
    assert!(
        !pet_inventory_has_room(&world, owner, 9_103),
        "a full bag refuses a new slot"
    );
    assert!(
        pet_inventory_has_room(&world, owner, 9_200),
        "…but a stackable already held needs no new slot"
    );

    // Raise the key and the same bag has room again — the limit is the config,
    // not a constant.
    world.cfg.npc.inventory_maximum_pet = 4;
    assert!(
        pet_inventory_has_room(&world, owner, 9_103),
        "MaximumSlotsForPet is what decides the bag size"
    );
}

/// `P|MAttackFinalizer` and `P|MDefenseFinalizer`'s `if (creature.isRaid())`
/// pass. At the dist's 100 (÷100 = ×1.0) it changes nothing, which is exactly
/// why it needs a test: the raw `100` must not reach the multiplier.
#[test]
fn raid_multipliers_scale_only_raids_and_are_neutral_at_the_shipped_value() {
    let (world, _db_rx, _link_rx) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(40904);
    t.type_name = "RaidBoss".into();
    t.level = 40;
    t.base_p_atk = 500.0;
    t.base_m_atk = 400.0;
    t.base_p_def = 300.0;
    t.base_m_def = 200.0;
    t.base_hp_max = 10_000.0;

    let finalize = |cfg: &crate::config::CombatConfig, is_raid: bool| {
        model::npc_stats::npc_finalized_stats(
            &world.data,
            &t,
            &Buffs::default(),
            model::npc_stats::NpcStatMods::of(cfg, false, is_raid),
        )
        .0
    };

    // Shipped values: a raid and a non-raid finalize identically.
    let shipped_raid = finalize(&world.cfg, true);
    let shipped_plain = finalize(&world.cfg, false);
    assert_eq!(
        shipped_raid.p_atk as i32, shipped_plain.p_atk as i32,
        "RaidPAttackMultiplier=100 must mean ×1.0, not ×100"
    );
    assert_eq!(shipped_raid.p_def as i32, shipped_plain.p_def as i32);

    // Turn the multipliers up and only the raid moves.
    let mut cfg = world.cfg.clone();
    cfg.npc.raid_p_atk_multiplier = 2.0;
    cfg.npc.raid_m_atk_multiplier = 3.0;
    cfg.npc.raid_p_def_multiplier = 4.0;
    cfg.npc.raid_m_def_multiplier = 5.0;
    let boosted_raid = finalize(&cfg, true);
    let boosted_plain = finalize(&cfg, false);
    assert_eq!(
        boosted_plain.p_atk as i32, shipped_plain.p_atk as i32,
        "a non-raid must be untouched by the raid multipliers"
    );
    let ratio = |a: f64, b: f64| (a / b).round() as i32;
    assert_eq!(ratio(boosted_raid.p_atk, shipped_raid.p_atk), 2, "p.atk ×2");
    assert_eq!(ratio(boosted_raid.m_atk, shipped_raid.m_atk), 3, "m.atk ×3");
    assert_eq!(ratio(boosted_raid.p_def, shipped_raid.p_def), 4, "p.def ×4");
    assert_eq!(ratio(boosted_raid.m_def, shipped_raid.m_def), 5, "m.def ×5");
}

/// `NpcStatMods::of` composes the two guards independently: a champion raid
/// takes both multipliers, matching Java's two separate `if`s in
/// `PAttackFinalizer`.
#[test]
fn champion_and_raid_multipliers_compose() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.cfg.champion.enable = true;
    world.cfg.champion.atk = 3.0;
    world.cfg.npc.raid_p_atk_multiplier = 5.0;
    let mods = model::npc_stats::NpcStatMods::of(&world.cfg, true, true);
    assert_eq!(mods.atk, 3.0);
    assert_eq!(mods.raid_p_atk, 5.0);
    // …and neither leaks into the other's case.
    let champ_only = model::npc_stats::NpcStatMods::of(&world.cfg, true, false);
    assert_eq!(champ_only.raid_p_atk, 1.0);
    let raid_only = model::npc_stats::NpcStatMods::of(&world.cfg, false, true);
    assert_eq!(raid_only.atk, 1.0);
}

/// The parse itself, against the shipped file. `NpcConfig::default()` is
/// Java's fallbacks rather than the dist's values (`AnnounceMammonSpawn`
/// already differs), so this pins the thirteen new keys by name instead of
/// comparing whole structs.
#[test]
fn the_thirteen_npc_keys_parse_to_the_shipped_values() {
    let c = NpcConfig::load_from(crate::data::DIST_GAME);
    assert_eq!(c.inventory_maximum_pet, 12, "MaximumSlotsForPet");
    assert_eq!(c.spoiled_corpse_extend_time, 10, "SpoiledCorpseExtendTime");
    assert_eq!(
        c.corpse_consume_skill_allowed_time_before_decay, 2000,
        "CorpseConsumeSkillAllowedTimeBeforeDecay"
    );
    // The four stat multipliers ship as `100` and Java divides by 100.
    assert_eq!(c.raid_p_atk_multiplier, 1.0, "RaidPAttackMultiplier");
    assert_eq!(c.raid_m_atk_multiplier, 1.0, "RaidMAttackMultiplier");
    assert_eq!(c.raid_p_def_multiplier, 1.0, "RaidPDefenceMultiplier");
    assert_eq!(c.raid_m_def_multiplier, 1.0, "RaidMDefenceMultiplier");
    assert_eq!(
        c.raid_min_respawn_multiplier, 1.0,
        "RaidMinRespawnMultiplier"
    );
    assert_eq!(
        c.raid_max_respawn_multiplier, 1.0,
        "RaidMaxRespawnMultiplier"
    );
    // The four inert ones, at the values that make them inert.
    assert!(c.alt_mob_agro_in_peace_zone, "AltMobAgroInPeaceZone");
    assert!(c.alt_attackable_npcs, "AltAttackableNpcs");
    assert!(
        !c.attackables_camp_player_corpses,
        "AttackablesCampPlayerCorpses"
    );
    assert!(!c.guard_attack_aggro_mob, "GuardAttackAggroMob");
}
