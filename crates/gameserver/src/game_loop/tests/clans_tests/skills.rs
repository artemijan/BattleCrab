//! Clan skills: granting and purging them, siege skills, reapplying on
//! login, the reputation a pledge skill costs, and what each does to the
//! member's max HP/MP/CP.

use super::*;

/// `//give_clan_skills` (Java `adminGiveClanSkills`): the clan learns every
/// pledge skill it qualifies for at its level; each applies to online members
/// gated by social class, lands as a (passive, icon-less) stat buff, shows in
/// the merged SkillList, and persists. Dispersing the clan strips them again.
#[test]
fn give_clan_skills_grants_gates_and_persists() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::skills::{Buffs, ClanSkills};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Two clan skills: 370 gated at HEIR (ordinal 3), 371 gated at COUNT (8).
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(371));
    let learn = |id, social| PledgeSkillLearn {
        skill_id: id,
        skill_level: 1,
        get_level: 3,
        social_class: Some(social),
        residencial: false,
        residence_ids: Vec::new(),
        level_up_sp: 0,
    };
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(370, 3), false);
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(371, 8), false);

    // A level-8 clan: leader 3001 (pledge class 8 → social 9), member 3002
    // (pledge class 5 → social 6). Both online.
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0055;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "SkillClan".into(),
            leader_id: 3001,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001), cm(3002)],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = clan_id;
    }

    let clan_skill = |world: &World, oid: i32, id: i32| {
        world
            .objects
            .get_component::<ClanSkills>(&oid)
            .is_some_and(|c| c.0.contains_key(&id))
    };
    let has_passive_buff = |world: &World, oid: i32, id: i32| {
        world
            .objects
            .get_component::<Buffs>(&oid)
            .is_some_and(|b| b.0.iter().any(|x| x.skill_id == id && x.passive))
    };

    let count = clans::give_clan_skills(&mut world, clan_id, false);
    assert_eq!(
        count, 2,
        "clan learns both level-3 pledge skills at clan level 8"
    );

    // Stored on the clan and persisted.
    assert_eq!(world.clans[&clan_id].skills.get(&370), Some(&1));
    assert_eq!(world.clans[&clan_id].skills.get(&371), Some(&1));
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::SaveClanSkill { clan_id: c, skill_id: 370, skill_level: 1, .. } if *c == clan_id)));
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::SaveClanSkill { skill_id: 371, .. }))
    );

    // Leader (social 9) gets both; member (social 6) gets only the HEIR skill.
    assert!(
        clan_skill(&world, 3001, 370) && clan_skill(&world, 3001, 371),
        "leader gets both"
    );
    assert!(
        clan_skill(&world, 3002, 370),
        "member qualifies for the HEIR skill"
    );
    assert!(
        !clan_skill(&world, 3002, 371),
        "member is gated out of the COUNT skill"
    );
    // Applied skills land as icon-less passive buffs (stat effect, no abnormal row).
    assert!(
        has_passive_buff(&world, 3001, 370),
        "clan skill applied as a passive buff"
    );
    assert!(
        !has_passive_buff(&world, 3002, 371),
        "gated-out skill not applied"
    );

    // The clan skill shows in the member's merged SkillList (opcode 0x5F).
    let pkt = helpers::skill_list_packet(&world, 3001).expect("skill list");
    assert_eq!(pkt[0], 0x5F);
    let count_in_list = i32::from_le_bytes(pkt[1..5].try_into().unwrap());
    assert!(
        count_in_list >= 2,
        "leader's skill list carries the 2 clan skills"
    );

    // Dispersing the clan strips the clan skills from the (still-online) members.
    clans::destroy_clan(&mut world, clan_id);
    assert!(
        !clan_skill(&world, 3001, 370) && !clan_skill(&world, 3001, 371),
        "leader clan skills cleared on disperse"
    );
    assert!(
        !has_passive_buff(&world, 3001, 370),
        "leader clan-skill buff reverted"
    );
    assert!(
        !clan_skill(&world, 3002, 370),
        "member clan skills cleared on disperse"
    );
}

