//! `SilentMove` + `FakeDeath` — the two ways a playable drops off an aggro
//! scan (G19).
//!
//! Java checks both on adjacent lines of `AttackableAI.isAggressiveTowards`,
//! which is why they land together. Before this slice the port's aggro scan
//! carried a literal comment saying "invisibility/silent-move/GM states don't
//! exist": Silent Move 221 and Stealth 411 gave their stat bonuses but never
//! actually hid anyone, and Fake Death 60 — whose only two effects are these —
//! parsed to an empty effect list and was **dropped whole**.

use super::*;

use crate::model::components::Buffs;
use crate::model::skill::{SkillEffect, effect_flag};

const PLAYER: i32 = 5001;
const CID: u32 = 1;
const MOB_ID: i32 = 45000;
const RAID_ID: i32 = 45001;
const MOB_OID: i32 = NPC_OID;

/// An aggressive monster that would notice anyone in range.
fn aggressive_template(id: i32, type_name: &str) -> crate::data::npc_data::NpcTemplate {
    let mut t = crate::data::npc_data::default_template(id);
    t.type_name = type_name.into();
    t.name = format!("Watcher {id}");
    t.level = 20;
    t.base_hp_max = 500.0;
    t.is_aggressive = true;
    t.aggro_range = 450;
    t.collision_radius = 10.0;
    t
}

fn stealth_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    world
        .data
        .npc_data
        .insert_for_test(aggressive_template(MOB_ID, "Monster"));
    world
        .data
        .npc_data
        .insert_for_test(aggressive_template(RAID_ID, "RaidBoss"));
    (world, db, l)
}

/// Long enough for the aggro scan to actually run. `NpcAi.global_aggro` starts
/// at **-10** and creeps up by 1 per think tick (every 10 game ticks), and the
/// generic scan is gated on it reaching 0 — so a monster needs ~100 ticks of
/// existence before it notices anybody. (Guards are exempt: their PK scan isn't
/// gated on it, which is why the guard tests elsewhere get away with 20.)
const AGGRO_WARMUP: u64 = 120;

/// Stamp a buff carrying `flags` straight onto the player — the state the
/// aggro gate reads, without needing a real cast.
fn give_flag_buff(world: &mut World, oid: i32, skill_id: i32, flags: u32) {
    world
        .objects
        .get_component_mut::<Buffs>(&oid)
        .unwrap()
        .0
        .push(crate::model::skill::ActiveBuff {
            displayed: true,
            skill_id,
            skill_level: 1,
            abnormal_type_client_id: 0,
            abnormal_type: "NONE".to_string(),
            abnormal_level: 0,
            slot: crate::model::skill::BuffSlot::Buff,
            expires_at_tick: u64::MAX,
            passive: false,
            effect_flags: flags,
            blocked_abnormals: Vec::new(),
            abnormal_visuals: Vec::new(),
            effects: Vec::new(),
        });
}

// ---------------------------------------------------------------------------
// The aggro gate
// ---------------------------------------------------------------------------

/// The baseline the rest of the file is measured against: an ordinary player
/// standing in front of an aggressive monster *is* noticed.
#[test]
fn an_unhidden_player_is_noticed() {
    let (mut world, _db, _l) = stealth_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 20, 300, 0, 0);

    advance_world(&mut world, AGGRO_WARMUP);
    assert!(
        hate_on(&world, MOB_OID, PLAYER) > 0.0,
        "sanity: the scan works at this range"
    );
}

/// `SILENT_MOVE`: the same player, same spot, now stealthed — the monster
/// never seeds hate. This is the whole point of Silent Move 221 / Stealth 411.
#[test]
fn a_silent_moving_player_walks_past_an_aggressive_monster() {
    let (mut world, _db, _l) = stealth_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 20, 300, 0, 0);
    give_flag_buff(&mut world, PLAYER, 221, effect_flag::SILENT_MOVE);

    advance_world(&mut world, AGGRO_WARMUP);
    assert_eq!(
        hate_on(&world, MOB_OID, PLAYER),
        0.0,
        "stealth hides the player from the scan"
    );
}

