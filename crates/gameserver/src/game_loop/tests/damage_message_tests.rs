//! `sendDamageMessage`, sent from the end of `reduceCurrentHp`: the
//! `Player`, `Summon` and `Cubic` overrides, their blocked and door/tower
//! lines, and the `Hp` effect's second line.

use super::*;
use crate::game_loop::helpers;
use crate::model::components::player::AdminFlags;
use crate::model::components::summons::{CubicOf, PetOf, ServitorOf};
use server_packets::{SmParam, sm_ids};

const ATTACKER: i32 = 4201;
const VICTIM: i32 = 4202;
const SUMMON: i32 = NPC_OID + 50;
const CID: u32 = 1;
const VICTIM_CID: u32 = 2;

fn world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    world.cfg.character.alt_game_stun_break = false;
    (world, db, l)
}

fn hit(world: &mut World, attacker: i32, target: i32, hit: combat::DamageHit) {
    combat::apply_physical_damage(world, attacker, target, 10.0, hit);
}

/// The system-message ids among `packets`, in order.
fn sm_ids_of(packets: &[Vec<u8>]) -> Vec<i16> {
    packets
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE)
        .map(|p| sm_id(p))
        .collect()
}

fn inflicted(attacker: SmParam, target: i32, name: String, popup_attacker: i32) -> Vec<u8> {
    server_packets::system_message_with(
        sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
        &[
            attacker,
            SmParam::Text(name),
            SmParam::Int(10),
            SmParam::Popup {
                target,
                attacker: popup_attacker,
                damage: -10,
            },
        ],
    )
}

fn hp_block(world: &mut World, oid: i32) {
    if !world.objects.has_component::<Buffs>(&oid) {
        world.objects.add_components(&oid, Buffs::default());
    }
    give_flag_buff(world, oid, 9999, model::skill::effect_flag::HP_BLOCK);
}

/// A melee crit: `C1_LANDED_A_CRITICAL_HIT`, then the damage line naming the
/// target as a plain string. No `ExMagicAttackInfo` without a skill.
#[test]
fn a_melee_crit_sends_the_crit_line_then_the_damage_line() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    drain(&mut rx);

    hit(
        &mut world,
        ATTACKER,
        NPC_OID,
        combat::DamageHit {
            critical: true,
            ..Default::default()
        },
    );
    let out = drain(&mut rx);
    assert_eq!(
        sm_ids_of(&out),
        [
            sm_ids::C1_LANDED_A_CRITICAL_HIT,
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2
        ]
    );
    let name = helpers::player_name_or_empty(&world, ATTACKER);
    assert!(out.contains(&inflicted(
        SmParam::PlayerName(name),
        NPC_OID,
        game_loop::npc::npc_name_or_empty(&world, NPC_OID),
        ATTACKER,
    )));
    assert!(!out.iter().any(|p| p[0] == server_packets::opcodes::EX));
}

/// A skill crit adds the `CRITICAL` popup: `M_CRITICAL` for a magic skill,
/// `C1_LANDED_A_CRITICAL_HIT` for a physical one.
#[test]
fn a_skill_crit_adds_the_critical_popup() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let popup = server_packets::ex_magic_attack_info(
        ATTACKER,
        NPC_OID,
        server_packets::magic_attack_info::CRITICAL,
    );
    drain(&mut rx);

    for (magic, crit_line) in [
        (true, sm_ids::M_CRITICAL),
        (false, sm_ids::C1_LANDED_A_CRITICAL_HIT),
    ] {
        hit(
            &mut world,
            ATTACKER,
            NPC_OID,
            combat::DamageHit {
                critical: true,
                magic,
                ..combat::DamageHit::SKILL
            },
        );
        let out = drain(&mut rx);
        let crit = out
            .iter()
            .position(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE && sm_id(p) == crit_line)
            .expect("crit line");
        assert_eq!(out[crit + 1], popup, "magic {magic}: the popup follows");
        assert_eq!(
            sm_id(&out[crit + 2]),
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2
        );
    }
}

/// `(target.isHpBlocked() && !target.isNpc()) || target.isInvul()`: an
/// HP-blocked player is "blocked", an HP-blocked mob still shows the damage
/// line, and an invulnerable mob is "blocked".
#[test]
fn the_blocked_line_follows_javas_npc_exemption() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    hp_block(&mut world, VICTIM);
    hp_block(&mut world, NPC_OID);
    drain(&mut rx);

    hit(&mut world, ATTACKER, VICTIM, combat::DamageHit::default());
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        [sm_ids::THE_ATTACK_HAS_BEEN_BLOCKED]
    );

    hit(&mut world, ATTACKER, NPC_OID, combat::DamageHit::default());
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        [sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2]
    );

    helpers::update_admin_flags(&mut world, NPC_OID, |f: &mut AdminFlags| f.invul = true);
    hit(&mut world, ATTACKER, NPC_OID, combat::DamageHit::default());
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        [sm_ids::THE_ATTACK_HAS_BEEN_BLOCKED]
    );
}

/// A control tower gets the bare `YOU_HIT_FOR_S1_DAMAGE`.
#[test]
fn a_control_tower_gets_the_plain_damage_line() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 13002, "ControlTower", 5, 100, 0, 0);
    drain(&mut rx);

    hit(&mut world, ATTACKER, NPC_OID, combat::DamageHit::default());
    let expected =
        server_packets::system_message_with(sm_ids::YOU_HIT_FOR_S1_DAMAGE, &[SmParam::Int(10)]);
    let out = drain(&mut rx);
    assert!(out.contains(&expected));
    assert_eq!(sm_ids_of(&out), [sm_ids::YOU_HIT_FOR_S1_DAMAGE]);
}