/// `//give_clan_skills` self-heal: a clan carrying a residence skill (stored by
/// a pre-fix grant that read the wrong attribute) has it purged — removed from
/// the clan, reverted on online members, DB row deleted — while the grant
/// (re-)applies the real clan skills immediately and reports the clan's actual
/// skill count (not 0) even when it already owned them.
#[test]
fn give_clan_skills_purges_residence_and_reapplies() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::skills::ClanSkills;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Clan skill 370 (HEIR, non-residence) + residence skill 590.
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(590));
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn {
            skill_id: 370,
            skill_level: 1,
            get_level: 3,
            social_class: Some(3),
            residencial: false,
            residence_ids: Vec::new(),
            level_up_sp: 0,
        },
        false,
    );
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn {
            skill_id: 590,
            skill_level: 1,
            get_level: 4,
            social_class: None,
            residencial: true,
            residence_ids: Vec::new(),
            level_up_sp: 0,
        },
        false,
    );

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0056;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    // The clan already "owns" 370 and a residence 590 (as a pre-fix grant left it),
    // and the residence skill is applied to the online leader.
    let mut skills = std::collections::HashMap::new();
    skills.insert(370, 1);
    skills.insert(590, 1);
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "ResClan".into(),
            leader_id: 3001,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001)],
            skills,
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;
    world.objects.add_components(
        &3001,
        ClanSkills(std::collections::HashMap::from([(590, 1)])),
    );

    let count = clans::give_clan_skills(&mut world, clan_id, false);

    // Residence skill purged from the clan and the member; real skill re-applied.
    assert!(
        !world.clans[&clan_id].skills.contains_key(&590),
        "residence skill purged from clan"
    );
    assert!(
        world.clans[&clan_id].skills.contains_key(&370),
        "clan skill kept"
    );
    let leader_skills = world.objects.get_component::<ClanSkills>(&3001).unwrap();
    assert!(
        !leader_skills.0.contains_key(&590),
        "residence skill reverted on the member"
    );
    assert!(
        leader_skills.0.contains_key(&370),
        "real clan skill applied immediately (no relog)"
    );
    // DB row deleted so a relog can't re-apply it.
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(c, db::DbCommand::DeleteClanSkill { clan_id: c, skill_id: 590 } if *c == clan_id)), "residence DB row deleted");
    // Saturated clan still reports its real (non-residence) skill count, not 0.
    assert_eq!(count, 1, "reports the clan's applied skill count");
}

/// `Max{Hp,Mp,Cp}Finalizer`: the buff `mul`/`add` modifiers apply as
/// `mul·(base·statBonus) + add`, with equipped-item bonuses added *after* the
/// mul (Java doesn't scale item bonuses by the buff). Regression for the G7 gap
/// where these finalizers ignored buff modifiers entirely.
#[test]
fn max_vitals_finalizers_apply_buff_modifiers() {
    use crate::model::components::stats::StatModifiers;
    use crate::model::stats::Stat;

    let (world, _db_rx, _link_rx) = quest_test_world();
    let t = world.data.player_templates.get(0).cloned().unwrap();
    let mut mods = StatModifiers::default();
    mods.mul.insert(Stat::MaxHp, 1.5);
    mods.add.insert(Stat::MaxHp, 100.0);
    mods.mul.insert(Stat::MaxMp, 2.0);
    mods.mul.insert(Stat::MaxCp, 1.2);

    let hp_base = t.base_hp_max(80) * world.data.stat_bonus.con_bonus(t.base_con);
    let mp_base = t.base_mp_max(80) * world.data.stat_bonus.men_bonus(t.base_men);
    let cp_base = t.base_cp_max(80) * world.data.stat_bonus.con_bonus(t.base_con);

    let hp = model::max_vitals::calc_max_hp(&world.data, &t, 80, None, &mods);
    let mp = model::max_vitals::calc_max_mp(&world.data, &t, 80, None, &mods);
    let cp = model::max_vitals::calc_max_cp(&world.data, &t, 80, &mods);
    assert!(
        (hp - (1.5 * hp_base + 100.0)).abs() < 1e-6,
        "MaxHp = mul*base + add"
    );
    assert!((mp - (2.0 * mp_base)).abs() < 1e-6, "MaxMp = mul*base");
    assert!((cp - (1.2 * cp_base)).abs() < 1e-6, "MaxCp = mul*base");
    // Empty mods leave the base untouched (mul=1, add=0).
    let none = StatModifiers::default();
    assert!(
        (model::max_vitals::calc_max_hp(&world.data, &t, 80, None, &none) - hp_base).abs() < 1e-6
    );
}

