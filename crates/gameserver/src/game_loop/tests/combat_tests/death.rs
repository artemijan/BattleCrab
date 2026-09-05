//! Dying: kill rewards and corpse decay, the player death penalty and
//! revive, monster respawn, and spoil-then-sweep.

use super::*;

/// The full melee kill: AttackRequest → Attack packet + combat stance, the
/// scheduled hit lands with `Formulas` damage, the monster dies (Die), the
/// killer gets XP/SP (level-up: SocialAction 2122 + SM 96), auto-loot adena
/// (SM 28 + InventoryUpdate; memory-first — the loot persists on the next
/// flush, not on pickup), and the corpse decays (DeleteObject) with no respawn
/// for a respawn-less spawn line.
#[test]
fn melee_kill_rewards_and_decay() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        // Level 5 exactly at its threshold +500 (table: L5 = 4000, L6 = 5000).
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.exp = 4500;
    }
    let npc_oid = NPC_OID + 7;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    // Swing rolls: hit (miss roll 0), no crit (99), random-damage delta 0
    // (roll(21) == 10 → ±0 on rndDam 10).
    world.force_rolls([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));

    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ATTACK),
        "Attack broadcast"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::AUTO_ATTACK_START),
        "combat stance"
    );

    // Expected damage: pAtk × rand(1.0) [+ position bonus] ×77 / pDef.
    // Attacker at (0,0), target heading 0 at (30,0) → attacker is BEHIND.
    let p_atk = pcs(&world, 3001).p_atk;
    let p_def = 40.0 * (5.0 + 89.0) / 100.0;
    let expected = formulas::physical::calc_auto_attack_damage(
        p_atk,
        1.0,
        model::movement::Position::Back,
        p_def,
        false,
        formulas::physical::CritDamage::default(),
        false,
        1.0,
        false,
        1.0,
        1.0,
        1.0,
    );
    assert!(
        expected > 100.0,
        "sanity: one swing must kill the 100 HP monster ({expected})"
    );

    // Hit lands at timeToHit = 1666 × 0.644 ≈ 1073 ms ⇒ 11 ticks. Queue the
    // drop rolls it will consume on death: level-gap pass (0), chance pass
    // (0 < 70%).
    world.force_rolls([0, 0]);
    advance_world(&mut world, 12);

    // Monster died: Die broadcast, rewards granted.
    assert!(nvit(&world, npc_oid).dead);
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| is_for(p, server_packets::opcodes::DIE, npc_oid)),
        "Die broadcast for the monster"
    );
    // XP: 2000 × share 1.0 × gap 1.0 (same level) → 4500 + 2000 = 6500 ⇒ level 6.
    let p = &world
        .objects
        .get_component::<Player>(&3001)
        .expect("player");
    assert_eq!(p.exp, 6500);
    assert_eq!(p.level, 6);
    let cp = pcp(&world, 3001);
    assert_eq!(cp.cur_cp, cp.max_cp as f64, "level-up refills CP");
    assert_eq!(p.sp, 100);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p)
                    == server_packets::sm_ids::YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4),
        "XP/SP system message"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION
                && i32::from_le_bytes(p[5..9].try_into().unwrap())
                    == server_packets::SOCIAL_ACTION_LEVEL_UP),
        "level-up flourish"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_LEVEL_HAS_INCREASED),
        "level-up message"
    );
    // Auto-loot: 5 adena in the inventory, SM 28, persisted via InsertItem.
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let adena = inv
        .items()
        .iter()
        .find(|i| i.item_id == 57)
        .expect("looted adena");
    assert_eq!(adena.count, 5);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOU_HAVE_OBTAINED_S1_ADENA),
        "obtained-adena message"
    );
    // Java `Player.addItem` → `PlayerInventory.addItem` → `sendInventoryUpdate`,
    // which never sends the `InventoryUpdate` alone: the status-bar adena
    // counter and weight bar ride along, so the bar tracks the loot live.
    assert!(
        packets.iter().any(|p| p[0] == 0x21),
        "auto-loot InventoryUpdate"
    );
    assert!(
        packets.iter().any(|p| is_ex(p, 0x13E)),
        "auto-loot refreshes the status-bar adena counter (ExAdenaInvenCount)"
    );
    assert!(
        packets.iter().any(|p| is_ex(p, 0x166)),
        "auto-loot refreshes the weight bar (ExUserInfoInvenWeight)"
    );
    // Memory-first: loot lands in the Inventory component (adena count asserted
    // above); it persists on the next flush, not on pickup.

    // The attack intent drops on the next combat tick (dead target).
    advance_world(&mut world, 1);
    assert!(!world.objects.has_component::<Intent>(&3001));

    // Decay after the 2 s corpse time: DeleteObject, corpse gone, no respawn
    // scheduled (respawn_secs == 0).
    advance_world(&mut world, 20);
    assert!(!world.objects.has_component::<model::npc::Npc>(&npc_oid));
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "corpse DeleteObject"
    );
    assert!(
        world.scheduler.is_empty(),
        "no respawn for a respawn-less spawn line"
    );
}

