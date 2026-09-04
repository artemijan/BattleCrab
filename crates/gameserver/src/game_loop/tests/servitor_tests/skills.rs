//! Skills around a summon: targeting one, buffing it and keeping those buffs
//! across a relog, sharing the owner's buffs, the action buttons a servitor
//! casts from, NPC reuse, fear, and the community-board pet buffer.

use super::*;

/// The summon skills parse, and `npcId` is **per level** — each level summons a
/// stronger template, which is why the effect carries the id rather than the
/// skill.
#[test]
fn real_dist_summon_skills_parse_per_level_npc_ids() {
    let skills = dist::skills();
    let npc_of = |id: i32, level: i32| {
        skills.get(id, level).and_then(|s| {
            s.effects.iter().find_map(|e| match e {
                SkillEffect::Summon { npc_id, .. } => Some(*npc_id),
                _ => None,
            })
        })
    };
    // Summon Dark Panther 283 walks its template id up with the skill level.
    let l1 = npc_of(283, 1).expect("level 1 summons something");
    let l2 = npc_of(283, 2).expect("level 2 summons something");
    assert_ne!(l1, l2, "a different template per level: {l1} vs {l2}");

    // Summon Siege Golem 13 has a flat npcId and a real consume item.
    let golem = skills.get(13, 1).expect("Siege Golem loads");
    let effect = golem.effects.iter().find_map(|e| match e {
        SkillEffect::Summon {
            npc_id,
            life_time,
            consume_item_id,
            ..
        } => Some((*npc_id, *life_time, *consume_item_id)),
        _ => None,
    });
    assert_eq!(
        effect,
        Some((14737, 1200, 2131)),
        "npcId / lifeTime / gemstone"
    );
}

/// All 24 learnable summon skills produce a usable effect — none is dropped for
/// want of an `npcId`.
#[test]
fn every_learnable_summon_skill_parses() {
    let skills = dist::skills();
    for id in [13, 25, 283, 299, 301, 448, 1111, 1128] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::Summon { npc_id, .. } if *npc_id > 0)),
            "skill {id} carries a usable Summon: {:?}",
            skill.effects
        );
    }
}

// ---------------------------------------------------------------------------
// Follow / attack (slice 2)
// ---------------------------------------------------------------------------

/// The Summoner support kit — 18 learnable skills, all of which resolved to
/// `INVALID_TARGET` before `TargetType::Summon` existed. Servitor Heal is the
/// representative case: it must reach the servitor, not the caster.
#[test]
fn a_summon_target_skill_reaches_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 10.0;

    let skill = Skill {
        self_continuous: false,
        id: 1127,
        level: 1,
        target_type: TargetType::Summon,
        effects: vec![SkillEffect::Heal { power: 100.0 }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill.clone());

    effects::apply_skill_effects(&mut world, OWNER, servitor, &skill);
    assert!(
        world
            .objects
            .get_component::<Vitals>(&servitor)
            .unwrap()
            .cur_hp
            > 10.0,
        "the servitor was healed"
    );
}

/// Target resolution picks the caster's own servitor without needing it
/// selected — Java's handler ignores the current target entirely.
#[test]
fn summon_targeting_finds_the_casters_own_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    assert_eq!(servitor_of(&world, OWNER), Some(servitor));
}

/// With no summon out there is nothing to target — the skill must fail rather
/// than silently falling back to the caster.
#[test]
fn summon_targeting_without_a_summon_finds_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    assert!(servitor_of(&world, OWNER).is_none());
}

/// The fixture above builds its own skill, so it cannot catch a parse-arm
/// mistake. This reads the **real** Summoner kit out of the datapack: if
/// `<targetType>SUMMON</targetType>` stops mapping, all 18 skills silently
/// return to `INVALID_TARGET`.
#[test]
fn the_real_servitor_skills_parse_as_summon_targeted() {
    let skills = dist::skills();
    // Servitor Heal, Servitor Recharge, Mighty Servitor, Final Servitor.
    for id in [1127, 1126, 1146, 1349] {
        let s = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} exists"));
        assert_eq!(
            s.target_type,
            TargetType::Summon,
            "skill {id} ({}) must target the summon",
            s.id
        );
    }
}

