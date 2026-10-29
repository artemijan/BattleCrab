//! `Creature.reduceCurrentHp`'s amount after the damage events — the
//! `DAMAGE_LIMIT` cap and the PvP/PvE damage-taken stats — and the GM
//! `giveDamage` gate in `reduceHp`.

use super::*;
use crate::model::components::stats::{PlayerVitals, StatModifiers, Vitals};
use crate::model::stats::Stat;

const ATTACKER: i32 = 4101;
const VICTIM: i32 = 4102;
const CID: u32 = 1;
const VICTIM_CID: u32 = 2;

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}

/// HP and CP lost together — a playable attacker's hit lands on CP first.
fn hp_cp(world: &World, oid: i32) -> f64 {
    hp(world, oid)
        + world
            .objects
            .get_component::<PlayerVitals>(&oid)
            .map(|v| v.cur_cp)
            .unwrap_or(0.0)
}

fn add_stat(world: &mut World, oid: i32, stat: Stat, amount: f64) {
    if !world.objects.has_component::<StatModifiers>(&oid) {
        world.objects.add_components(&oid, StatModifiers::default());
    }
    let mods = world
        .objects
        .get_component_mut::<StatModifiers>(&oid)
        .unwrap();
    *mods.add.entry(stat).or_insert(0.0) += amount;
}

fn hit(world: &mut World, attacker: i32, target: i32, damage: f64) {
    combat::apply_physical_damage(
        world,
        attacker,
        target,
        damage,
        combat::DamageHit::default(),
    );
}

fn players() -> World {
    let (mut world, _db, _l) = combat_test_world();
    let _a = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    world.cfg.character.alt_game_stun_break = false;
    world
}

/// `damageCap`: a hit lands at most the cap — on a monster and on a player —
/// unless a GM struck it.
#[test]
fn the_damage_cap_limits_a_hit_unless_a_gm_struck_it() {
    let mut world = players();
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    add_stat(&mut world, NPC_OID, Stat::DamageLimit, 30.0);
    add_stat(&mut world, VICTIM, Stat::DamageLimit, 30.0);

    let before = hp(&world, NPC_OID);
    hit(&mut world, ATTACKER, NPC_OID, 40.0);
    assert_eq!(before - hp(&world, NPC_OID), 30.0, "capped on the mob");
    let before = hp_cp(&world, VICTIM);
    hit(&mut world, ATTACKER, VICTIM, 40.0);
    assert_eq!(before - hp_cp(&world, VICTIM), 30.0, "capped on the player");

    world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    world
        .objects
        .get_component_mut::<Player>(&ATTACKER)
        .unwrap()
        .access_level = 70;
    let before = hp(&world, NPC_OID);
    hit(&mut world, ATTACKER, NPC_OID, 40.0);
    assert_eq!(before - hp(&world, NPC_OID), 40.0, "a GM is not capped");
}

/// `pvpDamageTaken` scales a hit from a playable, floored at -80 %.
#[test]
fn pvp_damage_taken_scales_a_player_hit_down_to_a_floor() {
    let mut world = players();
    add_stat(&mut world, VICTIM, Stat::PvpDamageTaken, -50.0);
    let before = hp_cp(&world, VICTIM);
    hit(&mut world, ATTACKER, VICTIM, 40.0);
    assert_eq!(before - hp_cp(&world, VICTIM), 20.0);

    add_stat(&mut world, VICTIM, Stat::PvpDamageTaken, -50.0);
    let before = hp_cp(&world, VICTIM);
    hit(&mut world, ATTACKER, VICTIM, 40.0);
    assert_eq!(before - hp_cp(&world, VICTIM), 8.0, "-100 reads as -80");

    // The PvE stat is not consulted for a playable attacker.
    add_stat(&mut world, VICTIM, Stat::PveDamageTaken, 100.0);
    let before = hp_cp(&world, VICTIM);
    hit(&mut world, ATTACKER, VICTIM, 40.0);
    assert_eq!(before - hp_cp(&world, VICTIM), 8.0);
}

