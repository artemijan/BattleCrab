//! Baium — archangels and the strider debuff.

use super::*;

use crate::game_loop::baium::{ARCHANGEL, BAIUM};

const BAIUM_OID: i32 = NPC_OID + 110;
const PLAYER: i32 = 9995;
const CID: u32 = 1;
const ANTI_STRIDER: i32 = 4258;
const MOUNT_STRIDER: u8 = 1;

fn baium_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [(BAIUM, "GrandBoss"), (ARCHANGEL, "Monster")] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 75;
        t.base_hp_max = 100_000.0;
        t.base_mp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    world.data.skill_data.insert_for_test(crate::model::skill::Skill {
        id: ANTI_STRIDER,
        level: 1,
        abnormal_time: 60,
        effects: vec![crate::model::skill::SkillEffect::StatModifier(crate::model::skill::StatModifierEffect {
            stat: crate::model::stats::Stat::RunSpeed,
            mode: crate::model::stats::StatModifierType::Diff,
            amount: -50.0,
            ..Default::default()
        })],
        ..Default::default()
    });
    (world, db, l)
}

fn count(world: &mut World, npc_id: i32) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
        if x.npc_id == npc_id {
            n += 1;
        }
    });
    n
}

fn has_debuff(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == ANTI_STRIDER))
}

/// Baium brings out five archangels. They are **not** in a minion table, so
/// nothing but the script would place them.
#[test]
fn baium_spawns_five_archangels() {
    let (mut world, _db, _l) = baium_world();
    crate::game_loop::baium::on_baium_spawned(&mut world);
    assert_eq!(count(&mut world, ARCHANGEL), 5);
}

/// A strider-mounted attacker is hindered.
#[test]
fn a_strider_rider_is_hindered() {
    let (mut world, _db, _l) = baium_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&BAIUM_OID).unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world.objects.get_component_mut::<crate::model::Player>(&PLAYER).unwrap().mount_type = MOUNT_STRIDER;

    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert!(has_debuff(&world, PLAYER), "the rider was hindered");
}

/// An unmounted attacker is left alone — the debuff is aimed at striders
/// specifically, not at everyone.
#[test]
fn an_unmounted_attacker_is_not_hindered() {
    let (mut world, _db, _l) = baium_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);

    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert!(!has_debuff(&world, PLAYER), "on foot, no debuff");
}

/// The debuff is cast **once**, not on every swing — Java guards on
/// `!isAffectedBySkill(4258)`.
#[test]
fn the_strider_debuff_is_not_recast_while_it_holds() {
    let (mut world, _db, _l) = baium_world();
    let mut rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&BAIUM_OID).unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world.objects.get_component_mut::<crate::model::Player>(&PLAYER).unwrap().mount_type = MOUNT_STRIDER;

    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }
    assert!(has_debuff(&world, PLAYER));
    while rx.try_recv().is_ok() {}

    // A second hit while it still holds must start no new cast.
    crate::game_loop::baium::on_baium_attacked(&mut world, BAIUM_OID, PLAYER);
    let mut casts = 0;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&0x48) {
            // MagicSkillUse
            casts += 1;
        }
    }
    assert_eq!(casts, 0, "already hindered, nothing recast");
}

// ---------------------------------------------------------------------------
// The threat table (slice 12)
// ---------------------------------------------------------------------------

use crate::game_loop::boss_threat::BossThreat;

fn threat(world: &World, oid: i32) -> [(i32, i32); 3] {
    world.objects.get_component::<BossThreat>(&oid).map(|t| t.slots).unwrap_or_default()
}

fn wound_baium_to(world: &mut World, fraction: f64) {
    let v = world.objects.get_component_mut::<Vitals>(&BAIUM_OID).unwrap();
    v.cur_hp = v.max_hp as f64 * fraction;
}

/// These two read the table straight after weighting, via `on_boss_damage`
/// rather than the `onAttack` hook — because the hook goes on to *choose* a
/// target, and choosing knocks the top threat down to 500 seven times out of
/// ten. That is Java's order (`refreshAiParams` then `manageSkills`), so the
/// weighting has to be observed where it is written, not after the boss has
/// acted on it.
///
/// **Melee threat is worth fifty times a caster's at full health** — `×1000`
/// against `(damage/3) × 20`. That asymmetry is the fight, so it is asserted
/// as a ratio rather than two independent numbers.
#[test]
fn melee_threat_dwarfs_caster_threat_at_full_health() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    let melee = PLAYER;
    let caster = PLAYER + 1;
    // No jitter, so the ladder alone decides.
    world.forced_rolls.push_back(0);
    crate::game_loop::boss_threat::on_boss_damage(&mut world, BAIUM_OID, melee, 300, true);
    world.forced_rolls.push_back(0);
    crate::game_loop::boss_threat::on_boss_damage(&mut world, BAIUM_OID, caster, 300, false);

    let t = threat(&world, BAIUM_OID);
    let melee_v = t.iter().find(|(id, _)| *id == melee).unwrap().1;
    let caster_v = t.iter().find(|(id, _)| *id == caster).unwrap().1;
    assert_eq!(melee_v, 300 * 1000);
    assert_eq!(caster_v, (300 / 3) * 20);
    assert_eq!(melee_v / caster_v, 150, "melee is worth 150x this caster hit");
}