/// **Raid bosses see through stealth** (`!me.isRaid()` in Java's condition) —
/// the exemption that stops Stealth from trivialising raids.
#[test]
fn a_raid_boss_sees_through_silent_move() {
    let (mut world, _db, _l) = stealth_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, RAID_ID, "RaidBoss", 20, 300, 0, 0);
    give_flag_buff(&mut world, PLAYER, 221, effect_flag::SILENT_MOVE);

    advance_world(&mut world, AGGRO_WARMUP);
    assert!(
        hate_on(&world, MOB_OID, PLAYER) > 0.0,
        "a raid boss is not fooled by stealth"
    );
}

/// `FAKE_DEATH` folds into `isAlikeDead()`, the *first* check in
/// `isAggressiveTowards` — a fake-dead player is a corpse as far as aggro is
/// concerned, and a raid boss is **not** exempt from that (unlike stealth).
#[test]
fn a_fake_dead_player_is_ignored_even_by_a_raid_boss() {
    for npc_id in [MOB_ID, RAID_ID] {
        let (mut world, _db, _l) = stealth_world();
        let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
        add_test_npc(&mut world, MOB_OID, npc_id, "Monster", 20, 300, 0, 0);
        give_flag_buff(&mut world, PLAYER, 60, effect_flag::FAKE_DEATH);

        advance_world(&mut world, AGGRO_WARMUP);
        assert_eq!(
            hate_on(&world, MOB_OID, PLAYER),
            0.0,
            "npc {npc_id} ignores a fake-dead player"
        );
    }
}

/// Guards run the same `isAggressiveTowards` (Java `Guard extends
/// Attackable`), so a stealthed PK slips past them too — the guard-specific
/// scan needed the gate as well, not just the generic monster one.
#[test]
fn a_stealthed_pk_slips_past_a_guard() {
    let (mut world, _db, _l) = stealth_world();
    world
        .data
        .npc_data
        .insert_for_test(aggressive_template(45002, "Guard"));
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, 45002, "Guard", 20, 300, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&PLAYER)
        .unwrap()
        .reputation = -500;

    // Unhidden first: the guard must actually want this PK.
    advance_world(&mut world, AGGRO_WARMUP);
    assert!(
        hate_on(&world, MOB_OID, PLAYER) > 0.0,
        "sanity: the guard hunts this PK"
    );

    // Now the same setup with stealth up.
    let (mut world, _db, _l) = stealth_world();
    world
        .data
        .npc_data
        .insert_for_test(aggressive_template(45002, "Guard"));
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, 45002, "Guard", 20, 300, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&PLAYER)
        .unwrap()
        .reputation = -500;
    give_flag_buff(&mut world, PLAYER, 411, effect_flag::SILENT_MOVE);

    advance_world(&mut world, AGGRO_WARMUP);
    assert_eq!(
        hate_on(&world, MOB_OID, PLAYER),
        0.0,
        "stealth hides a PK from a guard too"
    );
}

// ---------------------------------------------------------------------------
// Fake death: getting up
// ---------------------------------------------------------------------------

/// `Creature.reduceCurrentHp`'s fake-death branch (`FakeDeathDamageStand =
/// True` on this dist): taking a hit ends the act outright — effect removed,
/// not just the pose — so a rogue can't soak a fight from the floor.
#[test]
fn damage_breaks_fake_death() {
    let (mut world, _db, _l) = stealth_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 20, 300, 0, 0);
    give_flag_buff(&mut world, PLAYER, 60, effect_flag::FAKE_DEATH);
    assert_ne!(
        crate::game_loop::abnormal::flags_of(&world, PLAYER) & effect_flag::FAKE_DEATH,
        0,
        "playing dead to begin with"
    );

    crate::game_loop::combat::apply_physical_damage(
        &mut world, MOB_OID, PLAYER, 10.0, false, false,
    );

    assert_eq!(
        crate::game_loop::abnormal::flags_of(&world, PLAYER) & effect_flag::FAKE_DEATH,
        0,
        "one hit and the act is over"
    );
}