/// The admin `//superhaste 4` case (Super Haste 7029 L4, a toggle): its
/// `+100% MaxMp` effect must double the MP bar through the active-buff path
/// (`apply_skill_effects` → `recompute_max_vitals`). This is the modifier that
/// was missing from the Archmage's MP (Java applied it, Rust didn't recompute
/// the vitals for it).
#[test]
fn superhaste_maxmp_doubles_mp() {
    use crate::model::components::stats::Vitals;

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    // The real Super Haste 7029 L4 from the datapack (+100% MaxMp, PER).
    let sh = dist::skills()
        .get(7029, 4)
        .expect("Super Haste 7029 L4")
        .clone();
    world.data.skill_data.insert_for_test(sh.clone());

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let base_mp = world.objects.get_component::<Vitals>(&3001).unwrap().max_mp;

    effects::apply_skill_effects(&mut world, 3001, 3001, &sh);

    let after = world.objects.get_component::<Vitals>(&3001).unwrap().max_mp;
    assert!(
        (after - base_mp * 2).abs() <= 1,
        "Super Haste +100% MaxMp doubles the bar: {base_mp} -> {after}"
    );
}

/// Login path: a passive skill in the character's book that carries a `MaxMp`
/// modifier (a mystic's MP passives — most of an Archmage's MP pool) is folded
/// into the vitals at load. Regression: `from_char` computed the vitals before
/// applying the passive skills, so the boosted MP never reached the first
/// `UserInfo` (the character showed only its base MP).
#[test]
fn passive_max_mp_skill_boosts_mp_at_login() {
    use crate::model::components::stats::StatModifiers;
    use crate::model::skill::effects::{SkillEffect, StatModifierEffect};
    use crate::model::skill::target::OperateType;
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    // A passive skill that doubles MaxMp (+100%), like a stacked mage MP passive.
    let mut s = passive_clan_test_skill(9001);
    s.operate_type = OperateType::Passive;
    s.effects = vec![SkillEffect::StatModifier(StatModifierEffect {
        stat: Stat::MaxMp,
        mode: StatModifierType::Per,
        amount: 100.0,
        armor_condition: 0,
        weapon_condition: 0,
        qualifier: None,
        two_handed: false,
        hp_percent: 0,
    })];
    world.data.skill_data.insert_for_test(s);

    let t = world.data.player_templates.get(0).cloned().unwrap();
    let base_mp =
        model::max_vitals::calc_max_mp(&world.data, &t, 1, None, &StatModifiers::default());

    let mut chr = dummy_char(7001, "Mage");
    chr.skills = vec![(9001, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    assert_eq!(
        bundle.vitals.max_mp,
        (base_mp * 2.0) as i32,
        "passive MaxMp folded into max_mp at login"
    );
}

/// End-to-end: clan skills carrying `MaxHp`/`MaxMp`/`MaxCp` modifiers (Clan
/// Health / Clan Mind, the Archmage clan-leader case) now move the HP/MP/CP bar
/// immediately — `%` modifiers stack multiplicatively, flat ones add. Regression
/// for the bug where these clan skills applied as buffs but never changed the
/// vitals (the finalizers ignored the modifier maps).
#[test]
fn clan_skills_move_max_hp_mp_cp() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::stats::{PlayerVitals, StatModifiers, Vitals};
    use crate::model::skill::effects::{SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, mut db_rx, _link_rx) = quest_test_world();

    // Skill 370: +100% MaxMp and +300 flat MaxHp. Skill 371: +50% MaxMp, +20% MaxCp.
    for (id, effs) in [
        (
            370,
            vec![
                (Stat::MaxMp, StatModifierType::Per, 100.0),
                (Stat::MaxHp, StatModifierType::Diff, 300.0),
            ],
        ),
        (
            371,
            vec![
                (Stat::MaxMp, StatModifierType::Per, 50.0),
                (Stat::MaxCp, StatModifierType::Per, 20.0),
            ],
        ),
    ] {
        let mut s = passive_clan_test_skill(id);
        s.effects = effs
            .into_iter()
            .map(|(stat, mode, amount)| {
                SkillEffect::StatModifier(StatModifierEffect {
                    stat,
                    mode,
                    amount,
                    armor_condition: 0,
                    weapon_condition: 0,
                    qualifier: None,
                    two_handed: false,
                    hp_percent: 0,
                })
            })
            .collect();
        world.data.skill_data.insert_for_test(s);
        world.data.pledge_skill_trees.insert_for_test(
            PledgeSkillLearn {
                skill_id: id,
                skill_level: 1,
                get_level: 1,
                social_class: None,
                residencial: false,
                residence_ids: Vec::new(),
                level_up_sp: 0,
            },
            false,
        );
    }

    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    // Exact pre-buff maxima (empty modifier maps).
    let (base_hp, base_mp, base_cp) = {
        let p = world.objects.get_component::<Player>(&3001).unwrap();
        let t = world
            .data
            .player_templates
            .get(p.class_id)
            .cloned()
            .unwrap();
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        let none = StatModifiers::default();
        (
            model::max_vitals::calc_max_hp(&world.data, &t, p.level, Some(inv), &none),
            model::max_vitals::calc_max_mp(&world.data, &t, p.level, Some(inv), &none),
            model::max_vitals::calc_max_cp(&world.data, &t, p.level, &none),
        )
    };

    let clan_id = 0x3000_00AA;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "VitalClan".into(),
            leader_id: 3001,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001)],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;

    clans::give_clan_skills(&mut world, clan_id, false);

    let v = *world.objects.get_component::<Vitals>(&3001).unwrap();
    let pv = *world.objects.get_component::<PlayerVitals>(&3001).unwrap();
    // MaxMp: two % buffs stack multiplicatively (2.0 * 1.5 = 3.0).
    assert_eq!(
        v.max_mp,
        (base_mp * 3.0) as i32,
        "MaxMp % buffs stacked onto the bar"
    );
    // MaxHp: flat +300.
    assert_eq!(
        v.max_hp,
        (base_hp + 300.0) as i32,
        "flat MaxHp buff applied"
    );
    // MaxCp: +20%.
    assert_eq!(pv.max_cp, (base_cp * 1.2) as i32, "MaxCp % buff applied");
}