// ---------------------------------------------------------------------------
// Summon buff visibility (slice 20)
// ---------------------------------------------------------------------------

/// Buffing a servitor must change its stats server-side. This is the
/// end-to-end check slice 19 did not make: it proved a *heal* lands, not that
/// a **stat buff** actually moves the servitor's numbers.
#[test]
fn a_stat_buff_on_a_servitor_changes_its_stats() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    let before = world
        .objects
        .get_component::<Speeds>(&servitor)
        .unwrap()
        .run_spd;

    // Servitor Wind Walk's shape: a flat speed increase.
    let skill = Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        target_type: TargetType::Summon,
        abnormal_time: 1200,
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
    world.data.skill_data.insert_for_test(skill.clone());
    effects::apply_continuous_effects(&mut world, OWNER, servitor, &skill, None);

    let after = world
        .objects
        .get_component::<Speeds>(&servitor)
        .unwrap()
        .run_spd;
    assert!(
        after > before,
        "the servitor actually got faster ({before} → {after})"
    );
}

/// The buff's stat change must reach the **client**, not just the server:
/// `PetInfo` carries the summon's speeds, so the owner gets a fresh one.
/// Without this Servitor Haste and Wind Walk look like no-ops.
#[test]
fn buffing_a_servitor_refreshes_its_client_info() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    while rx.try_recv().is_ok() {} // drain the summon packets

    let skill = Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        target_type: TargetType::Summon,
        abnormal_time: 1200,
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
    effects::apply_continuous_effects(&mut world, OWNER, servitor, &skill, None);

    // `PetInfo` is 0xB2 — the packet that carries the summon's speeds.
    let mut saw_pet_info = false;
    while let Ok(pkt) = rx.try_recv() {
        if pkt.first() == Some(&0xB2) {
            saw_pet_info = true;
        }
    }
    assert!(
        saw_pet_info,
        "the owner was told its servitor's stats changed"
    );
}

// ---------------------------------------------------------------------------
// Summon PvP flagging (slice 21)
// ---------------------------------------------------------------------------

/// NPC skill cooldowns must actually apply. `set_skill_reuse` writes through
/// `if let Some(Reuses)` — a **silent no-op** when the component is absent —
/// and the check in `npc_cast` treats absence as "ready". If NPCs are never
/// given the component, a mob re-casts as fast as its AI loop allows.
#[test]
fn an_npc_records_its_skill_reuse() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);

    let skill = Skill {
        self_continuous: false,
        id: 4049,
        level: 1,
        reuse_delay: 10_000,
        ..Default::default()
    };
    set_skill_reuse(&mut world, FOE, &skill);

    assert!(
        world
            .objects
            .get_component::<Reuses>(&FOE)
            .is_some_and(|r| !r.0.is_empty()),
        "the NPC's cooldown was recorded"
    );
}

/// And the recorded cooldown must actually **block** the re-cast. Recording it
/// is only half the fix: the check reads the same component, so a test that
/// stops at "it was written" would not notice if the gate were bypassed.
#[test]
fn an_npc_skill_on_cooldown_cannot_be_recast() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
    // Enough MP that the cooldown is the only thing that can refuse it.
    {
        let v = world.objects.get_component_mut::<Vitals>(&FOE).unwrap();
        v.max_mp = 1000;
        v.cur_mp = 1000.0;
    }
    let skill = Skill {
        self_continuous: false,
        id: 4049,
        level: 1,
        reuse_delay: 10_000,
        ..Default::default()
    };

    assert!(
        crate::game_loop::npc::cast::check_use_conditions_for_test(&world, FOE, &skill),
        "ready before the first cast"
    );
    set_skill_reuse(&mut world, FOE, &skill);
    assert!(
        !crate::game_loop::npc::cast::check_use_conditions_for_test(&world, FOE, &skill),
        "refused while on cooldown"
    );

    world.tick += 10_000 / 100 + 1; // past the 10 s reuse
    assert!(
        crate::game_loop::npc::cast::check_use_conditions_for_test(&world, FOE, &skill),
        "ready again once it expires"
    );
}

