//! Landing and expiring a buff: stacking by abnormal type, the slot cap,
//! dances and toggles, relog survival, and transformation.

use super::*;

/// G6 cast-pipeline gate: learn a class skill (SP spend + level gate),
/// cast it, watch the buff land (P.Def +8%) and the right packet sequence
/// go out, then fast-forward the scheduler past `abnormalTime` and watch
/// it expire and P.Def come back down. Runs entirely against a synthetic
/// `World` (no sockets) driven by manually advancing `world.tick` — real
/// time would mean actually waiting out the buff's 20 in-game seconds,
/// which a unit test shouldn't do (PLAN_GAME_SERVER.md §8.5: tick systems
/// are tested against synthetic `World` state, not real time).
#[test]
fn learn_and_cast_buff_skill_applies_and_expires() {
    use model::skill::Skill;
    use model::skill::effects::{SkillEffect, StatModifierEffect};
    use model::skill::target::{AffectObject, AffectScope};
    use model::stats::{Stat, StatModifierType};

    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();

    let mut hp_table = vec![0.0; 90];
    let mut mp_table = vec![0.0; 90];
    let mut cp_table = vec![0.0; 90];
    hp_table[5] = 100.0;
    mp_table[5] = 50.0;
    cp_table[5] = 20.0;
    let template = crate::data::player_template::PlayerTemplate {
        class_id: 0,
        base_str: 40,
        base_dex: 30,
        base_con: 43,
        base_int: 21,
        base_wit: 11,
        base_men: 25,
        hp_table,
        mp_table,
        cp_table,
        base_p_def: 80, // naked P.Def, matches the real HumanFighter.xml sum
        ..Default::default()
    };

    let mut data = GameData::for_test();
    data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![template]);
    data.skill_trees.insert_for_test(
        0,
        crate::data::skill_tree::SkillLearn {
            skill_id: 91,
            skill_level: 1,
            name: "Defense Aura".into(),
            get_level: 5,
            level_up_sp: 100,
            auto_get: false,
            required_items: Vec::new(),
        },
    );
    data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        basic_property: model::skill::BasicProperty::None,
        conditions: Vec::new(),
        target_conditions: Vec::new(),
        passive_conditions: Vec::new(),
        without_action: false,
        is_suicide_attack: false,
        icon: String::from("icon.skill0000"),
        trait_type: model::skill::traits::TraitType::None,
        static_reuse: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id: 91,
        level: 1,
        name: "Defense Aura".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Self_,
        magic_type: 1,
        magic_level: 0,
        effect_point: 0,
        cast_range: 0,
        effect_range: 0,
        hit_time: 400,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 2000,
        reuse_delay_group: -1,
        mp_consume: 4,
        mp_initial_consume: 1,
        hp_consume: 0,
        abnormal_time: 20,
        abnormal_level: 1,
        abnormal_type: "PD_UP".into(),
        activate_rate: -1,
        lvl_bonus_rate: 0,
        over_hit: false,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        fan_range: [0; 4],
        attribute_type: None,
        sub_level: 0,
        attribute_value: 0,
        end_effects: Vec::new(),
        channeling_effects: Vec::new(),
        mp_per_channeling: 0,
        channeling_skill_id: 0,
        channeling_tick_ms: 0,
        channeling_start_ms: 0,
        can_be_dispelled: true,
        is_debuff: false,
        excluded_from_check: false,
        shared_with_summon: true,
        stay_after_death: false,
        removed_on_damage: false,
        self_effects: Vec::new(),
        pve_effects: Vec::new(),
        pvp_effects: Vec::new(),
        effects: vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalDefence,
            mode: StatModifierType::Per,
            amount: 8.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        })],
    });

    let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

    // A level-5 character with 200 SP, walked straight to `InGame` (same
    // `Session` transition chain `handle_enter_world` uses in production).
    let mut chr = dummy_char(2001, "Def");
    chr.level = 5;
    chr.sp = 200;
    chr.cur_mp = 50.0;
    let bundle = Player::from_char(&world.data, &chr);
    // Naked P.Def = base(80) × levelMod((5+89)/100 = 0.94) = 75.2 (no gear,
    // so no slot subtraction); stored unrounded, the display truncates to 75.
    assert!(
        (bundle.combat.p_def - 75.2).abs() < 1e-9,
        "naked P.Def before any buff: {}",
        bundle.combat.p_def
    );

    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    let (session, bundle) = s.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));

    // --- Learn: RequestAcquireSkill(id=91, level=1, type=CLASS). ---
    let mut w = PacketWriter::new();
    w.write_i32(91);
    w.write_i32(1);
    w.write_i32(cp::combat::RequestAcquireSkill::CLASS);
    handle_request_acquire_skill(&mut world, 1, &w.into_bytes());

    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&2001)
            .unwrap()
            .0
            .get(&91),
        Some(&1)
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&2001)
            .expect("player")
            .sp,
        100,
        "200 SP - levelUpSp(100)"
    );
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACQUIRE_SKILL_DONE
    );
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x5F); // SkillList
    let _ = out_rx.try_recv().unwrap(); // AcquireSkillList
    let _ = out_rx.try_recv().unwrap(); // UserInfo

    // --- Cast: RequestMagicSkillUse(91). ---
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(91, false));

    assert!(world.objects.has_component::<Casting>(&2001));
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // initial MP consume
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_USE
    );
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::SYSTEM_MESSAGE
    ); // YOU_USE_S1
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::SETUP_GAUGE
    );
    assert_eq!(pvit(&world, 2001).cur_mp, 49.0, "50 - mpInitialConsume(1)");

    // --- Launch: hit = max(400/factor(1.0) − cancel(500), 0) = 0 ms, so
    // the launch task is already due; the finish follows 500 ms later.
    apply_due_tasks(&mut world);
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert!(
        world
            .objects
            .get_component::<Casting>(&2001)
            .is_some_and(|c| c.0.launched)
    );

    world.tick += 5;
    apply_due_tasks(&mut world);
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    ); // final MP consume
    assert_eq!(out_rx.try_recv().unwrap()[0], 0x85); // AbnormalStatusUpdate
    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff changed pDef → broadcastUserInfo)

    {
        assert!(
            !world.objects.has_component::<Casting>(&2001),
            "coolTime 0 frees the cast slot inline"
        );
        assert_eq!(pbuffs(&world, 2001), 1);
        assert!(
            (pcs(&world, 2001).p_def - 75.2 * 1.08).abs() < 1e-9,
            "75.2 × 1.08 (PhysicalDefence +8%): {}",
            pcs(&world, 2001).p_def
        );
    }
    assert_eq!(pvit(&world, 2001).cur_mp, 45.0, "49 - mpConsume(4)");

    // --- Advance past expiry (abnormalTime 20 s = 200 ticks) and drain again. ---
    world.tick += 200;
    apply_due_tasks(&mut world);

    let _ = out_rx.try_recv().unwrap(); // UserInfo (buff removal reverted pDef → broadcastUserInfo)
    let expired = out_rx.try_recv().unwrap();
    assert_eq!(expired[0], 0x85);
    assert_eq!(
        &expired[1..3],
        &[0, 0],
        "AbnormalStatusUpdate count = 0 once expired"
    );

    assert_eq!(pbuffs(&world, 2001), 0);
    assert!(
        (pcs(&world, 2001).p_def - 75.2).abs() < 1e-9,
        "P.Def restored after the buff expired: {}",
        pcs(&world, 2001).p_def
    );
}