/// Siege/leader skills (Java `SiegeManager.addSiegeSkills`): a clan leader gains
/// Build Headquarters (247) + Imprint of Light/Darkness (19034/19035) once the
/// clan reaches level 5, the two Outpost skills (844/845) only with a castle;
/// regular members get none. Delivered through the transient [`ClanSkills`]
/// channel so they show in the merged SkillList without persisting.
#[test]
fn siege_skills_granted_to_level5_clan_leader_only() {
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::skills::ClanSkills;

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let clan_id = 0x3000_0077;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "SiegeClan".into(),
            leader_id: 3001,
            level: 4,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001), cm(3002)],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = clan_id;
    }

    let has = |world: &World, oid: i32, id: i32| {
        world
            .objects
            .get_component::<ClanSkills>(&oid)
            .is_some_and(|c| c.0.contains_key(&id))
    };

    // Level 4: below the siege min level — the leader gets no siege skills.
    clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        !has(&world, 3001, 247),
        "no siege skills below clan level 5"
    );

    // Reaching level 5 grants the three core siege skills to the online leader.
    clans::set_clan_level(&mut world, clan_id, 5);
    for id in [247, 19034, 19035] {
        assert!(
            has(&world, 3001, id),
            "leader gains siege skill {id} at clan level 5"
        );
    }
    // No castle yet → no Outpost skills.
    assert!(
        !has(&world, 3001, 844) && !has(&world, 3001, 845),
        "Outpost skills need a castle"
    );
    // A regular member never gets siege skills.
    clans::on_enter_world(&mut world, 2, 3002);
    assert!(
        !has(&world, 3002, 247),
        "non-leader member gets no siege skills"
    );

    // Owning a castle adds the two Outpost skills on the leader's next login.
    world.clans.get_mut(&clan_id).unwrap().castle_id = 3;
    clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        has(&world, 3001, 844) && has(&world, 3001, 845),
        "castle owner gets Outpost skills"
    );
}

/// A member logging in re-derives the clan's skills (Java `addSkillEffects` on
/// enter-world), gated by social class — nothing is persisted on the player.
#[test]
fn clan_skills_reapply_on_member_login() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;
    use crate::model::clan::{Clan, ClanMember};
    use crate::model::components::skills::ClanSkills;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    world.data.pledge_skill_trees.insert_for_test(
        PledgeSkillLearn {
            skill_id: 370,
            skill_level: 1,
            get_level: 3,
            social_class: Some(3),
            residencial: false,
            residence_ids: Vec::new(),
            level_up_sp: 0,
        },
        false,
    );
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0066;
    let cm = |id: i32| ClanMember {
        char_id: id,
        name: format!("P{id}"),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    };
    // The clan already knows skill 370 (as if loaded from clan_skills).
    let mut skills = std::collections::HashMap::new();
    skills.insert(370, 1);
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "SkillClan".into(),
            leader_id: 3001,
            level: 8,
            reputation_score: 0,
            castle_id: 0,
            members: vec![cm(3001)],
            skills,
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;

    // Simulate the leader's login → clan skills re-applied from the clan.
    clans::on_enter_world(&mut world, 1, 3001);
    assert!(
        world
            .objects
            .get_component::<ClanSkills>(&3001)
            .is_some_and(|c| c.0.contains_key(&370)),
        "clan skills re-derived on login"
    );
    // Nothing was written to the player's own persisted skill book.
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&3001)
            .is_some_and(|b| !b.0.contains_key(&370)),
        "clan skill is transient — never in the persisted SkillBook"
    );
}