/// **The caster weighting climbs as Baium weakens** — a caster who is beneath
/// notice at full health becomes a real threat below 25%.
#[test]
fn caster_threat_climbs_as_baium_weakens() {
    let weighted_at = |fraction: f64| {
        let (mut world, _db, _l) = baium_world();
        add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
        wound_baium_to(&mut world, fraction);
        world.forced_rolls.push_back(0);
        crate::game_loop::boss_threat::on_boss_damage(&mut world, BAIUM_OID, PLAYER, 300, false);
        threat(&world, BAIUM_OID).iter().find(|(id, _)| *id == PLAYER).unwrap().1
    };

    let full = weighted_at(1.0); // (300/3)*20 = 2000
    let three_quarters = weighted_at(0.6); // *10 = 3000
    let half = weighted_at(0.4); // *20 = 6000
    let quarter = weighted_at(0.1); // (300/3)*100 = 10000

    assert_eq!((full, three_quarters, half, quarter), (2000, 3000, 6000, 10000));
    assert!(quarter > full * 4, "a caster matters five times more once Baium is nearly dead");
}

/// The table holds **three** attackers, and a fourth displaces the weakest —
/// not the oldest, and not nobody.
#[test]
fn a_fourth_attacker_displaces_the_weakest() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    for (oid, dmg) in [(101, 500), (102, 100), (103, 400)] {
        world.forced_rolls.push_back(0);
        crate::game_loop::boss_threat::refresh_threat(&mut world, BAIUM_OID, oid, dmg, dmg);
    }
    // 102 is the weakest at 100.
    world.forced_rolls.push_back(0);
    crate::game_loop::boss_threat::refresh_threat(&mut world, BAIUM_OID, 104, 300, 300);

    let ids: Vec<i32> = threat(&world, BAIUM_OID).iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&104), "the newcomer got on the table");
    assert!(!ids.contains(&102), "by displacing the weakest, not the oldest");
    assert!(ids.contains(&101) && ids.contains(&103), "the stronger two stayed");
}

/// An attacker already on the table is **only raised when it is below the
/// floor** — repeated small hits don't ratchet a threat upward forever.
#[test]
fn an_existing_entry_is_not_ratcheted_by_small_hits() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    world.forced_rolls.push_back(0);
    crate::game_loop::boss_threat::refresh_threat(&mut world, BAIUM_OID, PLAYER, 10_000, 10_000);
    let after_big = threat(&world, BAIUM_OID)[0].1;

    // A small follow-up: its floor (50 + 1000) is far below the stored value,
    // so nothing changes.
    world.forced_rolls.push_back(0);
    crate::game_loop::boss_threat::refresh_threat(&mut world, BAIUM_OID, PLAYER, 50, 50);
    assert_eq!(threat(&world, BAIUM_OID)[0].1, after_big, "a small hit does not move a large threat");
}

// ---------------------------------------------------------------------------
// Skill selection (slice 13)
// ---------------------------------------------------------------------------

const BAIUM_ATTACK: i32 = 4127;
const ENERGY_WAVE: i32 = 4128;
const EARTH_QUAKE: i32 = 4129;
const THUNDERBOLT: i32 = 4130;
const GROUP_HOLD: i32 = 4131;

/// Put an attacker on the table next to Baium so it survives pruning.
fn seed_threat(world: &mut World, oid: i32, value: i32) {
    add_test_npc(world, oid, ARCHANGEL, "Monster", 75, 20, 0, 0);
    world.forced_rolls.push_back(0);
    crate::game_loop::boss_threat::refresh_threat(world, BAIUM_OID, oid, value, value);
}

/// Baium acts on the **highest** threat.
#[test]
fn baium_targets_the_highest_threat() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    seed_threat(&mut world, 201, 100);
    seed_threat(&mut world, 202, 9_000);
    seed_threat(&mut world, 203, 500);

    world.forced_rolls.push_back(99); // skip the decay
    world.forced_rolls.push_back(99); // and the skill rolls
    world.forced_rolls.push_back(99);
    let (target, _) = crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID).expect("a target");
    assert_eq!(target, 202, "the biggest threat");
}

