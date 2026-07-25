//! Baium — archangels and the strider debuff.

use super::*;

use crate::game_loop::baium::{ARCHANGEL, BAIUM};

const BAIUM_OID: i32 = NPC_OID + 110;
const PLAYER: i32 = 9995;
const CID: u32 = 1;
const ANTI_STRIDER: i32 = 4258;
const MOUNT_STRIDER: u8 = 1;

fn baium_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [
        (BAIUM, "GrandBoss"),
        (ARCHANGEL, "Monster"),
        (crate::game_loop::baium::BAIUM_STONE, "Folk"),
        (crate::game_loop::baium::TELE_CUBE, "Folk"),
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 75;
        t.base_hp_max = 100_000.0;
        t.base_mp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            id: ANTI_STRIDER,
            level: 1,
            abnormal_time: 60,
            effects: vec![crate::model::skill::SkillEffect::StatModifier(
                crate::model::skill::StatModifierEffect {
                    stat: crate::model::stats::Stat::RunSpeed,
                    mode: crate::model::stats::StatModifierType::Diff,
                    amount: -50.0,
                    ..Default::default()
                },
            )],
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
    crate::game_loop::baium::spawn_archangels(&mut world);
    assert_eq!(count(&mut world, ARCHANGEL), 5);
}

/// A strider-mounted attacker is hindered.
#[test]
fn a_strider_rider_is_hindered() {
    let (mut world, _db, _l) = baium_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 20, 0, 0);
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&BAIUM_OID)
            .unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world
        .objects
        .get_component_mut::<crate::model::Player>(&PLAYER)
        .unwrap()
        .mount_type = MOUNT_STRIDER;

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
        let v = world
            .objects
            .get_component_mut::<Vitals>(&BAIUM_OID)
            .unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    world
        .objects
        .get_component_mut::<crate::model::Player>(&PLAYER)
        .unwrap()
        .mount_type = MOUNT_STRIDER;

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
    world
        .objects
        .get_component::<BossThreat>(&oid)
        .map(|t| t.slots)
        .unwrap_or_default()
}

fn wound_baium_to(world: &mut World, fraction: f64) {
    let v = world
        .objects
        .get_component_mut::<Vitals>(&BAIUM_OID)
        .unwrap();
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
    assert_eq!(
        melee_v / caster_v,
        150,
        "melee is worth 150x this caster hit"
    );
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
        threat(&world, BAIUM_OID)
            .iter()
            .find(|(id, _)| *id == PLAYER)
            .unwrap()
            .1
    };

    let full = weighted_at(1.0); // (300/3)*20 = 2000
    let three_quarters = weighted_at(0.6); // *10 = 3000
    let half = weighted_at(0.4); // *20 = 6000
    let quarter = weighted_at(0.1); // (300/3)*100 = 10000

    assert_eq!(
        (full, three_quarters, half, quarter),
        (2000, 3000, 6000, 10000)
    );
    assert!(
        quarter > full * 4,
        "a caster matters five times more once Baium is nearly dead"
    );
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

    let ids: Vec<i32> = threat(&world, BAIUM_OID)
        .iter()
        .map(|(id, _)| *id)
        .collect();
    assert!(ids.contains(&104), "the newcomer got on the table");
    assert!(
        !ids.contains(&102),
        "by displacing the weakest, not the oldest"
    );
    assert!(
        ids.contains(&101) && ids.contains(&103),
        "the stronger two stayed"
    );
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
    assert_eq!(
        threat(&world, BAIUM_OID)[0].1,
        after_big,
        "a small hit does not move a large threat"
    );
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
    let (target, _) =
        crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID).expect("a target");
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
    world
        .objects
        .get_component_mut::<Vitals>(&201)
        .unwrap()
        .dead = true;
    world.objects.get_component_mut::<Position>(&202).unwrap().x = 999_999;
    seed_threat(&mut world, 203, 100);

    world.forced_rolls.push_back(99);
    for _ in 0..4 {
        world.forced_rolls.push_back(99);
    }
    let (target, _) =
        crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID).expect("a target");
    assert_eq!(
        target, 203,
        "the only live, nearby attacker — despite the lowest raw threat"
    );
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
            let v = world
                .objects
                .get_component_mut::<Vitals>(&BAIUM_OID)
                .unwrap();
            v.cur_hp = v.max_hp as f64 * fraction;
        }
        world.forced_rolls.push_back(99); // skip the decay
        world.forced_rolls.push_back(5); // the first skill roll hits
        crate::game_loop::baium::manage_skills(&mut world, BAIUM_OID)
            .unwrap()
            .1
    };

    assert_eq!(
        first_option_at(1.0),
        ENERGY_WAVE,
        "above 75%: Energy Wave leads"
    );
    assert_eq!(
        first_option_at(0.6),
        GROUP_HOLD,
        "below 75%: Group Hold joins and leads"
    );
    assert_eq!(
        first_option_at(0.4),
        THUNDERBOLT,
        "below 50%: Thunderbolt joins and leads"
    );
    assert_eq!(
        first_option_at(0.1),
        THUNDERBOLT,
        "below 25%: the full repertoire"
    );
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
        let v = world
            .objects
            .get_component_mut::<Vitals>(&BAIUM_OID)
            .unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
    // BAIUM_ATTACK, the fallback every band ends on.
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            id: 4127,
            level: 1,
            ..Default::default()
        });
    while rx.try_recv().is_ok() {}

    // Jitter 0, no decay, then every ladder roll missing -> the basic attack.
    world.forced_rolls.push_back(0);
    world.forced_rolls.push_back(99);
    for _ in 0..6 {
        world.forced_rolls.push_back(99);
    }
    crate::game_loop::baium::on_baium_damage(&mut world, BAIUM_OID, PLAYER, 500, true);

    let casts = std::iter::from_fn(|| rx.try_recv().ok())
        .filter(|p| p.first() == Some(&0x48))
        .count();
    assert_eq!(casts, 1, "the damage hook chose a skill and cast it");
}