/// A monster's hit takes `pveDamageTaken` and then `pveDamageTakenMonster`;
/// a raid boss's takes `pveDamageTakenRaid` instead of the monster one.
#[test]
fn pve_damage_taken_stacks_with_the_monster_or_raid_stat() {
    let mut world = players();
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 20002, "RaidBoss", 5, 100, 0, 0);
    add_stat(&mut world, VICTIM, Stat::PveDamageTaken, -50.0);
    add_stat(&mut world, VICTIM, Stat::PveDamageTakenMonster, -50.0);
    add_stat(&mut world, VICTIM, Stat::PveDamageTakenRaid, 100.0);
    add_stat(&mut world, VICTIM, Stat::PvpDamageTaken, -80.0);

    let before = hp(&world, VICTIM);
    hit(&mut world, NPC_OID, VICTIM, 40.0);
    assert_eq!(before - hp(&world, VICTIM), 10.0, "40 × 0.5 × 0.5");

    let before = hp(&world, VICTIM);
    hit(&mut world, NPC_OID + 1, VICTIM, 40.0);
    assert_eq!(before - hp(&world, VICTIM), 40.0, "40 × 0.5 × 2");
}

/// A GM whose access level has `giveDamage="false"` deals nothing, to a mob
/// or to a player.
#[test]
fn a_gm_without_give_damage_deals_none() {
    let mut world = players();
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    world
        .data
        .admin
        .insert_level_for_test(crate::data::admin_data::AccessLevel {
            level: 80,
            is_gm: true,
            give_damage: false,
            ..Default::default()
        });
    world
        .objects
        .get_component_mut::<Player>(&ATTACKER)
        .unwrap()
        .access_level = 80;

    let before = hp(&world, NPC_OID);
    hit(&mut world, ATTACKER, NPC_OID, 40.0);
    assert_eq!(hp(&world, NPC_OID), before);
    let before = hp_cp(&world, VICTIM);
    hit(&mut world, ATTACKER, VICTIM, 40.0);
    assert_eq!(hp_cp(&world, VICTIM), before);
}

/// `sendDamageMessage` reports the amount `reduceCurrentHp` landed.
#[test]
fn the_damage_line_reports_the_capped_amount() {
    let (mut world, _db, _l) = combat_test_world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    add_stat(&mut world, NPC_OID, Stat::DamageLimit, 30.0);
    drain(&mut rx);

    effects::apply_skill_damage(
        &mut world,
        ATTACKER,
        NPC_OID,
        effects::SkillHit {
            damage: 40.0,
            caster_name: "Caster",
            skill_id: 1177,
            ..Default::default()
        },
    );
    let expected = server_packets::system_message_with(
        server_packets::sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
        &[
            server_packets::SmParam::PlayerName("Caster".into()),
            server_packets::SmParam::NpcName(20001),
            server_packets::SmParam::Int(30),
            server_packets::SmParam::Popup {
                target: NPC_OID,
                attacker: ATTACKER,
                damage: -30,
            },
        ],
    );
    assert!(drain(&mut rx).contains(&expected));
}

/// The two Salvation effects feeding these stats parse off the dist.
#[test]
fn get_damage_limit_and_damage_by_attack_parse() {
    let sd = dist::skills();
    let of = |id: i32, stat: Stat| {
        sd.get(id, 1)
            .unwrap_or_else(|| panic!("skill {id}"))
            .stat_modifier_effects()
            .iter()
            .find(|m| m.stat == stat)
            .map(|m| m.amount)
    };
    // Adolph's Final Ultimate Defense.
    assert_eq!(of(14852, Stat::DamageLimit), Some(300.0));
    // Nurka's Belt.
    assert_eq!(of(13406, Stat::PvpDamageTaken), Some(-5.0));
    assert_eq!(of(13406, Stat::PveDamageTaken), Some(-5.0));
}
