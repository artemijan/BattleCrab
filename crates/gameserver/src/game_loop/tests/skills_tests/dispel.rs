//! The dispel family and cure poison.

use super::*;

/// Cure Poison (1012) cleanses a POISON debuff via `DispelBySlot`: it removes a
/// landed Poison (129) DoT whose `abnormalLevel` is at or below the cure's
/// dispel level, and leaves a higher-level poison alone. Before the fix
/// `DispelBySlot` fell through the effect registry and the cure was a silent
/// no-op (the poison kept ticking).
#[test]
fn cure_poison_dispels_matching_poison_debuff() {
    use model::components::skills::Buffs;
    use model::skill::Skill;
    use model::skill::effects::SkillEffect;
    use model::skill::target::{OperateType, TargetType};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 31;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

    // The test world builds skills by hand (no real XML) — mirror the dist
    // values: Poison 129 (abnormalType POISON, abnormalLevel 3 @ lvl 1 / 7 @
    // lvl 4, a DamOverTime debuff) and Cure Poison 1012 (DispelBySlot POISON,3).
    let poison = |level: i32, abnormal_level: i32| Skill {
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
        id: 129,
        level,
        name: "Poison".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::EnemyOnly,
        magic_type: 1,
        magic_level: 20,
        effect_point: -204,
        cast_range: 600,
        effect_range: 1100,
        hit_time: 3000,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 2000,
        reuse_delay_group: -1,
        mp_consume: 8,
        mp_initial_consume: 2,
        hp_consume: 0,
        abnormal_time: 30,
        abnormal_level,
        abnormal_type: "POISON".into(),
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
        effects: vec![SkillEffect::DamOverTime {
            power: 24.0,
            ticks: 5,
            can_kill: false,
        }],
    };
    world.data.skill_data.insert_for_test(poison(1, 3));
    world.data.skill_data.insert_for_test(poison(4, 7));
    world.data.skill_data.insert_for_test(Skill {
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
        id: 1012,
        level: 1,
        name: "Cure Poison".into(),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Target,
        magic_type: 1,
        magic_level: 7,
        effect_point: 121,
        cast_range: 600,
        effect_range: 1100,
        hit_time: 4000,
        next_action: Default::default(),
        abnormal_resists: Vec::new(),
        hit_cancel_time: 0.0,
        cool_time: 0,
        reuse_delay: 4000,
        reuse_delay_group: -1,
        mp_consume: 8,
        mp_initial_consume: 2,
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
        effects: vec![SkillEffect::DispelBySlot {
            dispel: vec![("POISON".into(), 3)],
        }],
    });

    let poison1 = world.data.skill_data.get(129, 1).unwrap().clone();
    let poison4 = world.data.skill_data.get(129, 4).unwrap().clone();
    let cure = world.data.skill_data.get(1012, 1).unwrap().clone();

    // Land Poison lvl 1 (abnormalLevel 3) on the mob.
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &poison1);
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .len(),
        1,
        "poison landed"
    );

    // Cure Poison lvl 1 dispels POISON up to level 3 → the debuff is removed.
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &cure);
    assert!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .is_empty(),
        "poison cured"
    );

    // A higher-level poison (lvl 4, abnormalLevel 7) is above Cure Poison lvl 1's
    // reach (POISON,3) and survives the cleanse.
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &poison4);
    effects::apply_skill_effects(&mut world, 3001, npc_oid, &cure);
    assert_eq!(
        world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .len(),
        1,
        "a poison above the cure's dispel level is not removed",
    );
}

/// G19 `DispelByCategory` (the "Cancel" family: Cancellation, Cleanse,
/// Purification Field, Touch of Death) — unlike `DispelBySlot`/
/// `DispelBySlotProbability` (a fixed abnormal-type list) this steals
/// *whatever* is up. Before this slice the effect name fell through
/// unregistered and every skill in the family cast but stripped nothing.
mod dispel_by_category {
    use super::*;
    use model::components::skills::Buffs;
    use model::skill::Skill;
    use model::skill::effects::{DispelSlot, SkillEffect, StatModifierEffect};
    use model::skill::target::{AffectObject, AffectScope, OperateType, TargetType};
    use model::stats::{Stat, StatModifierType};

    /// A minimal continuous skill — override `id`/`magic_type`/`effect_point`/
    /// `can_be_dispelled`/`is_debuff`/`effects` per case.
    fn base_skill(id: i32, name: &str) -> Skill {
        Skill {
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
            id,
            level: 1,
            name: name.into(),
            operate_type: OperateType::Active,
            is_continuous: true,
            target_type: TargetType::Target,
            magic_type: 1,
            magic_level: 20,
            effect_point: 100,
            cast_range: 600,
            effect_range: 900,
            hit_time: 1000,
            next_action: Default::default(),
            abnormal_resists: Vec::new(),
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 0,
            reuse_delay_group: -1,
            mp_consume: 0,
            mp_initial_consume: 0,
            hp_consume: 0,
            abnormal_time: 120,
            abnormal_level: 1,
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
            effects: Vec::new(),
        }
    }