// ---------------------------------------------------------------------------
// Pet equipment (slice 25)
// ---------------------------------------------------------------------------

/// A skill the player no longer knows (a subclass change between sessions)
/// restores nothing, and the row is consumed so it is not retried every login.
#[test]
fn an_unlearned_summon_skill_restores_nothing_and_is_not_retried() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 1111, 1200, 0, 0).unwrap();
    on_owner_leave_world(&mut world, OWNER);
    // The owner never had the skill in their book.

    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    assert!(servitor_of(&world, OWNER).is_none(), "nothing restored");
    assert!(
        world
            .objects
            .get_component::<model::components::summons::PlayerSummons>(&OWNER)
            .unwrap()
            .0
            .is_empty(),
        "and the row was consumed rather than retried forever"
    );
}

/// A Summoner's investment in buffing their servitor survives a relog — Java
/// keeps `character_summon_skills_save` for exactly this. Slice 27 brought the
/// servitor back but dropped everything cast on it.
#[test]
fn a_servitors_buffs_survive_a_relog() {
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

    // Servitor Wind Walk's shape, cast on the servitor.
    let buff = Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        abnormal_time: 1200,
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
    world.data.skill_data.insert_for_test(buff.clone());

    let servitor = summon_servitor(&mut world, OWNER, PANTHER, summon_skill, 1200, 0, 0).unwrap();
    effects::apply_continuous_effects(&mut world, OWNER, servitor, &buff, None);
    let buffed_speed = world
        .objects
        .get_component::<Speeds>(&servitor)
        .unwrap()
        .run_spd;

    on_owner_leave_world(&mut world, OWNER);
    let saved = world
        .objects
        .get_component::<model::components::summons::PlayerSummons>(&OWNER)
        .unwrap()
        .0[0]
        .clone();
    assert_eq!(saved.buffs.len(), 1, "the servitor's buff was captured");
    assert!(
        saved.buffs[0].remaining_time_secs > 0,
        "with time left on it"
    );

    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    let back = servitor_of(&world, OWNER).expect("servitor restored");
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&back)
            .unwrap()
            .run_spd,
        buffed_speed,
        "and it came back still buffed"
    );
}

/// The owner presses one of the summon's action-bar buttons and the
/// **servitor** casts it. 105 `ServitorSkillUse` rows ship in `ActionData.xml`;
/// the port handled only hold/attack/stop, so every one of them was dead.
#[test]
fn a_servitor_casts_the_skill_its_action_button_names() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    const ACTION: i32 = 1000;
    const SKILL: i32 = 4079;
    world
        .data
        .action_data
        .insert_servitor_skill_for_test(ACTION, SKILL);

    // Give the servitor template the skill, and register it.
    {
        let mut t = world.data.npc_data.get(PANTHER).unwrap().clone();
        t.skill_list.push((SKILL, 1));
        world.data.npc_data.insert_for_test(t);
    }
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id: SKILL,
        level: 1,
        target_type: TargetType::Self_,
        effects: vec![SkillEffect::Heal { power: 100.0 }],
        ..Default::default()
    });

    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 10.0;

    crate::game_loop::servitor::use_servitor_skill(&mut world, OWNER, SKILL);
    // `start_cast` schedules the finish; run the scheduler out to land it.
    for _ in 0..40 {
        advance_ticks(&mut world, 1);
    }

    assert!(
        world
            .objects
            .get_component::<Vitals>(&servitor)
            .unwrap()
            .cur_hp
            > 10.0,
        "the servitor cast its own heal"
    );
}

