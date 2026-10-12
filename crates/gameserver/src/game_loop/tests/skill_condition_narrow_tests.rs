//! The off-chronicle tail of `<condition>` names, and conditions on a summon's
//! owner-ordered cast. The handlers are small, so each is checked directly through
//! `conditions::check_for_test`; the summon half goes through `npc::cast`.

use super::*;
use crate::data::zone_data::ZoneKind;
use crate::game_loop::npc::cast::cast_checked;
use crate::game_loop::skills::conditions::check_for_test;
use crate::model::components::skills::{Buffs, SkillBook};
use crate::model::siege::{Siege, SiegeClanType};
use crate::model::skill::condition::SkillCondition as C;
use crate::model::skill::{AffectType, Skill};

const CID: u32 = 1;
const CASTER: i32 = 2001;
const OTHER: i32 = 2002;
const MOB: i32 = NPC_OID + 30;

/// A caster, a second player 100 units off, and a monster 300 units off.
fn world() -> World {
    let (mut world, _db, _l) = cast_test_world();
    let _ = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _ = ingame_caster(&mut world, 2, OTHER, 100, 0);
    add_test_npc(&mut world, MOB, 20001, "Monster", 5, 300, 0, 0);
    world
}

fn player_mut(world: &mut World, oid: i32) -> &mut Player {
    world.objects.get_component_mut::<Player>(&oid).unwrap()
}

fn passes(world: &World, target: i32, cond: C) -> bool {
    check_for_test(world, CASTER, target, &[cond])
}

/// The real skill data now builds these instead of dropping them.
#[test]
fn dist_carriers_build_their_conditions() {
    let skills = dist::skills();
    let conds = |id: i32| skills.get(id, 1).expect("skill").conditions.clone();

    assert!(conds(2003).contains(&C::CasterIsPlayer), "Squash Seed");
    assert!(conds(5110).contains(&C::InsideSiegeZone), "Cannon Fodder");
    assert!(
        conds(6642)
            .iter()
            .any(|c| matches!(c, C::CheckCastRange { .. })),
        "Dash"
    );
    assert!(
        conds(5413).contains(&C::Constant { passes: false }),
        "an agathion trick, with no agathions"
    );
    assert!(
        conds(2444).iter().any(|c| matches!(
            c,
            C::CheckClass {
                class_id: Some(_),
                affect: AffectType::Caster,
                is_within: false
            }
        )),
        "Fighter's Will — not for the mage classes"
    );
}

#[test]
fn constants_and_caster_kind() {
    let world = world();
    assert!(passes(&world, 0, C::Constant { passes: true }));
    assert!(!passes(&world, 0, C::Constant { passes: false }));
    assert!(passes(&world, 0, C::CasterIsPlayer));
    assert!(
        !check_for_test(&world, MOB, 0, &[C::CasterIsPlayer]),
        "Java's `caster.isPlayer() &&` leg"
    );
}

/// `OpCheckAbnormal`: type and minimum level, wanted or unwanted, on either
/// end of the cast.
#[test]
fn check_abnormal_reads_type_and_level() {
    let mut world = world();
    let mut buff = test_buff();
    buff.abnormal_type = "VP_UP".into();
    buff.abnormal_level = 5;
    world.objects.add_components(&OTHER, Buffs(vec![buff]));

    let cond = |level: i32, has_abnormal: bool, affect: AffectType| C::CheckAbnormal {
        abnormal_type: "VP_UP".into(),
        level,
        has_abnormal,
        affect,
    };
    assert!(passes(&world, OTHER, cond(5, true, AffectType::Target)));
    assert!(!passes(&world, OTHER, cond(6, true, AffectType::Target)));
    assert!(passes(&world, OTHER, cond(6, false, AffectType::Target)));
    assert!(!passes(&world, OTHER, cond(5, true, AffectType::Caster)));
    assert!(passes(&world, OTHER, cond(5, false, AffectType::Caster)));
    assert!(
        !passes(&world, 0, cond(5, false, AffectType::Target)),
        "no target, no answer"
    );
    assert!(!passes(&world, OTHER, cond(5, true, AffectType::Both)));
}

#[test]
fn class_and_skill_checks() {
    let mut world = world();
    player_mut(&mut world, CASTER).class_id = 52;
    player_mut(&mut world, OTHER).class_id = 10;

    let class = |id: i32, is_within: bool| C::CheckClass {
        class_id: Some(id),
        affect: AffectType::Caster,
        is_within,
    };
    assert!(passes(&world, 0, class(52, true)));
    assert!(!passes(&world, 0, class(10, true)));
    assert!(passes(&world, 0, class(10, false)));
    assert!(!passes(
        &world,
        0,
        C::CheckClass {
            class_id: None,
            affect: AffectType::Caster,
            is_within: true
        }
    ));

    let list = |affect: AffectType, is_within: bool| C::CheckClassList {
        class_ids: vec![10, 11],
        affect,
        is_within,
    };
    assert!(!passes(&world, OTHER, list(AffectType::Caster, true)));
    assert!(passes(&world, OTHER, list(AffectType::Caster, false)));
    assert!(passes(&world, OTHER, list(AffectType::Target, true)));
    assert!(
        !passes(&world, MOB, list(AffectType::Target, false)),
        "not a player"
    );

    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(9100, 1);
    let knows = |ids: Vec<i32>| C::CheckSkillList {
        skill_ids: ids,
        affect: AffectType::Caster,
    };
    assert!(passes(&world, 0, knows(vec![1, 9100])));
    assert!(!passes(&world, 0, knows(vec![1, 2])));
}