/// **The rotation**: 70% of the time the chosen threat is knocked down to 500,
/// so Baium doesn't tunnel one player for the whole fight.
#[test]
fn the_top_threat_is_knocked_down_so_others_get_a_turn() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    seed_threat(&mut world, 201, 9_000);
    seed_threat(&mut world, 202, 4_000);

    world.forced_rolls.push_back(10); // < 70: the decay fires
    for _ in 0..4 {
        world.forced_rolls.push_back(99);
    }
    crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID);

    let slots = threat(&world, BAIUM_OID);
    let top = slots.iter().find(|(id, _)| *id == 201).unwrap().1;
    assert_eq!(top, 500, "knocked down");
    let other = slots.iter().find(|(id, _)| *id == 202).unwrap().1;
    assert!(other > top, "so the next player is now the biggest threat");
}

/// A dead or fled attacker stops holding a slot.
#[test]
fn dead_and_distant_attackers_are_pruned() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    seed_threat(&mut world, 201, 9_000);
    seed_threat(&mut world, 202, 4_000);
    world.objects.get_component_mut::<Vitals>(&201).unwrap().dead = true;
    world.objects.get_component_mut::<Position>(&202).unwrap().x = 999_999;
    seed_threat(&mut world, 203, 100);

    world.forced_rolls.push_back(99);
    for _ in 0..4 {
        world.forced_rolls.push_back(99);
    }
    let (target, _) = crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID).expect("a target");
    assert_eq!(target, 203, "the only live, nearby attacker — despite the lowest raw threat");
}

/// **The skill pool widens as Baium weakens.** Above 75% he has two options
/// beyond his basic attack; below 25% he has four. Each roll is forced to miss
/// so the fallback is reached, then forced to hit so the *first* option of each
/// band is revealed.
#[test]
fn the_skill_pool_widens_as_baium_weakens() {
    let first_option_at = |fraction: f64| {
        let (mut world, _db, _l) = baium_world();
        add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
        seed_threat(&mut world, 201, 9_000);
        {
            let v = world.objects.get_component_mut::<Vitals>(&BAIUM_OID).unwrap();
            v.cur_hp = v.max_hp as f64 * fraction;
        }
        world.forced_rolls.push_back(99); // skip the decay
        world.forced_rolls.push_back(5); // the first skill roll hits
        crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID).unwrap().1
    };

    assert_eq!(first_option_at(1.0), ENERGY_WAVE, "above 75%: Energy Wave leads");
    assert_eq!(first_option_at(0.6), GROUP_HOLD, "below 75%: Group Hold joins and leads");
    assert_eq!(first_option_at(0.4), THUNDERBOLT, "below 50%: Thunderbolt joins and leads");
    assert_eq!(first_option_at(0.1), THUNDERBOLT, "below 25%: the full repertoire");
}

/// Every roll missing falls back to the basic attack — the common case.
#[test]
fn all_rolls_missing_falls_back_to_the_basic_attack() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    seed_threat(&mut world, 201, 9_000);

    world.forced_rolls.push_back(99); // no decay
    for _ in 0..4 {
        world.forced_rolls.push_back(99); // every skill roll misses
    }
    let (_, skill) = crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID).unwrap();
    assert_eq!(skill, BAIUM_ATTACK);
    let _ = (EARTH_QUAKE,);
}

/// An empty table means nothing to act on.
#[test]
fn an_empty_threat_table_yields_no_action() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    assert!(crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID).is_none());
}

/// **Baium casts too.** His `manage_skills` had existed since the threat slice
/// with no caller anywhere in the crate — he chose skills into the void and
/// only ever swung. The chooser being correct and tested is exactly what made
/// it invisible: nothing about it looked unfinished.
///
/// This asserts through the real damage hook, so it fails against the previous
/// commit for both bosses.
#[test]
fn a_hit_makes_baium_cast() {
    let (mut world, _db, _l) = baium_world();
    let mut rx = ingame_caster(&mut world, CID, PLAYER, 20, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&BAIUM_OID).unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    // BAIUM_ATTACK, the fallback every band ends on.
    world.data.skill_data.insert_for_test(crate::model::skill::Skill { id: 4127, level: 1, ..Default::default() });
    while rx.try_recv().is_ok() {}

    // Jitter 0, no decay, then every ladder roll missing -> the basic attack.
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(99);
    for _ in 0..6 {
        world.forced_rolls.push_back(99);
    }
    crate::game_loop::baium::on_baium_damage(&mut world, BAIUM_OID, PLAYER, 500, true);

    let casts = std::iter::from_fn(|| rx.try_recv().ok()).filter(|p| p.first() == Some(&0x48)).count();
    assert_eq!(casts, 1, "the damage hook chose a skill and cast it");
}