/// A buff cast on another player lands on the *target*: their stats pump,
/// their client gets the AbnormalStatusUpdate, and the expiry restores.
#[test]
fn buff_on_other_player_lands_on_target() {
    let (mut world, ..) = cast_test_world();
    let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut b_rx);
    let base_p_atk = pcs(&world, 3002).p_atk;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, false));
    advance_ticks(&mut world, 10);

    {
        assert_eq!(pbuffs(&world, 3002), 1);
        assert!(
            pcs(&world, 3002).p_atk > base_p_atk,
            "P.Atk pumped by Might (+8%)"
        );
    }
    let b_packets = drain(&mut b_rx);
    assert!(
        b_packets.iter().any(|p| p[0] == 0x85),
        "target's client gets the AbnormalStatusUpdate"
    );
    assert_eq!(pbuffs(&world, 3001), 0, "nothing lands on the caster");

    advance_ticks(&mut world, 200);
    assert_eq!(pbuffs(&world, 3002), 0);
    assert_eq!(pcs(&world, 3002).p_atk, base_p_atk, "restored after expiry");
}

/// Bug fix: casting a beneficial (`Target`-type) skill on a monster requires
/// Ctrl (force). Without it the cast is refused (INVALID_TARGET); with it, it
/// proceeds.
#[test]
fn buff_on_monster_requires_ctrl() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 20;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

    // No Ctrl → refused, no cast.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, false));
    assert_eq!(
        sm_id(&a_rx.try_recv().unwrap()),
        server_packets::sm_ids::INVALID_TARGET
    );
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "no cast on a mob without force"
    );

    // Ctrl (force) → the cast starts.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "ctrl force-targets the mob"
    );
}