/// A passive Archangel engages the nearest player when it re-picks its target
/// (Java `SELECT_TARGET`) — without this the archangels never fight.
#[test]
fn an_archangel_engages_a_nearby_player() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    let _rx = ingame_player(&mut world, 1, 500, 100, 100, 0);
    add_test_npc(&mut world, 601, ARCHANGEL, "Monster", 75, 150, 150, 0);

    crate::game_loop::baium::handle_select_target(&mut world);

    let hate = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&601)
        .and_then(|a| a.0.get(&500))
        .map(|h| h.hate)
        .unwrap_or(0.0);
    assert!(hate > 0.0, "the archangel engaged the intruder");
}

/// When Baium falls, his archangels leave with him.
#[test]
fn archangels_despawn_when_baium_dies() {
    let (mut world, _db, _l) = baium_world();
    add_test_npc(&mut world, BAIUM_OID, BAIUM, "GrandBoss", 75, 0, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&BAIUM_OID)
        .unwrap()
        .dead = true;
    add_test_npc(&mut world, 601, ARCHANGEL, "Monster", 75, 150, 150, 0);
    assert_eq!(count(&mut world, ARCHANGEL), 1);

    crate::game_loop::baium::handle_select_target(&mut world);
    assert_eq!(
        count(&mut world, ARCHANGEL),
        0,
        "the archangels left with Baium"
    );
}

// ---------------------------------------------------------------------------
// The sleeping stone + the wakeUp awakening
// ---------------------------------------------------------------------------

use crate::game_loop::baium::{BaiumWaker, BAIUM_STONE};

/// Give Baium a grand-boss record at the given status.
fn insert_baium(world: &mut World, status: i32) {
    world.grand_bosses.insert(
        BAIUM,
        crate::model::grand_boss::GrandBoss {
            boss_id: BAIUM,
            loc_x: 116_033,
            loc_y: 17_447,
            loc_z: 10_107,
            heading: 40_188,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status,
        },
    );
}