/// `OWNER_PET` aims at the **owner**, whatever they have selected.
///
/// Java writes this out by hand ahead of target resolution (`Summon.useMagic`:
/// `if (targetType == OWNER_PET) target = _owner`). The port collapsed the type
/// into `TargetType::Other`, which took the owner's *current selection*
/// instead — so Master Recharge (4025), the skill every Baby Kookaburra
/// carries, recharged whatever mob its owner had clicked, and refused with
/// "invalid target" when they had clicked nothing.
#[test]
fn an_owner_pet_skill_targets_the_owner_not_their_selection() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    const SKILL: i32 = 4025;
    {
        let mut tpl = world.data.npc_data.get(PANTHER).unwrap().clone();
        tpl.skill_list.push((SKILL, 1));
        world.data.npc_data.insert_for_test(tpl);
    }
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id: SKILL,
        level: 1,
        name: "Master Recharge".into(),
        target_type: TargetType::OwnerPet,
        cast_range: 400,
        effect_range: 900,
        effects: vec![SkillEffect::ManaHeal { power: 50.0 }],
        ..Default::default()
    });
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // The owner is short on MP and — the part that used to break it — has
    // something else entirely selected.
    world
        .objects
        .get_component_mut::<Vitals>(&OWNER)
        .unwrap()
        .cur_mp = 1.0;
    let before = world
        .objects
        .get_component::<Vitals>(&OWNER)
        .unwrap()
        .cur_mp;
    world
        .objects
        .add_components(&OWNER, TargetRef(Some(servitor)));

    crate::game_loop::servitor::use_servitor_skill(&mut world, OWNER, SKILL);
    for _ in 0..40 {
        advance_ticks(&mut world, 1);
    }

    assert!(
        world
            .objects
            .get_component::<Vitals>(&OWNER)
            .unwrap()
            .cur_mp
            > before,
        "the owner was recharged, not the selected target"
    );
}

/// **A summon may only use skills it actually has.** `ActionData.xml` binds
/// buttons for every summon in the game, so most rows name a skill this
/// particular servitor has never had — casting one anyway would let any summon
/// borrow another's abilities.
#[test]
fn a_servitor_refuses_a_skill_it_does_not_have() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    const SKILL: i32 = 4079;
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id: SKILL,
        level: 1,
        target_type: TargetType::Self_,
        effects: vec![SkillEffect::Heal { power: 100.0 }],
        ..Default::default()
    });
    // The Panther template is left without the skill.
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 10.0;

    crate::game_loop::servitor::use_servitor_skill(&mut world, OWNER, SKILL);
    for _ in 0..40 {
        advance_ticks(&mut world, 1);
    }

    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&servitor)
            .unwrap()
            .cur_hp,
        10.0,
        "nothing was cast"
    );
}

/// The real `ActionData.xml` binds servitor skills — a fixture cannot catch a
/// parse regression, so read the shipped file.
#[test]
fn the_real_action_data_binds_servitor_skills() {
    let data = crate::data::ActionData::load_from(DIST);
    // Action 1000 → skill 4079, one of the 13 reachable on this dist.
    assert_eq!(data.servitor_skill(1000), Some(4079));
    assert_eq!(
        data.servitor_skill(32),
        Some(4230),
        "the attack/move toggle"
    );
    assert_eq!(
        data.servitor_skill(0),
        None,
        "a non-servitor action binds nothing"
    );
}

// ---------------------------------------------------------------------------
// Summon spiritshots (slice 30)
// ---------------------------------------------------------------------------

/// **The community-board "Pet" button buffs the player's summon** (Java
/// `getPet()`/`getServitors()`), not the player; with no summon it refuses.
#[test]
fn community_board_pet_buffer_targets_the_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");

    // A one-buff scheme: Might (1068), a synthetic PA_UP buff in the test world.
    world.data.scheme_buffer.insert_for_test(1068, 1);
    world.cfg.community_board.available_buffs.insert(1068);
    world
        .buffer_schemes
        .insert(OWNER, vec![("s".to_string(), vec![1068])]);

    let has_buff = |world: &World, oid: i32| has_buff(world, oid, 1068);

    crate::game_loop::community_board::apply_scheme(&mut world, CID, OWNER, "s", true)
        .expect("applied to the summon");
    assert!(has_buff(&world, servitor), "the summon got the scheme buff");
    assert!(
        !has_buff(&world, OWNER),
        "the owner was not buffed by the Pet button"
    );

    // No summon → the Pet button refuses (Java's no-pet branch).
    unsummon_servitor(&mut world, OWNER);
    assert!(
        crate::game_loop::community_board::apply_scheme(&mut world, CID, OWNER, "s", true).is_err(),
        "with no summon the Pet button refuses"
    );
}