#[test]
fn player_state_checks() {
    let mut world = world();
    assert!(passes(&world, 0, C::NotCursed));
    assert!(passes(&world, 0, C::NotOlympiad));
    assert!(!passes(&world, 0, C::Subjob));
    assert!(!passes(
        &world,
        0,
        C::PkCount {
            affect: AffectType::Caster
        }
    ));
    assert!(!passes(&world, 0, C::HaveSummon));
    assert!(passes(&world, 0, C::VitalityRoom { amount: 1000 }));
    assert!(!passes(
        &world,
        0,
        C::VitalityRoom {
            amount: crate::model::MAX_VITALITY_POINTS + 1
        }
    ));
    assert!(passes(&world, 0, C::CanSummonMulti { summon_points: 0 }));
    assert!(
        !passes(&world, 0, C::CanSummonMulti { summon_points: 1 }),
        "there are no summon points to spend"
    );

    {
        let p = player_mut(&mut world, CASTER);
        p.cursed_weapon_equipped_id = 8190;
        p.class_index = 1;
        p.pk_kills = 3;
    }
    assert!(!passes(&world, 0, C::NotCursed));
    assert!(passes(&world, 0, C::Subjob));
    assert!(passes(
        &world,
        0,
        C::PkCount {
            affect: AffectType::Caster
        }
    ));
    assert!(!passes(
        &world,
        OTHER,
        C::PkCount {
            affect: AffectType::Target
        }
    ));
}

#[test]
fn target_and_place_checks() {
    let mut world = world();

    // Clan.
    assert!(!passes(&world, OTHER, C::TargetMyPledge), "no clan at all");
    player_mut(&mut world, CASTER).clan_id = 88;
    player_mut(&mut world, OTHER).clan_id = 88;
    assert!(passes(&world, OTHER, C::TargetMyPledge));
    assert!(!passes(&world, MOB, C::TargetMyPledge));

    // Private store: only a player with one open is refused.
    assert!(passes(&world, OTHER, C::TargetNoPrivateStore));
    assert!(passes(&world, MOB, C::TargetNoPrivateStore));
    assert!(passes(&world, 0, C::TargetNoPrivateStore));
    player_mut(&mut world, OTHER).store_type = 1;
    assert!(!passes(&world, OTHER, C::TargetNoPrivateStore));

    // Minimum distance: 100 away is not 200 away.
    assert!(passes(&world, OTHER, C::CheckCastRange { distance: 100 }));
    assert!(!passes(&world, OTHER, C::CheckCastRange { distance: 200 }));
    assert!(passes(&world, MOB, C::CheckCastRange { distance: 200 }));
    assert!(!passes(&world, 0, C::CheckCastRange { distance: 0 }));

    // Peace zone, both polarities.
    assert!(!passes(&world, 0, C::PeaceZone { inside: true }));
    assert!(passes(&world, 0, C::PeaceZone { inside: false }));
    insert_zone(&mut world, ZoneKind::Peace, -50, 50, -50, 50);
    assert!(passes(&world, 0, C::PeaceZone { inside: true }));
    assert!(!passes(&world, 0, C::PeaceZone { inside: false }));

    // Crt effect: an NPC target whose template allows it.
    assert!(passes(&world, MOB, C::CheckCrtEffect));
    assert!(!passes(&world, OTHER, C::CheckCrtEffect));
    let mut t = world.data.npc_data.get(20001).cloned().unwrap();
    t.ex_crt_effect = false;
    world.data.npc_data.insert_for_test(t);
    assert!(!passes(&world, MOB, C::CheckCrtEffect));
}

/// `OpInSiege` and the siege-zone trio, for a player and for an NPC — the
/// latter has no cached zone flags and is placed by position.
#[test]
fn siege_checks_follow_the_running_siege() {
    let mut world = world();
    insert_siege_zone(&mut world, 3, -1000, 1000, -1000, 1000);
    world.castles = vec![castle_row(3, "Giran")];
    let mut siege = Siege::new(3);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);

    let in_siege = C::InSiege {
        residence_ids: vec![3],
    };
    assert!(!passes(&world, 0, in_siege.clone()), "not started");
    assert!(!check_for_test(&world, MOB, 0, &[C::InsideSiegeZone]));

    world.sieges.get_mut(&3).unwrap().in_progress = true;
    assert!(passes(&world, 0, in_siege));
    assert!(!passes(
        &world,
        0,
        C::InSiege {
            residence_ids: vec![5]
        }
    ));
    assert!(
        check_for_test(&world, MOB, 0, &[C::InsideSiegeZone]),
        "a siege machine standing in the live zone"
    );
}

/// Java evaluates `<conditions>` in `Player.useMagic` and `Summon.useMagic`
/// only. A summon's owner-ordered rush wants its run-up; the same skill from a
/// monster's AI or a script (`doCast`) is not conditioned at all.
#[test]
fn only_an_owner_ordered_cast_obeys_skill_conditions() {
    use crate::game_loop::npc::cast::cast_ordered_by_owner;

    let mut world = world();
    let rush = Skill {
        id: 9300,
        level: 1,
        conditions: vec![C::CheckCastRange { distance: 200 }],
        ..Default::default()
    };
    let near = MOB + 1;
    add_test_npc(&mut world, near, 20001, "Monster", 5, 50, 0, 0);
    let also_near = MOB + 2;
    add_test_npc(&mut world, also_near, 20001, "Monster", 5, 50, 50, 0);

    assert!(
        cast_ordered_by_owner(&mut world, MOB, CASTER, &rush),
        "300 units is far enough"
    );
    assert!(
        !cast_ordered_by_owner(&mut world, near, CASTER, &rush),
        "50 units is not"
    );
    assert!(
        cast_checked(&mut world, also_near, CASTER, &rush),
        "a script's or the AI's cast has no such gate"
    );
}