/// The line closes `reduceCurrentHp`, so on a kill it comes after `Die`.
#[test]
fn the_damage_line_follows_the_kill() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    drain(&mut rx);

    combat::apply_physical_damage(
        &mut world,
        ATTACKER,
        NPC_OID,
        1_000.0,
        combat::DamageHit::default(),
    );
    let out = drain(&mut rx);
    let die = out
        .iter()
        .position(|p| p[0] == server_packets::opcodes::DIE)
        .expect("Die");
    let line = out
        .iter()
        .position(|p| {
            p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2
        })
        .expect("damage line");
    assert!(die < line);
}

/// A missed swing sends nothing: Salvation never calls `sendDamageMessage`
/// with `miss`.
#[test]
fn a_missed_swing_sends_no_line() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    let mut victim_rx = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    drain(&mut rx);
    drain(&mut victim_rx);

    combat::handle_attack_hit(&mut world, ATTACKER, VICTIM, 10, true, false, 0);
    assert!(sm_ids_of(&drain(&mut rx)).is_empty());
    assert!(sm_ids_of(&drain(&mut victim_rx)).is_empty());
}

fn own_summon(world: &mut World, owner: i32, pet: bool) {
    add_test_npc(world, SUMMON, 12077, "Servitor", 5, 100, 0, 0);
    world.objects.add_components(
        &SUMMON,
        ServitorOf {
            owner_object_id: owner,
            reference_skill: 0,
            expires_at_tick: u64::MAX,
            life_time_secs: 0,
            following: false,
            defending: false,
            consume_item_id: 0,
            consume_item_count: 0,
            next_consume_tick: u64::MAX,
        },
    );
    if pet {
        world.objects.add_components(
            &SUMMON,
            PetOf {
                collar_object_id: 0,
                fed: 0,
                max_fed: 0,
                level: 5,
                exp: 0,
                sp: 0,
                exp_before_death: 0,
            },
        );
    }
}

/// `Summon.sendDamageMessage` goes to the owner: the servitor's or the pet's
/// crit line, then the damage line under the summon's NPC name — and nothing
/// when the summon hit its own owner.
#[test]
fn a_summons_line_goes_to_its_owner() {
    for (pet, crit_line) in [
        (false, sm_ids::SUMMONED_MONSTER_S_CRITICAL_HIT),
        (true, sm_ids::PET_S_CRITICAL_HIT),
    ] {
        let (mut world, _db, _l) = world();
        let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
        add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
        own_summon(&mut world, ATTACKER, pet);
        drain(&mut rx);

        hit(
            &mut world,
            SUMMON,
            NPC_OID,
            combat::DamageHit {
                critical: true,
                ..Default::default()
            },
        );
        let out = drain(&mut rx);
        assert_eq!(
            sm_ids_of(&out),
            [crit_line, sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2],
            "pet {pet}"
        );
        assert!(out.contains(&inflicted(
            SmParam::NpcName(12077),
            NPC_OID,
            game_loop::npc::npc_name_or_empty(&world, NPC_OID),
            SUMMON,
        )));

        hit(&mut world, SUMMON, ATTACKER, combat::DamageHit::default());
        assert!(
            !sm_ids_of(&drain(&mut rx)).contains(&sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2),
            "pet {pet}: no line for hitting the owner"
        );
    }
}

/// `Cubic.sendDamageMessage`: to the owner, named after the owner, with the
/// owner as the popup's attacker.
#[test]
fn a_cubics_line_goes_to_its_owner_under_the_owners_name() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let cubic = NPC_OID + 60;
    world.objects.spawn(
        cubic,
        crate::model::components::stats::Vitals::hp_full(1, 1),
    );
    world.objects.add_components(
        &cubic,
        CubicOf {
            owner_object_id: ATTACKER,
        },
    );
    drain(&mut rx);

    hit(&mut world, cubic, NPC_OID, combat::DamageHit::SKILL);
    let out = drain(&mut rx);
    assert!(out.contains(&inflicted(
        SmParam::Text(helpers::player_name_or_empty(&world, ATTACKER)),
        NPC_OID,
        game_loop::npc::npc_name_or_empty(&world, NPC_OID),
        ATTACKER,
    )));
}

/// `Hp` with a negative amount is damage through `reduceCurrentHp` — which
/// sends the line — and then `sendDamageMessage` again.
#[test]
fn a_negative_hp_effect_damages_and_reports_twice() {
    let (mut world, _db, _l) = world();
    let mut rx = ingame_caster(&mut world, CID, ATTACKER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);
    let skill = Skill {
        id: 15197,
        level: 1,
        effects: vec![model::skill::effects::SkillEffect::Hp {
            amount: -10.0,
            percent: false,
        }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill.clone());
    let before = nvit(&world, NPC_OID).cur_hp;
    drain(&mut rx);

    effects::apply_skill_effects(&mut world, ATTACKER, NPC_OID, &skill);
    assert_eq!(before - nvit(&world, NPC_OID).cur_hp, 10.0);
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        [
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2
        ]
    );
}
