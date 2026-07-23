//! Over-hit (G20): bonus XP for a killing blow that overshoots.

use super::*;

use crate::model::components::Overhit;
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const CASTER: i32 = 2001;
const CID: u32 = 1;
const OVERHIT_SKILL: i32 = 8300;
const PLAIN_SKILL: i32 = 8301;
/// A deliberately feeble over-hit nuke, for the cases that need the target to
/// *survive* — the 5000-power ones above hit for six figures.
const WEAK_OVERHIT_SKILL: i32 = 8302;

fn nuke(id: i32, power: f64, over_hit: bool) -> Skill {
    Skill {
        without_action: false,
        item_consume_id: 0,
        item_consume_count: 0,
        id,
        level: 1,
        name: format!("Nuke {id}"),
        operate_type: OperateType::Active,
        is_continuous: false,
        target_type: TargetType::Enemy,
        magic_type: 1,
        magic_level: 40,
        effect_point: -100,
        cast_range: 900,
        effect_range: 1000,
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
        over_hit,
        abnormal_visuals: Vec::new(),
        toggle_group_id: 0,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        affect_range: 0,
        affect_limit: (0, 0),
        can_be_dispelled: true,
        is_debuff: false,
        stay_after_death: false,
        effects: vec![SkillEffect::MagicalAttack { power }],
        ..Default::default()
    }
}

fn overhit_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cast_test_world();
    world.data.experience = crate::data::ExperienceData::from_table(
        vec![
            0, 0, 1000, 5000, 20_000, 100_000, 500_000, 2_000_000, 10_000_000,
        ],
        8,
    );
    world
        .data
        .skill_data
        .insert_for_test(nuke(OVERHIT_SKILL, 5000.0, true));
    world
        .data
        .skill_data
        .insert_for_test(nuke(PLAIN_SKILL, 5000.0, false));
    world
        .data
        .skill_data
        .insert_for_test(nuke(WEAK_OVERHIT_SKILL, 1.0, true));
    (world, db, l)
}

/// Register a mob with a real exp reward and put it on low HP so the next blow
/// overshoots massively.
fn wounded_mob(world: &mut World, remaining_hp: f64) -> i32 {
    let mut t = crate::data::npc_data::default_template(20500);
    t.type_name = "Monster".into();
    t.level = 5;
    t.base_hp_max = 1000.0;
    t.base_mp_max = 100.0;
    t.exp = 10_000.0;
    t.sp = 100.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(world, NPC_OID, 20500, "Monster", 5, 60, 0, 0);
    {
        // Order matters: raise the pool before the current value, or a
        // "healthy" fixture ends up with cur_hp above max_hp and dies anyway.
        let v = world.objects.get_component_mut::<Vitals>(&NPC_OID).unwrap();
        v.max_hp = v.max_hp.max(remaining_hp.ceil() as i32);
        v.cur_hp = remaining_hp;
    }
    // The killer needs a damage share to be rewarded at all.
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&NPC_OID)
        .unwrap()
        .0
        .insert(
            CASTER,
            crate::model::npc::AggroInfo {
                hate: 100.0,
                damage: 100.0,
            },
        );
    NPC_OID
}

fn cast(world: &mut World, skill_id: i32, target: i32) {
    let skill = world
        .data
        .skill_data
        .get(skill_id, 1)
        .cloned()
        .expect("registered");
    // Pin the two rolls a `MagicalAttack` consumes: the magic crit (999 → no
    // crit, so the exp comparisons aren't skewed by a random doubling) and the
    // `MagicFailures` success roll (0 → lands). A resisted cast floors the
    // damage to 1, which would leave the wounded mob alive and trip the
    // must-kill assertion in `kill_for_exp`.
    world.forced_rolls.extend([999, 0]);
    crate::game_loop::skills::effects::apply_skill_effects(world, CASTER, target, &skill);
}

// ---------------------------------------------------------------------------
//
// Note on shape: the over-hit record is **transient by design**. A lethal blow
// runs `apply_skill_damage` → … → `npc_do_die` → the reward path, which spends
// the record and clears it, all inside the `cast` below. So these assert the
// observable outcome (exp paid, notice sent) rather than the intermediate
// component, which is already gone by the time a test could look at it.

/// Exp gained by killing the wounded mob with `skill_id`, plus the packets the
/// killer received.
fn kill_for_exp(skill_id: i32, remaining_hp: f64) -> (i64, Vec<Vec<u8>>) {
    let (mut world, _db, _l) = overhit_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mob = wounded_mob(&mut world, remaining_hp);
    let before = world.objects.get_component::<Player>(&CASTER).unwrap().exp;
    drain(&mut out);

    cast(&mut world, skill_id, mob);
    assert!(
        world.objects.get_component::<Vitals>(&mob).unwrap().dead,
        "the fixture blow must kill for over-hit to apply"
    );
    let gained = world.objects.get_component::<Player>(&CASTER).unwrap().exp - before;
    (gained, drain(&mut out))
}

/// **The feature:** a killing `<overHit>` blow pays more exp than the same kill
/// without it, and announces "Over-hit!".
#[test]
fn overhit_kill_pays_bonus_exp_and_announces() {
    let (plain, _) = kill_for_exp(PLAIN_SKILL, 5.0);
    let (with_overhit, pkts) = kill_for_exp(OVERHIT_SKILL, 5.0);

    assert!(plain > 0, "baseline kill paid exp: {plain}");
    assert!(
        with_overhit > plain,
        "over-hit paid more ({with_overhit} vs {plain})"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::OVER_HIT),
        "the killer is told it was an over-hit"
    );
}

/// The bonus is capped at **25 %** of the exp share, however far the blow
/// overshoots — a 1-HP mob hit for thousands still pays at most a quarter more.
#[test]
fn overhit_bonus_is_capped_at_25_percent() {
    let (plain, _) = kill_for_exp(PLAIN_SKILL, 1.0);
    let (with_overhit, _) = kill_for_exp(OVERHIT_SKILL, 1.0);

    assert!(with_overhit > plain, "the bonus was paid");
    assert!(
        with_overhit as f64 <= plain as f64 * 1.25 + 1.0,
        "capped at +25% ({with_overhit} vs base {plain})"
    );
}

/// A blow that **doesn't kill** is not an over-hit at all — no bonus is banked,
/// so a later kill pays only the base (Java's negative-excess branch).
#[test]
fn non_killing_blow_banks_nothing() {
    let (mut world, _db, _l) = overhit_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mob = wounded_mob(&mut world, 100_000.0);

    cast(&mut world, WEAK_OVERHIT_SKILL, mob);
    assert!(
        !world.objects.get_component::<Vitals>(&mob).unwrap().dead,
        "fixture survives the blow"
    );
    assert!(
        world.objects.get_component::<Overhit>(&mob).is_none(),
        "a blow that didn't kill banks no over-hit"
    );
}

/// A skill **without** `<overHit>` banks nothing even on a lethal overshoot —
/// which is what makes the exp comparison above meaningful.
#[test]
fn plain_skill_banks_nothing() {
    let (mut world, _db, _l) = overhit_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mob = wounded_mob(&mut world, 100_000.0);

    cast(&mut world, PLAIN_SKILL, mob);

    assert!(world.objects.get_component::<Overhit>(&mob).is_none());
}