/// Bug fix: a buff cast on a monster modifies the mob's stats (like on a
/// character) and reverts on expiry.
#[test]
fn buff_on_monster_modifies_stats_and_reverts() {
    use model::components::skills::Buffs;
    use model::components::stats::CombatStats;
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 21;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);
    let base_p_atk = world
        .objects
        .get_component::<CombatStats>(&npc_oid)
        .unwrap()
        .p_atk;
    assert!(base_p_atk > 0.0, "sanity: the mob has a base pAtk");

    // Might (+8% pAtk), forced onto the mob; lands after hit_time (10 ticks).
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    advance_ticks(&mut world, 12);
    let buffed = world
        .objects
        .get_component::<CombatStats>(&npc_oid)
        .unwrap()
        .p_atk;
    assert!(
        (buffed - base_p_atk * 1.08).abs() < 1e-6,
        "Might raises the mob pAtk 8% ({base_p_atk} -> {buffed})"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .len(),
        1,
        "buff tracked on the mob"
    );

    // abnormal_time 20 s = 200 ticks → expiry reverts the stat.
    advance_ticks(&mut world, 205);
    let reverted = world
        .objects
        .get_component::<CombatStats>(&npc_oid)
        .unwrap()
        .p_atk;
    assert!(
        (reverted - base_p_atk).abs() < 1e-6,
        "expiry reverts the mob pAtk"
    );
    assert!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .is_empty(),
        "buff removed on expiry"
    );
}

/// Bug fix: a buff cast on a monster is shown in the target window of players
/// who have it selected (`ExAbnormalStatusUpdateFromTarget`, 0xFE:0xE6).
#[test]
fn buff_on_monster_shows_in_target_window() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 22;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50); // caster now targets the mob

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1068, true));
    advance_ticks(&mut world, 12);

    let pkt = drain(&mut a_rx)
        .into_iter()
        .find(|p| p.len() >= 13 && p[0] == 0xFE && p[1] == 0xE6 && p[2] == 0x00)
        .expect("ExAbnormalStatusUpdateFromTarget sent to the observer");
    assert_eq!(
        i32::from_le_bytes(pkt[3..7].try_into().unwrap()),
        npc_oid,
        "for the buffed mob"
    );
    assert_eq!(
        i16::from_le_bytes(pkt[7..9].try_into().unwrap()),
        1,
        "one buff shown"
    );
    assert_eq!(
        i32::from_le_bytes(pkt[9..13].try_into().unwrap()),
        1068,
        "Might listed in the target window"
    );
}