/// **At rest Baium is a stone statue, not the boss.** ALIVE spawns 29025, and no
/// live Baium (29020) — the old code spawned a fully-aggressive boss at boot.
#[test]
fn baium_rests_as_a_stone_when_alive() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 0); // ALIVE
    let b = world.grand_bosses.get(&BAIUM).unwrap().clone();

    crate::game_loop::baium::spawn_from_record(&mut world, &b);

    assert_eq!(count(&mut world, BAIUM_STONE), 1, "the statue is placed");
    assert_eq!(count(&mut world, BAIUM), 0, "the live boss is not");
}

/// WAITING (server died during the entry window) folds down to ALIVE and still
/// comes back as the statue.
#[test]
fn waiting_folds_to_a_stone() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 1); // WAITING
    let b = world.grand_bosses.get(&BAIUM).unwrap().clone();

    crate::game_loop::baium::spawn_from_record(&mut world, &b);

    assert_eq!(count(&mut world, BAIUM_STONE), 1);
    assert_eq!(
        world.grand_bosses.get(&BAIUM).unwrap().status,
        0,
        "WAITING was folded to ALIVE"
    );
}

/// **Waking the stone raises Baium.** The status flips ALIVE→IN_FIGHT (locking
/// entry), the statue is removed, the live boss appears, and the cinematic is
/// armed.
#[test]
fn waking_the_stone_raises_baium() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 0); // ALIVE
    add_test_npc(
        &mut world,
        700,
        BAIUM_STONE,
        "Folk",
        75,
        116_033,
        17_447,
        10_107,
    );
    let before = world.scheduler.len();

    let raised = crate::game_loop::baium::wake_up(&mut world, 700, PLAYER);

    assert!(raised.is_some(), "the wake took");
    assert_eq!(
        world.grand_bosses.get(&BAIUM).unwrap().status,
        2,
        "IN_FIGHT — entry is now locked"
    );
    assert_eq!(count(&mut world, BAIUM_STONE), 0, "the statue is gone");
    assert_eq!(count(&mut world, BAIUM), 1, "the boss is up");
    assert!(world.scheduler.len() > before, "the cinematic is armed");
}

/// A second raid cannot wake an already-woken Baium — `wake_up` is a no-op
/// unless he is ALIVE, so two parties can't spawn two bosses.
#[test]
fn a_woken_baium_cannot_be_woken_again() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 2); // IN_FIGHT already
    add_test_npc(&mut world, 700, BAIUM_STONE, "Folk", 75, 0, 0, 0);

    let raised = crate::game_loop::baium::wake_up(&mut world, 700, PLAYER);

    assert!(raised.is_none(), "the second wake was refused");
    assert_eq!(count(&mut world, BAIUM), 0, "no second boss spawned");
}

/// **The cinematic's final beat starts the fight:** the archangels arrive,
/// Baium takes his AI back (the movement pin lifts) and he engages the waker.
#[test]
fn the_final_beat_spawns_archangels_and_engages() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 2); // IN_FIGHT
    let _rx = ingame_player(&mut world, 1, PLAYER, 116_000, 17_400, 10_107);
    add_test_npc(
        &mut world,
        BAIUM_OID,
        BAIUM,
        "GrandBoss",
        75,
        116_033,
        17_447,
        10_107,
    );
    world
        .objects
        .add_components(&BAIUM_OID, crate::model::components::Immobilized);
    world
        .objects
        .add_components(&BAIUM_OID, BaiumWaker { player_oid: PLAYER });

    // Step 5 is SPAWN_ARCHANGEL (the last beat).
    crate::game_loop::baium::handle_cinematic_step(&mut world, 5);

    assert_eq!(count(&mut world, ARCHANGEL), 5, "the guardians arrived");
    assert!(
        !world
            .objects
            .has_component::<crate::model::components::Immobilized>(&BAIUM_OID),
        "Baium is free to move — his AI is back"
    );
    let hate = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&BAIUM_OID)
        .and_then(|a| a.0.get(&PLAYER))
        .map(|h| h.hate)
        .unwrap_or(0.0);
    assert!(hate > 0.0, "Baium engaged the waker");
}

