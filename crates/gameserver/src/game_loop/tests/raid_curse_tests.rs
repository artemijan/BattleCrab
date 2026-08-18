//! The raid curse — G23's named gate clause.

use super::*;
use crate::game_loop::abnormal::has_buff;

const PLAYER: i32 = 9950;
const CID: u32 = 1;
const BOSS: i32 = NPC_OID + 40;
const BOSS_NPC: i32 = 29001;
const MOB_NPC: i32 = 29002;

/// 4515 `RAID_CURSE2` — petrification, for laying hands on the boss.
const RAID_CURSE2: i32 = 4515;
/// 4215 `RAID_CURSE` — silence, for helping from a distance.
const RAID_CURSE: i32 = 4215;

fn curse_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [(BOSS_NPC, "RaidBoss"), (MOB_NPC, "Monster")] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 20;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    for (id, effects) in [
        (
            RAID_CURSE2,
            vec![model::skill::SkillEffect::BlockActions { conditional: false }],
        ),
        (
            RAID_CURSE,
            vec![model::skill::SkillEffect::StatModifier(
                model::skill::StatModifierEffect {
                    stat: Stat::RunSpeed,
                    mode: model::stats::StatModifierType::Diff,
                    amount: -1.0,
                    ..Default::default()
                },
            )],
        ),
    ] {
        world.data.skill_data.insert_for_test(Skill {
            self_continuous: false,
            id,
            level: 1,
            abnormal_time: 120,
            effects,
            ..Default::default()
        });
    }
    (world, db, l)
}

fn set_level(world: &mut World, oid: i32, level: i32) {
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .level = level;
}

/// An over-levelled attacker is petrified. The boss is level 20, so 29 is the
/// first cursed level (Java's `> level + 8`).
#[test]
fn attacking_a_raid_boss_nine_levels_below_curses_the_attacker() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    set_level(&mut world, PLAYER, 29);

    crate::game_loop::raid_curse::on_raid_attacked(&mut world, BOSS, PLAYER);
    assert!(
        has_buff(&world, PLAYER, RAID_CURSE2),
        "petrified for attacking it"
    );
}

/// Exactly 8 levels above is **not** cursed — the boundary Java writes as
/// `> level + 8`, and the one an "improvement" to `>= 9` would move.
#[test]
fn eight_levels_above_is_not_cursed() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    set_level(&mut world, PLAYER, 28);

    crate::game_loop::raid_curse::on_raid_attacked(&mut world, BOSS, PLAYER);
    assert!(
        !has_buff(&world, PLAYER, RAID_CURSE2),
        "28 vs 20 is exactly 8 — no curse"
    );
}

/// An ordinary monster never curses, however over-levelled the attacker.
#[test]
fn an_ordinary_monster_does_not_curse() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BOSS, MOB_NPC, "Monster", 20, 60, 0, 0);
    set_level(&mut world, PLAYER, 80);

    crate::game_loop::raid_curse::on_raid_attacked(&mut world, BOSS, PLAYER);
    assert!(!has_buff(&world, PLAYER, RAID_CURSE2));
}

/// `DisableRaidCurse` is honoured rather than assumed.
#[test]
fn the_disable_config_is_honoured() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    set_level(&mut world, PLAYER, 40);
    world.cfg.npc.disable_raid_curse = true;

    crate::game_loop::raid_curse::on_raid_attacked(&mut world, BOSS, PLAYER);
    assert!(!has_buff(&world, PLAYER, RAID_CURSE2));
}

/// Casting a **bad** skill near a boss that is fighting petrifies; a **good**
/// one silences. This is the clause the damage-side check never sees — a
/// high-level player buffing a low-level party from outside the fight.
#[test]
fn casting_near_a_fighting_raid_boss_curses_by_skill_kind() {
    for (is_bad, expected) in [(true, RAID_CURSE2), (false, RAID_CURSE)] {
        let (mut world, _db, _l) = curse_world();
        let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
        add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
        set_level(&mut world, PLAYER, 40);
        // The boss must be in combat: someone else is fighting it.
        world
            .objects
            .get_component_mut::<AggroList>(&BOSS)
            .unwrap()
            .0
            .entry(PLAYER + 1)
            .or_default()
            .damage = 100.0;

        crate::game_loop::raid_curse::on_skill_cast_near_raid(&mut world, PLAYER, is_bad);
        assert!(
            has_buff(&world, PLAYER, expected),
            "is_bad={is_bad} should apply {expected}"
        );
    }
}

/// An **idle** boss curses nobody — casting near one that is not fighting is
/// free, which is what keeps ordinary travel past a spawn point safe.
#[test]
fn casting_near_an_idle_raid_boss_is_free() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    set_level(&mut world, PLAYER, 40);

    crate::game_loop::raid_curse::on_skill_cast_near_raid(&mut world, PLAYER, true);
    assert!(
        !has_buff(&world, PLAYER, RAID_CURSE2),
        "an idle boss does not curse"
    );
}