    fn stat_buff(stat: Stat, amount: f64) -> SkillEffect {
        SkillEffect::StatModifier(StatModifierEffect {
            stat,
            mode: StatModifierType::Diff,
            amount,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        })
    }

    /// `BUFF` slot: dances are tried before ordinary buffs (Java's
    /// `getDances()` walked before `getBuffs()`, both in reverse cast order),
    /// and `can_be_dispelled=false` is respected.
    #[test]
    fn buff_slot_prefers_dances_and_respects_cant_dispel() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = NPC_OID + 40;
        spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

        let mut buff = base_skill(9001, "Regular Buff");
        buff.target_type = TargetType::Target;
        buff.effects = vec![stat_buff(Stat::MaxHp, 100.0)];

        let mut undispellable = base_skill(9002, "Undispellable Buff");
        undispellable.target_type = TargetType::Target;
        undispellable.can_be_dispelled = false;
        undispellable.effects = vec![stat_buff(Stat::MaxMp, 100.0)];

        let mut dance = base_skill(9003, "A Dance");
        dance.target_type = TargetType::Target;
        dance.magic_type = 3; // isMagic==3 -> Dance slot
        dance.effects = vec![stat_buff(Stat::MaxCp, 100.0)];

        let mut cancel = base_skill(1056, "Cancellation");
        cancel.target_type = TargetType::Target;
        cancel.magic_level = 40; // higher than the buffs' 20 so calcCancelSuccess isn't needed at rate=100
        cancel.effects = vec![SkillEffect::DispelByCategory {
            slot: DispelSlot::Buff,
            rate: 100,
            max: 1,
        }];

        for s in [&buff, &undispellable, &dance] {
            world.data.skill_data.insert_for_test(s.clone());
            effects::apply_continuous_effects(&mut world, 3001, npc_oid, s, None);
        }
        assert_eq!(
            world
                .objects
                .get_component::<Buffs>(&npc_oid)
                .unwrap()
                .0
                .len(),
            3,
            "all three landed"
        );

        effects::apply_skill_effects(&mut world, 3001, npc_oid, &cancel);

        let remaining: Vec<i32> = world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .iter()
            .map(|b| b.skill_id)
            .collect();
        assert_eq!(
            remaining,
            vec![9001, 9002],
            "the dance (9003) is stolen first, max=1 stops there"
        );
    }

    /// `DEBUFF` slot (Cleanse/Purification Field, rate 100): strips debuffs
    /// only, leaving positive buffs on the same target untouched.
    #[test]
    fn debuff_slot_strips_only_debuffs() {
        let (mut world, _db_rx, _link_rx) = combat_test_world();
        let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
        let npc_oid = NPC_OID + 41;
        spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 50);

        let mut buff = base_skill(9001, "Regular Buff");
        buff.target_type = TargetType::Target;
        buff.effects = vec![stat_buff(Stat::MaxHp, 100.0)];

        let mut debuff = base_skill(9010, "A Debuff");
        debuff.target_type = TargetType::Target;
        debuff.effect_point = -50;
        debuff.is_debuff = true;
        debuff.effects = vec![stat_buff(Stat::PhysicalDefence, -20.0)];

        let mut cleanse = base_skill(1409, "Cleanse");
        cleanse.target_type = TargetType::Target;
        cleanse.effects = vec![SkillEffect::DispelByCategory {
            slot: DispelSlot::Debuff,
            rate: 100,
            max: 10,
        }];

        world.data.skill_data.insert_for_test(buff.clone());
        world.data.skill_data.insert_for_test(debuff.clone());
        effects::apply_continuous_effects(&mut world, 3001, npc_oid, &buff, None);
        effects::apply_continuous_effects(&mut world, 3001, npc_oid, &debuff, None);
        assert_eq!(
            world
                .objects
                .get_component::<Buffs>(&npc_oid)
                .unwrap()
                .0
                .len(),
            2,
            "both landed"
        );

        effects::apply_skill_effects(&mut world, 3001, npc_oid, &cleanse);

        let remaining: Vec<i32> = world
            .objects
            .get_component::<Buffs>(&npc_oid)
            .unwrap()
            .0
            .iter()
            .map(|b| b.skill_id)
            .collect();
        assert_eq!(
            remaining,
            vec![9001],
            "the debuff is stripped, the buff stays"
        );
    }
}

/// The `RequestDispel` ex body after the sub-opcode: objectId, skillId,
/// skillLevel (short), skillSubLevel (short).
fn dispel_body(object_id: i32, skill_id: i32, skill_level: i32, skill_sub_level: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(object_id);
    w.write_i32(skill_id);
    w.write_i16(skill_level as i16);
    w.write_i16(skill_sub_level as i16);
    w.into_bytes()
}