/// A crash mid-fight recovers the **live** boss (not the statue) with his
/// archangels, at his stored HP.
#[test]
fn in_fight_status_recovers_the_live_boss() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 2); // IN_FIGHT
    world.grand_bosses.get_mut(&BAIUM).unwrap().current_hp = 40_000.0;
    let b = world.grand_bosses.get(&BAIUM).unwrap().clone();

    crate::game_loop::baium::spawn_from_record(&mut world, &b);

    assert_eq!(count(&mut world, BAIUM), 1, "the live boss is back");
    assert_eq!(count(&mut world, BAIUM_STONE), 0, "no statue");
    assert_eq!(count(&mut world, ARCHANGEL), 5, "his guardians too");
}

/// The 13F report: an archangel in the lobby (z 10 136) stood ~85 *2D* units
/// from a player on the tower floor below (z 9 208) and locked on straight
/// through the geometry. Java gates every pick on the boss zone
/// (`zone.isInsideZone(creature)`, `baium_no_restart` z 10 061 – 11 061) and
/// measures the 1000 reach in 3D — the player below must be invisible to it,
/// while a player actually in the lobby still gets engaged.
#[test]
fn an_archangel_ignores_a_player_on_the_floor_below_the_zone() {
    let (mut world, _db, _l) = baium_world();
    // `baium_no_restart` (70051), simplified to a cuboid over the lobby.
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        id: 70051,
        name: "baium_no_restart".into(),
        kind: crate::data::zone_data::ZoneKind::NoRestart,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: 113_000,
                x2: 118_000,
                y1: 14_000,
                y2: 19_000,
            },
            min_z: 10_061,
            max_z: 11_061,
        },
        castle_id: 0,
        effect: None,
        damage: None,
        swamp: None,
    });
    add_test_npc(
        &mut world,
        BAIUM_OID,
        BAIUM,
        "GrandBoss",
        75,
        116_033,
        17_447,
        10_107,
    );
    // The reported spot on 13F, right under archangel post 4.
    let _below = ingame_player(&mut world, 1, 500, 114_804, 16_197, 9_208);
    add_test_npc(
        &mut world, 601, ARCHANGEL, "Monster", 75, 114_880, 16_236, 10_136,
    );

    crate::game_loop::baium::handle_select_target(&mut world);
    let hate_below = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&601)
        .and_then(|a| a.0.get(&500))
        .map(|h| h.hate)
        .unwrap_or(0.0);
    assert_eq!(
        hate_below, 0.0,
        "a player below the boss zone must not be engaged through the floor"
    );

    // Control: a player actually inside the lobby zone is engaged.
    let _inside = ingame_player(&mut world, 2, 501, 114_900, 16_240, 10_100);
    crate::game_loop::baium::handle_select_target(&mut world);
    let hate_inside = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&601)
        .and_then(|a| a.0.get(&501))
        .map(|h| h.hate)
        .unwrap_or(0.0);
    assert!(hate_inside > 0.0, "the in-zone player is still engaged");
}

/// A hated player who leaves the boss zone is abandoned on the next re-pick
/// (Java's keep-branch requires `zone.isInsideZone(mostHated)`): the hate
/// entry goes away, so the generic attack loop stops chasing them out of the
/// room.
#[test]
fn an_archangel_abandons_a_target_that_left_the_zone() {
    let (mut world, _db, _l) = baium_world();
    world.data.zone_data.insert(crate::data::zone_data::Zone {
        id: 70051,
        name: "baium_no_restart".into(),
        kind: crate::data::zone_data::ZoneKind::NoRestart,
        territory: crate::data::spawn_data::Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: 113_000,
                x2: 118_000,
                y1: 14_000,
                y2: 19_000,
            },
            min_z: 10_061,
            max_z: 11_061,
        },
        castle_id: 0,
        effect: None,
        damage: None,
        swamp: None,
    });
    add_test_npc(
        &mut world,
        BAIUM_OID,
        BAIUM,
        "GrandBoss",
        75,
        116_033,
        17_447,
        10_107,
    );
    let _rx = ingame_player(&mut world, 1, 500, 114_900, 16_240, 10_100);
    add_test_npc(
        &mut world, 601, ARCHANGEL, "Monster", 75, 114_880, 16_236, 10_136,
    );
    crate::game_loop::baium::handle_select_target(&mut world);
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&601)
            .and_then(|a| a.0.get(&500))
            .is_some(),
        "engaged while inside"
    );

    // The player jumps down to 13F.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&500)
    {
        p.x = 114_804;
        p.y = 16_197;
        p.z = 9_208;
    }
    crate::game_loop::baium::handle_select_target(&mut world);
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&601)
            .and_then(|a| a.0.get(&500))
            .is_none(),
        "the departed player's hate entry is dropped"
    );
}