/// End-to-end through the real damage path: the helper being right proves
/// nothing if `apply_physical_damage` never calls it. Also pins Java's
/// ordering — *"in retail you deal damage to raid before curse"* — so the hit
/// that earns the curse still lands.
#[test]
fn a_real_hit_curses_and_still_deals_its_damage() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    set_level(&mut world, PLAYER, 40);

    let before = world.objects.get_component::<Vitals>(&BOSS).unwrap().cur_hp;
    combat::apply_physical_damage(&mut world, PLAYER, BOSS, 500.0, false, false);

    assert!(
        has_buff(&world, PLAYER, RAID_CURSE2),
        "the attacker was cursed"
    );
    assert!(
        world.objects.get_component::<Vitals>(&BOSS).unwrap().cur_hp < before,
        "and the hit still dealt its damage"
    );
}

// ---------------------------------------------------------------------------
// Raid points (slice 2)
// ---------------------------------------------------------------------------

fn raid_with_points(world: &mut World, points: f64) {
    let mut t = world.data.npc_data.get(BOSS_NPC).unwrap().clone();
    t.raid_points = points;
    t.exp = 1000.0;
    world.data.npc_data.insert_for_test(t);
}

fn points_of(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .raidboss_points
}

fn seed_damage(world: &mut World, npc: i32, dealer: i32, dmg: f64) {
    world
        .objects
        .get_component_mut::<AggroList>(&npc)
        .unwrap()
        .0
        .entry(dealer)
        .or_default()
        .damage = dmg;
}

/// Killing a raid boss awards its raid points to the top damage dealer.
#[test]
fn killing_a_raid_boss_awards_raid_points() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    raid_with_points(&mut world, 100.0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    seed_damage(&mut world, BOSS, PLAYER, 500.0);

    crate::game_loop::death::npc_do_die(&mut world, BOSS, PLAYER);
    assert_eq!(points_of(&world, PLAYER), 100, "solo killer takes the lot");
}

/// An ordinary monster awards none, however much damage was done.
#[test]
fn an_ordinary_monster_awards_no_raid_points() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, BOSS, MOB_NPC, "Monster", 20, 60, 0, 0);
    seed_damage(&mut world, BOSS, PLAYER, 500.0);

    crate::game_loop::death::npc_do_die(&mut world, BOSS, PLAYER);
    assert_eq!(points_of(&world, PLAYER), 0);
}

/// A party **splits** the award, and nobody rounds down to zero — Java's
/// `Math.max(points / size, 1)`.
#[test]
fn a_party_splits_the_raid_points() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let mate = PLAYER + 1;
    let _rx2 = ingame_caster(&mut world, CID + 1, mate, 40, 0);
    raid_with_points(&mut world, 100.0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    seed_damage(&mut world, BOSS, PLAYER, 500.0);

    make_party(&mut world, &[PLAYER, mate], LootRule::FindersKeepers);

    crate::game_loop::death::npc_do_die(&mut world, BOSS, PLAYER);
    assert_eq!(points_of(&world, PLAYER), 50, "split two ways");
    assert_eq!(
        points_of(&world, mate),
        50,
        "including the member who dealt no damage"
    );
}

/// A party member out of range of the corpse earns nothing — the split is
/// measured from the boss, so hanging back costs you the reward.
#[test]
fn a_distant_party_member_earns_no_raid_points() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let mate = PLAYER + 1;
    let _rx2 = ingame_caster(&mut world, CID + 1, mate, 40, 0);
    raid_with_points(&mut world, 100.0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    seed_damage(&mut world, BOSS, PLAYER, 500.0);
    make_party(&mut world, &[PLAYER, mate], LootRule::FindersKeepers);
    world
        .objects
        .get_component_mut::<Position>(&mate)
        .unwrap()
        .x += 100_000;

    crate::game_loop::death::npc_do_die(&mut world, BOSS, PLAYER);
    assert_eq!(points_of(&world, mate), 0, "out of range, no points");
    assert_eq!(
        points_of(&world, PLAYER),
        100,
        "and the one in range takes the whole share"
    );
}

/// `RateRaidbossPointsReward` is honoured.
#[test]
fn the_raid_point_rate_is_applied() {
    let (mut world, _db, _l) = curse_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    raid_with_points(&mut world, 100.0);
    add_test_npc(&mut world, BOSS, BOSS_NPC, "RaidBoss", 20, 60, 0, 0);
    seed_damage(&mut world, BOSS, PLAYER, 500.0);
    world.cfg.rates.rate_raidboss_points = 3.0;

    crate::game_loop::death::npc_do_die(&mut world, BOSS, PLAYER);
    assert_eq!(points_of(&world, PLAYER), 300);
}

/// The real datapack carries non-zero raid points — a fixture can't catch a
/// parse regression on `<acquire raidPoints>`.
#[test]
fn the_real_datapack_parses_raid_points() {
    let npcs = dist::npcs();
    let with_points = (1..=30000)
        .filter(|id| npcs.get(*id).is_some_and(|t| t.raid_points > 0.0))
        .count();
    assert!(
        with_points > 100,
        "expected many raid-point NPCs, got {with_points}"
    );
}