/// A live buff is captured into the save as its **remaining seconds** and comes
/// back on relog with that time intact (Java `storeEffect`/`restoreEffects`,
/// buff half). The countdown is frozen while offline: the restored buff gets
/// its full stored duration measured off the *new* tick, however long the
/// character was away.
#[test]
fn buff_survives_relog_without_offline_countdown() {
    use crate::game_loop::skills::effects::{apply_skill_effects, restore_persisted_buffs};

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // `synthetic_buff` has `abnormal_time` 100 s; burn 30 s of it.
    let buff = synthetic_buff(9500, 2, "RELOG", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    world.tick += 300;

    // The save captures the *remaining* 70 s, not the skill's full duration.
    let save = build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_buffs.len(), 1, "the live buff is captured");
    let row = save.skill_buffs[0];
    assert_eq!(
        (row.skill_id, row.skill_level, row.remaining_time_secs),
        (9500, 2, 70)
    );

    // Relog after a long "offline" gap — modelled by advancing the tick well past
    // where the buff would have expired had it kept counting down.
    world.tick += 100_000;
    let _rx2 = ingame_caster(&mut world, 2, 3002, 0, 0);
    restore_persisted_buffs(&mut world, 3002, &[row]);

    let restored = world
        .objects
        .get_component::<Buffs>(&3002)
        .and_then(|b| b.0.iter().find(|x| x.skill_id == 9500).cloned())
        .expect("buff restored");
    assert_eq!(
        restored.skill_level, 2,
        "the stored level came back, not the skill's level 1"
    );
    // 70 s off the *current* tick: the offline gap consumed none of the buff.
    assert_eq!(restored.expires_at_tick - world.tick, 700);

    // With the config off, buffs aren't persisted (and the DB rows get cleared).
    world.cfg.character.store_skill_cooltime = false;
    assert!(
        build_save_data(&world, 3001)
            .unwrap()
            .skill_buffs
            .is_empty()
    );
}

/// Java `storeEffect`'s skip list: a dance/song is dropped at logout unless
/// `AltStoreDances`, and a toggle (no expiry) is never stored at all.
#[test]
fn dances_and_toggles_are_not_stored_by_default() {
    use crate::game_loop::skills::effects::apply_skill_effects;

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // magic_type 3 = dance/song.
    let dance = synthetic_buff(9600, 1, "DANCE", 1, 3);
    // A toggle-ish buff: 0 `abnormal_time` → no expiry (the `u64::MAX` sentinel).
    let mut toggle = synthetic_buff(9601, 1, "TOGGLE", 1, 1);
    toggle.abnormal_time = 0;
    apply_skill_effects(&mut world, 3001, 3001, &dance);
    apply_skill_effects(&mut world, 3001, 3001, &toggle);
    assert_eq!(pbuffs(&world, 3001), 2, "both landed");

    world.cfg.character.alt_store_dances = false;
    let save = build_save_data(&world, 3001).expect("save data");
    assert!(
        save.skill_buffs.is_empty(),
        "dance dropped, toggle never stored"
    );

    // AltStoreDances=True (this dist) keeps the dance — but still not the toggle.
    world.cfg.character.alt_store_dances = true;
    let save = build_save_data(&world, 3001).expect("save data");
    assert_eq!(save.skill_buffs.len(), 1);
    assert_eq!(
        save.skill_buffs[0].skill_id, 9600,
        "only the dance came through"
    );
}

// --- Buff-slot stacking & count caps (Java `EffectList.addActive`) -----------

fn buff_skill_level(world: &World, oid: i32, skill_id: i32) -> i32 {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .and_then(|b| b.0.iter().find(|x| x.skill_id == skill_id))
        .map(|x| x.skill_level)
        .unwrap_or(0)
}

/// Same abnormal type: a lower-level cast is refused (no downgrade, no second
/// slot); an equal or higher level replaces in place.
#[test]
fn buff_same_abnormal_type_level_stacking() {
    use crate::game_loop::skills::effects::apply_skill_effects;
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let lvl1 = synthetic_buff(9001, 1, "MYBUFF", 1, 1);
    let lvl3 = synthetic_buff(9001, 3, "MYBUFF", 3, 1);

    // Land level 3.
    apply_skill_effects(&mut world, 3001, 3001, &lvl3);
    assert_eq!(pbuffs(&world, 3001), 1, "level 3 landed");
    assert_eq!(buff_skill_level(&world, 3001, 9001), 3);

    // A lower level is refused — no duplicate, still level 3.
    apply_skill_effects(&mut world, 3001, 3001, &lvl1);
    assert_eq!(
        pbuffs(&world, 3001),
        1,
        "lower level does not stack a second slot"
    );
    assert_eq!(
        buff_skill_level(&world, 3001, 9001),
        3,
        "lower level did not downgrade the buff"
    );

    // Re-casting the same level replaces in place (refresh) — still one slot.
    apply_skill_effects(&mut world, 3001, 3001, &lvl3);
    assert_eq!(pbuffs(&world, 3001), 1, "re-cast same level does not stack");
}