/// The dead mob stays *selected* for its whole corpse window (so future
/// sweep/loot logic can act on the selected corpse); the target is released
/// only when it decays. At decay, `TargetUnselected` goes to *every* player who
/// still had it selected — not just the killer — clearing each ground ring (our
/// client keeps a dead/deleted target locked without the packet). Each
/// server-side `TargetRef` is cleared too.
#[test]
fn decaying_mob_sends_target_unselected_to_all_holders() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // A second player nearby who also has the mob targeted but did not kill it.
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 20, 0);
    let npc_oid = NPC_OID + 11;
    add_test_npc(&mut world, npc_oid, 40001, "Monster", 5, 40, 0, 0);

    // Both players select the mob (each client now shows its target ring).
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    handle_action(&mut world, 2, &action_body(npc_oid, 0));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid)
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3002).unwrap().0,
        Some(npc_oid)
    );
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Player 1 lands the kill — the corpse stays selected (sweep window).
    npc::npc_do_die(&mut world, npc_oid, 3001);
    let got_unselect = |packets: &[Vec<u8>], player_oid: i32| {
        packets
            .iter()
            .any(|p| is_for(p, server_packets::opcodes::TARGET_UNSELECTED, player_oid))
    };
    assert!(
        !got_unselect(&drain(&mut a_rx), 3001),
        "no TargetUnselected at death"
    );
    assert!(
        !got_unselect(&drain(&mut b_rx), 3002),
        "no TargetUnselected at death"
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_oid),
        "corpse stays selected while it lasts (for sweep/loot)"
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3002).unwrap().0,
        Some(npc_oid)
    );

    // Corpse decays → both clients get their own TargetUnselected (payload
    // carries the *deselecting* player's id) and both server-side targets clear.
    npc::handle_npc_decay(&mut world, npc_oid);
    assert!(
        got_unselect(&drain(&mut a_rx), 3001),
        "killer's ring clears at decay"
    );
    assert!(
        got_unselect(&drain(&mut b_rx), 3002),
        "onlooker's ring clears at decay"
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        None
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3002).unwrap().0,
        None
    );
}