// ---------------------------------------------------------------------------
// Entry (Angelic Vortex), exit (teleport cube) and the death cube
// ---------------------------------------------------------------------------

use crate::game_loop::baium::{EntryOutcome, TELE_CUBE};

/// A fabric-bearer is admitted while Baium sleeps.
#[test]
fn the_vortex_admits_a_fabric_bearer() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 0); // ALIVE
    assert_eq!(
        crate::game_loop::baium::entry_outcome(&world, true),
        EntryOutcome::Admitted
    );
}

/// No fabric, no crossing — the vortex is inert.
#[test]
fn the_vortex_is_inert_without_a_fabric() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 0); // ALIVE
    assert_eq!(
        crate::game_loop::baium::entry_outcome(&world, false),
        EntryOutcome::NoFabric
    );
}

/// **The fight's state is read before the fabric.** A player without a fabric
/// still sees "it's busy" mid-fight and "it's over" once Baium is dead — Java's
/// order, which a reordering would silently lose (they'd get NoFabric instead).
#[test]
fn state_is_reported_before_the_fabric_check() {
    let (mut world, _db, _l) = baium_world();

    insert_baium(&mut world, 2); // IN_FIGHT
    assert_eq!(
        crate::game_loop::baium::entry_outcome(&world, false),
        EntryOutcome::InFight,
        "busy beats no-fabric"
    );

    world.grand_bosses.get_mut(&BAIUM).unwrap().status = 3; // DEAD
    assert_eq!(
        crate::game_loop::baium::entry_outcome(&world, false),
        EntryOutcome::Dead,
        "over beats no-fabric"
    );
}

/// Killing Baium drops the exit cube where the raid can find it.
#[test]
fn killing_baium_drops_the_exit_cube() {
    let (mut world, _db, _l) = baium_world();
    assert_eq!(count(&mut world, TELE_CUBE), 0);

    crate::game_loop::baium::on_baium_killed(&mut world);

    assert_eq!(count(&mut world, TELE_CUBE), 1, "the way out appeared");
}

/// The cube scatters people to one of three surface points, jittered — never
/// dropping them back at a fixed, campable spot.
#[test]
fn the_exit_scatters_to_a_surface_point() {
    let (mut world, _db, _l) = baium_world();
    // roll(3) -> point 1, then the two +100 jitters.
    world.forced_rolls.push_back(1);
    world.forced_rolls.push_back(40);
    world.forced_rolls.push_back(60);

    let (x, y, z) = crate::game_loop::baium::random_exit(&mut world);

    assert_eq!((x, y, z), (113_824 + 40, 10_448 + 60, -5_164));
}

// ---------------------------------------------------------------------------
// CHECK_ATTACK — the idle reset and the self-heal
// ---------------------------------------------------------------------------

use crate::game_loop::baium::BaiumCombat;

/// **Thirty minutes with nobody landing a hit resets the fight:** the zone is
/// emptied, the sleeping stone goes back and Baium reverts to ALIVE (Java's
/// CHECK_ATTACK reset). `18_000` ticks = 30 min at 10 ticks/s.
#[test]
fn a_thirty_minute_idle_reverts_baium_to_stone() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 2); // IN_FIGHT
    add_test_npc(
        &mut world,
        BAIUM_OID,
        BAIUM,
        "GrandBoss",
        75,
        116_033,
        17_447,
        10_107,
    );
    world.objects.add_components(
        &BAIUM_OID,
        BaiumCombat {
            last_attack_tick: 0,
        },
    );
    world.tick = 18_001; // > 30 min since last_attack 0

    crate::game_loop::baium::handle_check_attack(&mut world);

    assert_eq!(
        world.grand_bosses.get(&BAIUM).unwrap().status,
        0,
        "reverted to ALIVE"
    );
    assert_eq!(count(&mut world, BAIUM_STONE), 1, "the statue is back");
    assert_eq!(count(&mut world, BAIUM), 0, "the live boss was cleared");
}