/// A different skill sharing the abnormal type also can't stack; the higher
/// level overrides the lower.
#[test]
fn buff_higher_level_overrides_same_type() {
    use crate::game_loop::skills::effects::apply_skill_effects;
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // Two *different* skills, same abnormal type.
    let weak = synthetic_buff(9101, 1, "SHARED", 1, 1);
    let strong = synthetic_buff(9102, 1, "SHARED", 4, 1);

    apply_skill_effects(&mut world, 3001, 3001, &weak);
    assert!(has_buff(&world, 3001, 9101), "weak buff landed");

    // Higher abnormal level overrides — the weak one is removed.
    apply_skill_effects(&mut world, 3001, 3001, &strong);
    assert_eq!(pbuffs(&world, 3001), 1, "same abnormal type never stacks");
    assert!(has_buff(&world, 3001, 9102), "strong buff present");
    assert!(!has_buff(&world, 3001, 9101), "weak buff overridden");
}

// --- RequestDispel (alt+click buff-cancel, ex 0xD0:0x0048) -------------------

/// The good-buff slot cap (`MaxBuffAmount`) drops the oldest buff to make room;
/// dances count against their own cap, not the buff cap.
#[test]
fn buff_slot_cap_drops_oldest() {
    use crate::game_loop::skills::effects::apply_skill_effects;
    let (mut world, ..) = cast_test_world();
    world.data.combat_caps.max_buff_count = 2; // shrink the cap for the test
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // Three distinct-type buffs; the cap is 2.
    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9201, 1, "A", 1, 1));
    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9202, 1, "B", 1, 1));
    assert_eq!(pbuffs(&world, 3001), 2, "at the buff cap");

    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9203, 1, "C", 1, 1));
    assert_eq!(pbuffs(&world, 3001), 2, "cap holds after a third");
    assert!(!has_buff(&world, 3001, 9201), "oldest buff (A) dropped");
    assert!(has_buff(&world, 3001, 9203), "newest buff (C) present");

    // A dance uses its own pool, so it lands on top of the 2 buffs.
    apply_skill_effects(&mut world, 3001, 3001, &synthetic_buff(9301, 1, "D", 1, 3));
    assert_eq!(
        pbuffs(&world, 3001),
        3,
        "the dance is counted separately, not against the buff cap"
    );
    assert!(has_buff(&world, 3001, 9301), "dance landed");
}