/// Alt+clicking a normal self-buff strips it: the buff leaves and its stat
/// contribution (P.Def +8%) reverts.
#[test]
fn dispel_removes_self_buff() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let buff = synthetic_buff(9200, 1, "MYBUFF", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    assert!(has_buff(&world, 3001, 9200), "buff landed");
    assert_eq!(pbuffs(&world, 3001), 1);

    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9200, 1, 0));
    assert!(!has_buff(&world, 3001, 9200), "alt+click removed the buff");
    assert_eq!(pbuffs(&world, 3001), 0, "buff slot freed after dispel");
}

/// A debuff can't be self-dispelled (Java `skill.isDebuff()` guard), even though
/// it sits in the buff list.
#[test]
fn dispel_refuses_debuff() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let mut debuff = synthetic_buff(9201, 1, "MYDEBUFF", 1, 1);
    debuff.is_debuff = true;
    world.data.skill_data.insert_for_test(debuff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &debuff);
    assert!(has_buff(&world, 3001, 9201), "debuff landed");

    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9201, 1, 0));
    assert!(
        has_buff(&world, 3001, 9201),
        "debuff cannot be alt+click dispelled"
    );
}

/// A skill flagged `canBeDispelled=false` survives an alt+click.
#[test]
fn dispel_refuses_undispellable_buff() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let mut buff = synthetic_buff(9202, 1, "MYBUFF", 1, 1);
    buff.can_be_dispelled = false;
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    assert!(has_buff(&world, 3001, 9202), "buff landed");

    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9202, 1, 0));
    assert!(
        has_buff(&world, 3001, 9202),
        "undispellable buff survives alt+click"
    );
}

/// A dance/song (`magic_type == 3`) is only strippable when `DanceCancelBuff`
/// is on — this dist's Character.ini sets it True.
#[test]
fn dispel_dance_gated_by_config() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let dance = synthetic_buff(9203, 1, "MYDANCE", 1, 3);
    world.data.skill_data.insert_for_test(dance.clone());

    // Config off: the dance is not removed.
    world.cfg.character.dance_cancel_buff = false;
    apply_skill_effects(&mut world, 3001, 3001, &dance);
    assert!(has_buff(&world, 3001, 9203), "dance landed");
    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9203, 1, 0));
    assert!(
        has_buff(&world, 3001, 9203),
        "dance kept while DanceCancelBuff is off"
    );

    // Config on (this dist's default): the dance is removed.
    world.cfg.character.dance_cancel_buff = true;
    handle_request_dispel(&mut world, 1, &dispel_body(3001, 9203, 1, 0));
    assert!(
        !has_buff(&world, 3001, 9203),
        "dance removed while DanceCancelBuff is on"
    );
}

/// A dispel aimed at a foreign object id (not the player's own, nor their
/// summon) is a no-op for the player's buffs.
#[test]
fn dispel_wrong_object_id_ignored() {
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    let buff = synthetic_buff(9204, 1, "MYBUFF", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    assert!(has_buff(&world, 3001, 9204), "buff landed");

    handle_request_dispel(&mut world, 1, &dispel_body(9999, 9204, 1, 0));
    assert!(
        has_buff(&world, 3001, 9204),
        "dispel on a foreign object id leaves the player's buff"
    );
}

/// **Alt+click dispel on a summon's buff strips it off the summon** (Java's
/// `getPet()` / `getServitor(_objectId)` branch), leaving the player's own buff.
#[test]
fn dispel_strips_a_summon_buff() {
    use crate::game_loop::servitor::summon_servitor;
    use crate::game_loop::skills::{effects::apply_skill_effects, handle_request_dispel};
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    // Register a servitor template and summon one.
    let mut t = crate::data::npc_data::default_template(14799);
    t.type_name = "Servitor".into();
    t.base_hp_max = 400.0;
    t.base_mp_max = 200.0;
    world.data.npc_data.insert_for_test(t);
    let servitor = summon_servitor(&mut world, 3001, 14799, 283, 1200, 0, 0).expect("summoned");

    // Put the same buff on both the owner and the servitor.
    let buff = synthetic_buff(9210, 1, "MYBUFF", 1, 1);
    world.data.skill_data.insert_for_test(buff.clone());
    apply_skill_effects(&mut world, 3001, 3001, &buff);
    apply_skill_effects(&mut world, 3001, servitor, &buff);
    assert!(has_buff(&world, 3001, 9210) && has_buff(&world, servitor, 9210));

    // Alt+click the servitor's buff → removed from the servitor only.
    handle_request_dispel(&mut world, 1, &dispel_body(servitor, 9210, 1, 0));
    assert!(
        !has_buff(&world, servitor, 9210),
        "the summon's buff was stripped"
    );
    assert!(
        has_buff(&world, 3001, 9210),
        "the owner's own buff is untouched"
    );
}