/// The pledge-skill learn flow: the leader-only list (`ExAcquirableSkillListBy
/// Class` PLEDGE), the reputation gate, and a successful learn — rep deducted
/// + persisted, the skill stored/broadcast, and the refreshed list offering
///   the next level.
#[test]
fn pledge_skill_learning_spends_reputation() {
    use crate::data::pledge_skill_tree::PledgeSkillLearn;

    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(370));
    let learn = |lvl: i32, sp: i64| PledgeSkillLearn {
        skill_id: 370,
        skill_level: lvl,
        get_level: 3,
        social_class: None,
        residencial: false,
        residence_ids: Vec::new(),
        level_up_sp: sp,
    };
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(1, 1_500), false);
    world
        .data
        .pledge_skill_trees
        .insert_for_test(learn(2, 3_000), false);
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001, 3002]);
    world.clans.get_mut(&5000).unwrap().level = 3;
    drain_db(&mut db_rx);

    // Non-leader asking for the list → NotClanLeader.htm (an NpcHtmlMessage).
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_learn_clan_skills")),
    );
    assert!(
        drain(&mut b_rx)
            .iter()
            .any(|p| decode_npc_html(p).is_some()),
        "NotClanLeader html shown"
    );

    // Leader: the PLEDGE learnable list with the level-1 entry.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_learn_clan_skills")),
    );
    let pkts = drain(&mut a_rx);
    let list = pkts
        .iter()
        .find(|p| is_ex(p, 0xFA))
        .expect("ExAcquirableSkillListByClass sent");
    assert_eq!(i16::from_le_bytes([list[3], list[4]]), 2, "PLEDGE type");

    // The info request answers with the reputation cost.
    clans::handle_request_pledge_skill_info(&world, 1, 370, 1);
    let info = drain(&mut a_rx);
    assert!(
        info.iter()
            .any(|p| p[0] == server_packets::opcodes::ACQUIRE_SKILL_INFO)
    );

    // Learning without reputation fails.
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 1);
    assert!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::SKILL_ACQUIRE_FAILED_INSUFFICIENT_CLAN_REPUTATION)
    );
    assert!(world.clans[&5000].skills.is_empty());

    // Skipping a level is silently refused (Java's prev-level hack check).
    world.clans.get_mut(&5000).unwrap().reputation_score = 10_000;
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 2);
    assert!(world.clans[&5000].skills.is_empty());

    // A successful learn: rep −1500 (persisted), skill stored + pushed.
    drain_db(&mut db_rx);
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 1);
    assert_eq!(world.clans[&5000].skills.get(&370), Some(&1));
    assert_eq!(world.clans[&5000].reputation_score, 8_500);
    let a_pkts = drain(&mut a_rx);
    let a_sms = ids_after_opcode(&a_pkts, server_packets::opcodes::SYSTEM_MESSAGE);
    assert!(a_sms.contains(
        &server_packets::sm_ids::S1_POINTS_HAVE_BEEN_DEDUCTED_FROM_THE_CLAN_S_REPUTATION
    ));
    assert!(a_sms.contains(&server_packets::sm_ids::THE_CLAN_SKILL_S1_HAS_BEEN_ADDED));
    assert!(
        a_pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ACQUIRE_SKILL_DONE)
    );
    let cmds = drain_db(&mut db_rx);
    assert!(cmds.iter().any(|c| matches!(
        c,
        db::DbCommand::UpdateClanReputation {
            clan_id: 5000,
            reputation: 8_500
        }
    )));
    assert!(cmds.iter().any(|c| matches!(
        c,
        db::DbCommand::SaveClanSkill {
            clan_id: 5000,
            skill_id: 370,
            skill_level: 1,
            ..
        }
    )));
    // The member got the passive too (no social gate on the fixture).
    assert!(
        world
            .objects
            .get_component::<model::components::skills::ClanSkills>(&3002)
            .is_some_and(|c| c.0.get(&370) == Some(&1))
    );

    // The re-shown list now offers level 2.
    clans::handle_learn_pledge_skill(&mut world, 1, 370, 2);
    assert_eq!(world.clans[&5000].skills.get(&370), Some(&2));
    assert_eq!(world.clans[&5000].reputation_score, 5_500);
}

// --- G18 slice 3: ranks & power grades ---