/// A hit within the window keeps Baium fighting and the beat re-arms.
#[test]
fn a_recently_hit_baium_keeps_fighting() {
    let (mut world, _db, _l) = baium_world();
    insert_baium(&mut world, 2); // IN_FIGHT
    add_test_npc(
        &mut world,
        BAIUM_OID,
        BAIUM,
        "GrandBoss",
        75,
        116_033,
        17_447,
        10_107,
    );
    world.tick = 10_000;
    world.objects.add_components(
        &BAIUM_OID,
        BaiumCombat {
            last_attack_tick: world.tick,
        },
    );
    let before = world.scheduler.len();

    crate::game_loop::baium::handle_check_attack(&mut world);

    assert_eq!(
        world.grand_bosses.get(&BAIUM).unwrap().status,
        2,
        "still IN_FIGHT"
    );
    assert_eq!(count(&mut world, BAIUM), 1, "still up");
    assert!(world.scheduler.len() > before, "the beat re-armed");
}

/// Five idle minutes and below 75% HP → Baium heals himself (`HEAL_OF_BAIUM`),
/// rather than resetting — the window that lets a stalled raid recover is not
/// the same one that abandons the fight.
#[test]
fn a_wounded_idle_baium_heals_itself() {
    let (mut world, _db, _l) = baium_world();
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            id: 4135,
            level: 1,
            ..Default::default()
        });
    insert_baium(&mut world, 2); // IN_FIGHT
    add_test_npc(
        &mut world,
        BAIUM_OID,
        BAIUM,
        "GrandBoss",
        75,
        116_033,
        17_447,
        10_107,
    );
    {
        let v = world
            .objects
            .get_component_mut::<Vitals>(&BAIUM_OID)
            .unwrap();
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
        v.cur_hp = v.max_hp as f64 * 0.5; // wounded, below 75%
    }
    world.tick = 5_000;
    world.objects.add_components(
        &BAIUM_OID,
        BaiumCombat {
            last_attack_tick: 2_000, // 3_000 ticks (5 min) idle — heal, not reset
        },
    );

    crate::game_loop::baium::handle_check_attack(&mut world);

    assert!(
        world
            .objects
            .has_component::<crate::model::components::Casting>(&BAIUM_OID),
        "Baium began healing himself"
    );
    assert_eq!(
        world.grand_bosses.get(&BAIUM).unwrap().status,
        2,
        "the heal window does not reset the fight"
    );
}

/// Fifteen minutes after the kill the lair is force-emptied: the exit cube
/// despawns and any straggler is sent to the surface (Java's post-kill
/// `CLEAR_ZONE`).
#[test]
fn the_lair_is_emptied_after_the_kill() {
    let (mut world, _db, _l) = baium_world();
    let _rx = ingame_player(&mut world, 1, PLAYER, 116_000, 17_400, 10_107);
    let before = world.scheduler.len();

    crate::game_loop::baium::on_baium_killed(&mut world);
    assert_eq!(count(&mut world, TELE_CUBE), 1, "the cube dropped");
    assert!(world.scheduler.len() > before, "CLEAR_ZONE is armed");

    // The 900 s timer fires: cube gone, straggler ousted to the first exit.
    world.forced_rolls.push_back(0); // exit point 0
    world.forced_rolls.push_back(0); // x jitter
    world.forced_rolls.push_back(0); // y jitter
    crate::game_loop::baium::handle_clear_zone(&mut world);

    assert_eq!(count(&mut world, TELE_CUBE), 0, "the cube despawned");
    let p = world.objects.get_component::<Position>(&PLAYER).unwrap();
    assert_eq!((p.x, p.y), (108_784, 16_000), "the straggler was sent out");
}