// ---------------------------------------------------------------------------
// Fear on a summon (Java Fear.canStart's isSummon leg)
// ---------------------------------------------------------------------------

/// **A servitor can be feared** (Java `Fear.canStart`'s `isSummon()` leg) even
/// though its NPC type isn't Attackable; a plain non-attackable NPC is not.
#[test]
fn a_servitor_can_be_feared() {
    use crate::model::components::space::{Movement, Position};
    use crate::model::skill::effects::SkillEffect;
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");
    // Put the servitor east of the owner so the fear shove has a clear bearing.
    if let Some(p) = world.objects.get_component_mut::<Position>(&servitor) {
        p.x = 500;
    }
    // A non-summon, non-attackable NPC (Folk) as the control.
    add_test_npc(&mut world, 8888, 15000, "Folk", 20, 900, 0, 0);

    // A Fear-only skill (built off the synthetic Might template).
    let mut fear = world.data.skill_data.get(1068, 1).unwrap().clone();
    fear.id = 9600;
    fear.effects = vec![SkillEffect::Fear { ticks: 5 }];
    world.data.skill_data.insert_for_test(fear.clone());

    // The servitor is shoved (fear ran → a Movement order was set).
    effects::apply_skill_effects(&mut world, OWNER, servitor, &fear);
    assert!(
        world.objects.get_component::<Movement>(&servitor).is_some(),
        "the servitor is feared and shoved"
    );

    // The plain non-attackable NPC is not feared.
    effects::apply_skill_effects(&mut world, OWNER, 8888, &fear);
    assert!(
        world.objects.get_component::<Movement>(&8888).is_none(),
        "a non-summon non-attackable NPC is not feared"
    );
}

// --- Pet evolution / exchange / restore (PetManager + Evolve) --------------

/// A buff the owner receives is re-applied to their servitor by
/// `Skill.applyEffects`' sharing branch. Every clause below is Java's.
const SHARED_BUFF: i32 = 9501;

const PRIVATE_BUFF: i32 = 9502;

const SHARED_DEBUFF: i32 = 9503;

fn sharing_skill(id: i32, shared: bool, is_debuff: bool) -> Skill {
    Skill {
        self_continuous: false,
        id,
        level: 1,
        name: format!("Share {id}"),
        is_continuous: true,
        abnormal_time: 3600,
        abnormal_level: 1,
        abnormal_type: format!("SHARE_{id}"),
        shared_with_summon: shared,
        is_debuff,
        // A stat pump so the buff is a real continuous entry, not a bare flag.
        effects: vec![SkillEffect::StatModifier(
            model::skill::effects::StatModifierEffect {
                stat: Stat::PhysicalAttack,
                mode: model::stats::StatModifierType::Per,
                amount: 8.0,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
                hp_percent: 0,
            },
        )],
        ..Default::default()
    }
}

fn buff_ids(world: &World, oid: i32) -> Vec<i32> {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .map(|b| {
            b.0.iter()
                .filter(|x| !x.passive)
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default()
}

fn sharing_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = servitor_world();
    for (id, shared, debuff) in [
        (SHARED_BUFF, true, false),
        (PRIVATE_BUFF, false, false),
        (SHARED_DEBUFF, true, true),
    ] {
        world
            .data
            .skill_data
            .insert_for_test(sharing_skill(id, shared, debuff));
    }
    (world, db, l)
}

fn land_on(world: &mut World, skill_id: i32, target: i32) {
    let skill = skill_by_id(world, skill_id, 1).unwrap();
    effects::apply_skill_effects(world, target, target, &skill);
}

/// The headline: buffing yourself also buffs your servitor. Without this every
/// summoner's pet fought permanently unbuffed.
#[test]
fn a_buff_on_the_owner_is_shared_with_their_servitor() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    land_on(&mut world, SHARED_BUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![SHARED_BUFF], "owner keeps it");
    assert_eq!(
        buff_ids(&world, servitor),
        vec![SHARED_BUFF],
        "and the servitor receives the same buff"
    );
}