/// A zero-damage event must *not* stand the player up (Java gates on
/// `amount > 0`) — otherwise a missed swing would break fake death.
#[test]
fn a_zero_damage_hit_does_not_break_fake_death() {
    let (mut world, _db, _l) = stealth_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 20, 300, 0, 0);
    give_flag_buff(&mut world, PLAYER, 60, effect_flag::FAKE_DEATH);

    crate::game_loop::combat::apply_physical_damage(&mut world, MOB_OID, PLAYER, 0.0, false, false);

    assert_ne!(
        crate::game_loop::abnormal::flags_of(&world, PLAYER) & effect_flag::FAKE_DEATH,
        0,
        "a 0-damage event leaves the act running"
    );
}

/// Standing up sends the client `ChangeWaitType(WT_STOP_FAKEDEATH)` **and**
/// `Revive` — Java sends both, the second working around a client quirk.
#[test]
fn standing_up_sends_change_wait_type_and_revive() {
    let (mut world, _db, _l) = stealth_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    give_flag_buff(&mut world, PLAYER, 60, effect_flag::FAKE_DEATH);
    let _ = drain(&mut out);

    crate::game_loop::skills::effects::handle_buff_expire(&mut world, PLAYER, 60);

    let opcodes: Vec<u8> = drain(&mut out)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();
    assert!(
        opcodes.contains(&server_packets::opcodes::CHANGE_WAIT_TYPE),
        "ChangeWaitType is broadcast, got {opcodes:?}"
    );
    assert!(
        opcodes.contains(&server_packets::opcodes::REVIVE),
        "and Revive alongside it, got {opcodes:?}"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// Fake Death 60 carries **only** these two effects, so with both unported it
/// parsed to an empty effect list and the whole skill was dropped — it cast and
/// did nothing whatsoever. Its MP upkeep is the same `power`/`ticks` shape as
/// `ManaDamOverTime` (10 MP per tick at 5 ticks ≈ the datapack comment's
/// "10 MP per second").
#[test]
fn real_dist_fake_death_parses_both_halves() {
    let skills = dist::skills();
    let skill = skills.get(60, 1).expect("Fake Death loads");

    assert!(
        skill.effects.iter().any(|e| matches!(e, SkillEffect::FakeDeath { power, ticks } if *power == 10.0 && *ticks == 5)),
        "FakeDeath with the real power/ticks: {:?}",
        skill.effects
    );
    assert!(
        skill
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::SilentMove)),
        "and its SilentMove half"
    );
    let flags = skill.effect_flags();
    assert_ne!(flags & effect_flag::FAKE_DEATH, 0);
    assert_ne!(flags & effect_flag::SILENT_MOVE, 0);
}

/// The other three learnable `SilentMove` skills all contribute the flag.
/// These *did* land before (they carry other, ported effects) — the stealth
/// simply never happened, which is a quieter failure than Fake Death's.
#[test]
fn real_dist_silent_move_skills_contribute_the_flag() {
    let skills = dist::skills();
    // Silent Move 221, Dance of Shadows 366, Stealth 411.
    for id in [221, 366, 411] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::SilentMove)),
            "skill {id} carries SilentMove: {:?}",
            skill.effects
        );
        assert_ne!(
            skill.effect_flags() & effect_flag::SILENT_MOVE,
            0,
            "skill {id} contributes the flag"
        );
        // The pre-existing effects must survive the new arm.
        assert!(
            skill.effects.len() > 1,
            "skill {id} keeps its other effects too"
        );
    }
}