/// G19 `Transformation` effect: casting "Transform Doom Wraith" (618, real
/// dist data → `transformationId` 2) polymorphs the caster — durable
/// transform id + display id, the transform template's granted skill (586,
/// Rolling Attack) in the `SkillBook` — exactly like `//transform` (the admin
/// runtime `admin_ride_bike_transforms_and_reverts` covers), but reached
/// through the ordinary cast pipeline instead of the GM command. Re-casting
/// while transformed is refused (`ConditionPlayerCanTransform`'s
/// already-polymorphed leg); `handle_buff_expire` on the `TRANSFORM` buff
/// reverts everything, matching the buff's `BuffExpire`/dispel/death path.
#[test]
fn transformation_skill_polymorphs_and_reverts_on_expiry() {
    let (mut world, ..) = test_world();
    // The full real datapack, not just `transforms`/`skill_data`: `checkUseConditions`'
    // MP/HP prechecks need a real class template's hp/mp tables (`for_test`'s
    // `player_templates` is empty, so a level-1 dummy char would compute 0 max HP).
    world.data = dist::game_data_owned();

    let mut rx = ingame_player_access(&mut world, 1, 5001, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5001)
        .unwrap()
        .0
        .insert(618, 1);
    // A second, independent transform skill: Doom Wraith's own 4 h reuse means
    // re-clicking *it* is refused by the reuse gate long before any condition
    // runs (Java checks `isSkillDisabled` at `useMagic`'s top, well ahead of
    // `checkCondition`), so the already-polymorphed refusal below has to come
    // from a skill that is not on cooldown.
    world
        .objects
        .get_component_mut::<SkillBook>(&5001)
        .unwrap()
        .0
        .insert(617, 1);
    let base_run = world
        .objects
        .get_component::<Speeds>(&5001)
        .unwrap()
        .run_spd;

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    advance_world(&mut world, 40); // hitTime 2500 ms + finish, well inside 40 × 100 ms ticks

    {
        let p = world.objects.get_component::<Player>(&5001).unwrap();
        assert_eq!(
            p.transform_id, 2,
            "transformed into Doom Wraith (transformationId 2)"
        );
        assert_eq!(p.transform_display_id, 2, "display id == id on this dist");
    }
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&5001)
            .unwrap()
            .0
            .contains_key(&586),
        "transform's granted skill (Rolling Attack) present"
    );
    assert_ne!(
        world
            .objects
            .get_component::<Speeds>(&5001)
            .unwrap()
            .run_spd,
        base_run,
        "run speed overridden by the transform template"
    );
    assert_eq!(
        pbuffs(&world, 5001),
        1,
        "lands as one TRANSFORM buff (drives the expiry-based revert)"
    );

    // Transforming *again* while transformed is refused by the `CanTransform`
    // skill condition (G34 S1 — it used to be an inline block in `cast.rs`).
    // Java's `Skill.checkCondition` sends the handler's own message **and** the
    // generic "cannot be used due to unsuitable terms"; the inline version sent
    // only the first, which is the behaviour change this assertion pins.
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(617, false));
    let refused = drain(&mut rx);
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::YOU_ALREADY_POLYMORPHED_AND_CANNOT_POLYMORPH_AGAIN
        ),
        "already-polymorphed refusal sent"
    );
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS
        ),
        "…followed by the generic condition refusal, as Java sends both"
    );
    assert!(
        !world.objects.has_component::<Casting>(&5001),
        "the refused click never starts a cast"
    );

    // Expiry (natural `BuffExpire`, dispel, or death all route through this).
    effects::handle_buff_expire(&mut world, 5001, 618);
    // A TvT entrant cannot transform at all — Java's `isRegisteredOnEvent()`
    // leg, which sends a plain text line rather than a SystemMessage.
    {
        let p = world.objects.get_component::<Player>(&5001).unwrap();
        assert_eq!(p.transform_id, 0, "reverted before the event check");
    }
    // Clear the reuse the first cast left, or the refusal below would be the
    // cooldown talking rather than the event gate (it was, on the first
    // attempt at this test — the sabotage caught it).
    if let Some(r) = world.objects.get_component_mut::<Reuses>(&5001) {
        r.0.clear();
    }
    world.events.tvt.player_list.push(5001);
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    assert!(
        !world.objects.has_component::<Casting>(&5001),
        "an event entrant's transform click never starts a cast"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .transform_id,
        0,
        "…and they stay themselves"
    );
    world.events.tvt.player_list.clear();
    let p = world.objects.get_component::<Player>(&5001).unwrap();
    assert_eq!(p.transform_id, 0, "reverted");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&5001)
            .unwrap()
            .0
            .contains_key(&586),
        "transform skill removed"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&5001)
            .unwrap()
            .run_spd,
        base_run,
        "run speed restored"
    );
    assert_eq!(pbuffs(&world, 5001), 0, "buff cleared");

    // Mounted refusal: a strider rider casting the scroll gets SM 2063
    // (`ConditionPlayerCanTransform`'s `isMounted()` leg — a real mount sets
    // `mount_type`, not `transform_id`, so the polymorph leg alone misses it).
    crate::game_loop::admin::mounts::mount_player(&mut world, 5001, 12526, 1);
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    let refused = drain(&mut rx);
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::YOU_CANNOT_TRANSFORM_WHILE_RIDING_A_PET
        ),
        "mounted refusal sent"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .transform_id,
        0,
        "no transform applied while mounted"
    );
    crate::game_loop::admin::mounts::dismount(&mut world, 5001);

    // Sitting refusal — note *which* message. `Player.useMagic` turns away
    // every skill from a seated caster with SM 31, and it does so long before
    // it reaches `usedSkill.checkCondition`, so `ConditionPlayerCanTransform`'s
    // own `isSitting()` leg (SM 2283) is unreachable down the cast path in Java
    // too; it only answers for transforms that skip `useMagic` (the admin
    // `//transform`, gated separately). This assertion named 2283 while the
    // blanket seated-cast gate was still missing from the port.
    world
        .objects
        .get_component_mut::<Player>(&5001)
        .unwrap()
        .sitting = true;
    drain(&mut rx);
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(618, false));
    let refused = drain(&mut rx);
    assert!(
        has_system_message(
            &refused,
            server_packets::sm_ids::YOU_CANNOT_USE_ACTIONS_AND_SKILLS_WHILE_THE_CHARACTER_IS_SITTING
        ),
        "sitting refusal sent"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .transform_id,
        0,
        "no transform applied while sitting"
    );
}