/// Death and the to-village loop: a killing blow sends `Die` with the
/// to-village flag and applies the XP penalty; `RequestRestartPoint` ports
/// the corpse to the map-region town respawn (`TeleportToLocation`), and
/// `Appearing` revives at the configured 65% HP (`Revive` broadcast).
#[test]
fn player_death_penalty_and_revive_to_village() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    // One town region covering the fight location, respawn at (1000, 1000).
    world.data.map_region =
        crate::data::MapRegionData::from_regions(vec![crate::data::map_region::MapRegion {
            name: "test_town".into(),
            loc_id: 0,
            bbs: 0,
            respawn_points: vec![(1000, 1000, 7)],
            tiles: vec![(20, 18)],
        }]);
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.exp = 4500; // level 5 (threshold 4000) + 500 into the level
        p.level = 5;
    }
    world
        .objects
        .get_component_mut::<Vitals>(&3001)
        .unwrap()
        .cur_hp = 1.0;
    world
        .objects
        .get_component_mut::<PlayerVitals>(&3001)
        .unwrap()
        .cur_cp = 0.0;
    let npc_oid = NPC_OID + 10;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    // Wake the monster by damage (as if the player had hit it).
    combat::npc_receive_damage(&mut world, npc_oid, 3001, 10.0, false);
    drain(&mut a_rx);

    // Its swing kills the 1-HP player: force a clean hit.
    world.force_rolls([0, 99, 10]);
    advance_world(&mut world, 30);

    let p = pvit(&world, 3001);
    assert!(p.dead);
    assert_eq!(p.cur_hp, 0.0);
    // Death penalty: 1% (empty table default) of the 1000-XP level = 10.
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .expect("player")
            .exp,
        4490
    );
    let packets = drain(&mut a_rx);
    let die = packets
        .iter()
        .find(|p| is_for(p, server_packets::opcodes::DIE, 3001))
        .expect("player Die packet");
    assert_eq!(
        i32::from_le_bytes(die[5..9].try_into().unwrap()),
        1,
        "to-village enabled"
    );
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::YOUR_XP_HAS_DECREASED_BY_S1),
        "XP-loss message"
    );

    // To village: teleport to the region respawn point.
    world.force_roll(0); // random respawn-point pick
    handle_request_restart_point(&mut world, 1, &{
        let mut w = PacketWriter::new();
        w.write_i32(0); // TO_VILLAGE
        w.into_bytes()
    });
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (1000, 1000, 12),
        "respawn point z lifted by 5 (teleToLocation)"
    );
    let p = &world
        .objects
        .get_component::<Player>(&3001)
        .expect("player");
    assert!(p.teleporting && p.pending_revive && pvit(&world, 3001).dead);
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION)
    );

    // Client finished loading: Appearing → revive at 65% HP.
    on_packet(&mut world, 1, vec![cp::opcodes::APPEARING]);
    let p = &world
        .objects
        .get_component::<Player>(&3001)
        .expect("player");
    assert!(!pvit(&world, 3001).dead && !p.teleporting && !p.pending_revive);
    let v = pvit(&world, 3001);
    assert_eq!(v.cur_hp, v.max_hp as f64 * 0.65);
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::REVIVE)
    );
}

/// The decay → respawn loop over a real spawn line: the corpse decays
/// (`DeleteObject`), `Spawn.decreaseCount` schedules the respawn, and the
/// respawned NPC (fresh object id) is announced with `NpcInfo`.
#[test]
fn dead_monster_decays_and_respawns() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.data.spawn_data = crate::data::SpawnData {
        spawns: vec![crate::data::spawn_data::SpawnTemplate {
            file: "test/combat.xml".to_string(),
            ai: None,
            parameters: Default::default(),
            name: None,
            territories: vec![],
            groups: vec![crate::data::spawn_data::SpawnGroup {
                name: None,
                spawn_by_default: true,
                territories: vec![],
                npcs: vec![crate::data::spawn_data::NpcSpawnDef {
                    npc_id: 40001,
                    count: 1,
                    loc: Some(crate::data::spawn_data::FixedLoc {
                        x: 30,
                        y: 0,
                        z: 0,
                        heading: 0,
                    }),
                    respawn_secs: 3,
                    respawn_random_secs: 0,
                    chase_range: 0,
                    db_save: false,
                }],
            }],
        }],
    };
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = game_loop::npc::spawn_one(&mut world, 0, 0, 0).expect("spawned");
    world
        .objects
        .get_component_mut::<TargetRef>(&3001)
        .unwrap()
        .0 = Some(npc_oid);
    drain(&mut a_rx);

    // Kill it outright (drop level-gap roll forced to fail: no loot noise).
    world.force_roll(999_999);
    combat::npc_receive_damage(&mut world, npc_oid, 3001, 1_000_000.0, false);
    assert!(nvit(&world, npc_oid).dead);

    // Decay at +2 s: corpse gone, DeleteObject seen, dangling target dropped,
    // respawn pending.
    advance_world(&mut world, 21);
    assert!(!world.objects.has_component::<model::npc::Npc>(&npc_oid));
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        None
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT)
    );

    // Respawn at +3 s more: a fresh NPC on the same spawn line, announced.
    advance_world(&mut world, 31);
    let mut respawned_ids: Vec<i32> = Vec::new();
    world.objects.for_each_mut::<&model::npc::Npc>(|n| {
        if n.npc_id == 40001 {
            respawned_ids.push(n.object_id);
        }
    });
    let respawned_oid = *respawned_ids.first().expect("respawned");
    assert_ne!(respawned_oid, npc_oid, "transient ids are not reused");
    let rpos = world
        .objects
        .get_component::<Position>(&respawned_oid)
        .unwrap();
    assert_eq!((rpos.x, rpos.y, rpos.z), (30, 0, 0));
    assert!(!nvit(&world, respawned_oid).dead);
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_INFO),
        "respawn announced with NpcInfo"
    );
}