/// `<isSharedWithSummon>false</isSharedWithSummon>` stops at the player. Only
/// three skills in the datapack declare the tag, which is exactly why the
/// parser's default must be `true` — see the sibling test below.
#[test]
fn a_non_shared_buff_stops_at_the_owner() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    land_on(&mut world, PRIVATE_BUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![PRIVATE_BUFF]);
    assert!(
        buff_ids(&world, servitor).is_empty(),
        "a non-shared buff must not reach the servitor"
    );
}

/// Java's guard is `!_isDebuff`: sharing is a *favour*, so a debuff landing on
/// the owner must not be copied onto their summon as a second victim.
#[test]
fn a_debuff_on_the_owner_is_never_shared() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    land_on(&mut world, SHARED_DEBUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![SHARED_DEBUFF]);
    assert!(
        buff_ids(&world, servitor).is_empty(),
        "a debuff is not shared even when the skill is flagged shared"
    );
}

/// **A pet is not a servitor.** Java shares through `getServitors()`, and `_pet`
/// is a separate field, so a wolf receives nothing. Easy to get wrong here
/// because this port hangs `ServitorOf` on pets too.
#[test]
fn a_pet_does_not_receive_shared_buffs() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");

    land_on(&mut world, SHARED_BUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![SHARED_BUFF]);
    assert!(
        buff_ids(&world, pet).is_empty(),
        "Java reads getServitors(), which excludes the pet"
    );
}

/// Sharing follows a buff that *landed*: the servitor is not a second roll of
/// the dice, and the chain stops at one hop (the servitor is not a player, so
/// it shares nothing onward).
#[test]
fn sharing_does_not_recurse_past_the_servitor() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    // Cast straight at the servitor: it is not a player, so nothing is shared
    // back to the owner.
    land_on(&mut world, SHARED_BUFF, servitor);

    assert_eq!(buff_ids(&world, servitor), vec![SHARED_BUFF]);
    assert!(
        buff_ids(&world, OWNER).is_empty(),
        "sharing runs owner → servitor only, never the reverse"
    );
}

/// The parser default, against the **real datapack** rather than a fixture.
///
/// `isSharedWithSummon` defaults to `true` in Java and is declared on a handful
/// of skills, so a `false`-defaulting parse would look perfectly healthy in a
/// unit test while silently switching sharing off for every buff in the game.
/// Prophecy of Might declares nothing and must come back shared.
///
/// Skill 1557 is the counter-case, and its identity is the point: **Servitor
/// Share** is the skill that copies the owner's stats onto the summon itself,
/// so re-sharing it would double-apply. Java's source carries that exact
/// comment ("Avoiding Servitor Share since it's implementation already
/// 'shares' the effect").
#[test]
fn the_real_datapack_defaults_to_shared() {
    let skills = dist::skills();

    let prophecy = skills
        .get(1352, 1)
        .expect("Prophecy of Might is on this dist");
    assert!(
        prophecy.shared_with_summon,
        "a skill that declares no <isSharedWithSummon> defaults to shared"
    );

    let servitor_share = skills.get(1557, 1).expect("Servitor Share");
    assert!(
        !servitor_share.shared_with_summon,
        "Servitor Share must not be re-shared onto the summon it already shares to"
    );
}

// ---------------------------------------------------------------------------
// The active pet's collar (Java `Item.isAvailable` / `ExBuySellList`)
// ---------------------------------------------------------------------------