/// G19 `MpConsumePerLevel` effect: the fighter-toggle upkeep half of
/// "Accuracy" (256, real dist data — `<effects>` carries both a real
/// `+3 Accuracy` `StatModifier`, already landing before this slice, and an
/// `MpConsumePerLevel` that previously fell through unrecognised, so the
/// toggle was a free buff). Toggling it on lands the stat *and* starts a
/// periodic MP drain; running the MP pool dry switches the toggle back off
/// (Java's `false`-return-cancels-a-toggle path, SM 140) and reverts the
/// stat, exactly like the DoT/ManaDamOverTime tick chain this effect shares.
#[test]
fn mp_consume_per_level_toggle_drains_mp_and_self_deactivates() {
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();

    let mut rx = ingame_player_access(&mut world, 1, 5101, 0);
    drain(&mut rx);
    world
        .objects
        .get_component_mut::<SkillBook>(&5101)
        .unwrap()
        .0
        .insert(256, 1);
    let base_accuracy = pcs(&world, 5101).accuracy;
    let mp_before = pvit(&world, 5101).cur_mp;

    // Toggle on: instant (no cast bar) — `+3 Accuracy` lands immediately.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(256, false));
    assert_eq!(pbuffs(&world, 5101), 1, "toggle landed as one buff");
    assert_eq!(
        pcs(&world, 5101).accuracy,
        base_accuracy + 3,
        "Accuracy +3 (DIFF) applied"
    );
    assert_eq!(
        pvit(&world, 5101).cur_mp,
        mp_before,
        "no MP deducted at cast time (pre-existing gap, not this slice)"
    );

    // One upkeep tick: `power(0.4) * ticksMultiplier(5 × 666 / 1000 = 3.33) ≈ 1.332` MP.
    advance_world(&mut world, 40); // interval = (5 × 666) / 100 = 33 ticks
    let mp_after_one_tick = pvit(&world, 5101).cur_mp;
    assert!(
        (mp_before - mp_after_one_tick - 1.332).abs() < 1e-6,
        "first tick drained ~1.332 MP: {mp_before} -> {mp_after_one_tick}"
    );
    assert_eq!(
        pbuffs(&world, 5101),
        1,
        "toggle still up (MP not exhausted yet)"
    );

    // Drain the rest of the pool: the toggle self-deactivates the moment a
    // tick's drain would exceed current MP (Java's `false` return).
    drain(&mut rx);
    advance_world(
        &mut world,
        40 * (mp_after_one_tick / 1.332).ceil() as u64 + 40,
    );
    assert_eq!(
        pbuffs(&world, 5101),
        0,
        "toggle switched itself off once MP ran dry"
    );
    assert_eq!(
        pcs(&world, 5101).accuracy,
        base_accuracy,
        "Accuracy reverted"
    );
    let packets = drain(&mut rx);
    assert!(
        has_system_message(
            &packets,
            server_packets::sm_ids::YOUR_SKILL_WAS_DEACTIVATED_DUE_TO_LACK_OF_MP
        ),
        "deactivation SystemMessage sent"
    );
}