/// Spoil → death → Sweeper end to end: casting Spoil marks the mob
/// (`spoiler_object_id`), killing it rolls the `<spoil>` list into the corpse's
/// sweep loot, and the Sweeper cast hands that loot to the caster and consumes
/// the body (`ConsumeBody`). Drives the effect handlers directly (the cast
/// pipeline's targeting gate is unit-tested separately in `resolve_cast_target`).
#[test]
fn spoil_death_and_sweep_hands_loot_then_consumes_corpse() {
    use crate::game_loop::npc;
    use model::skill::Skill;
    use model::skill::effects::SkillEffect;
    use model::skill::target::{AffectObject, AffectScope, TargetType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // A spoil-only monster next to the caster: no death drops, one guaranteed
    // spoil item (Charcoal 1871, chance 100%). Register the item + template.
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::template::ItemTemplate {
            trade_flags: Default::default(),
            pre_conditions: Vec::new(),
            is_oly_restricted: false,
            is_event_restricted: false,
            for_npc: false,
            time: -1,
            duration: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::kinds::ActionType::Other,
            item_id: 1871,
            name: "Charcoal".into(),
            kind: crate::data::item_data::kinds::ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable: true,
            is_infinite: false,
            type1: 4,
            type2: 5,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 0,
            handler: crate::data::item_data::kinds::ItemHandler::None,
            crystal_type: crate::data::item_data::kinds::CrystalType::None,
            crystal_count: 0,
            attack_radius: 40,
            attack_angle: 0,
            mp_consume: 0,
            reduced_mp_consume: 0,
            reduced_mp_consume_chance: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
            etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
        });
    let mut t = crate::data::npc_data::default_template(40777);
    t.type_name = "Monster".into();
    t.level = 5;
    t.base_hp_max = 100.0;
    t.base_mp_max = 30.0;
    t.corpse_time = Some(10);
    t.drop_list_spoil.push(crate::data::npc_data::DropHolder {
        item_id: 1871,
        min: 3,
        max: 3,
        chance: 100.0,
    });
    world.data.npc_data.insert_for_test(t);
    let npc_oid = NPC_OID + 77;
    add_test_npc(&mut world, npc_oid, 40777, "Monster", 5, 10, 0, 0);

    // A skill carrying just the Spoil effect (magic level 10 ⇒ near-certain
    // land on a level-5 mob), and the Sweeper skill (Sweeper then ConsumeBody).
    let make = |id: i32, target_type, magic_level, effects| Skill {
        self_continuous: false,
        without_action: false,
        trait_type: model::skill::traits::TraitType::None,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: String::new(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type,
        magic_type: 0,
        magic_level,
        effect_point: -1,
        cast_range: 400,
        effect_range: 400,
        hit_time: 0,
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 0,
        reuse_delay_group: -1,
        mp_consume: 0,
        mp_initial_consume: 0,
        hp_consume: 0,
        abnormal_time: 0,
        abnormal_level: 0,
        abnormal_type: "NONE".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects,
        ..Default::default()
    };
    let spoil = make(254, TargetType::EnemyOnly, 10, vec![SkillEffect::Spoil]);
    let sweeper = make(
        42,
        TargetType::NpcBody,
        0,
        vec![SkillEffect::Sweeper, SkillEffect::ConsumeBody],
    );

    // Cast Spoil → the mob is marked as spoiled by the caster.
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &spoil);
    assert_eq!(
        world
            .objects
            .get_component::<model::npc::Npc>(&npc_oid)
            .unwrap()
            .spoiler_object_id,
        3001,
        "Spoil set the spoiler to the caster"
    );

    // Kill it → the spoil list rolls into the corpse's sweep loot.
    npc::npc_do_die(&mut world, npc_oid, 3001);
    assert_eq!(
        world
            .objects
            .get_component::<model::npc::Npc>(&npc_oid)
            .unwrap()
            .sweep_items
            .as_deref(),
        Some([(1871, 3)].as_slice()),
        "death rolled the guaranteed spoil item into sweep loot"
    );

    // Sweep → loot lands in the caster's inventory and the corpse is consumed.
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &sweeper);
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(1871),
        3,
        "sweep loot handed to the sweeper"
    );
    assert!(
        !world.objects.has_component::<model::npc::Npc>(&npc_oid),
        "ConsumeBody decayed the corpse immediately"
    );
}

// --- G24 sweep: siege sides ------------------------------------------------
