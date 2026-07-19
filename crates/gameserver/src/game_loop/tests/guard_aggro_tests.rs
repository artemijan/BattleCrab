//! Town-guard PK aggro and faction help calls (G21 slice 2).

use super::*;

use crate::model::npc::{AggroInfo, AggroList, NpcAi, NpcIntention};

const PLAYER: i32 = 2001;
const CID: u32 = 1;
const GUARD_ID: i32 = 41100;
const ORC_A_ID: i32 = 41101;
const ORC_B_ID: i32 = 41102;
const LONER_ID: i32 = 41103;
const GUARD_OID: i32 = NPC_OID;
const MATE_OID: i32 = NPC_OID + 1;

/// A `Guard` template — note guards are flagged **passive** in the datapack,
/// which is exactly why the guard branch can't be gated on `is_aggressive`.
fn guard_template() -> crate::data::npc_data::NpcTemplate {
    let mut t = crate::data::npc_data::default_template(GUARD_ID);
    t.type_name = "Guard".into();
    t.name = "Town Guard".into();
    t.level = 20;
    t.base_hp_max = 500.0;
    t.is_aggressive = false;
    t.aggro_range = 0;
    t.collision_radius = 10.0;
    t
}

fn clan_template(id: i32, clans: &[&str]) -> crate::data::npc_data::NpcTemplate {
    let mut t = crate::data::npc_data::default_template(id);
    t.type_name = "Monster".into();
    t.name = format!("Clan Mob {id}");
    t.level = 10;
    t.base_hp_max = 200.0;
    t.clan_help_range = 300;
    t.collision_radius = 10.0;
    t.clans = clans.iter().map(|s| s.to_string()).collect();
    t
}

fn guard_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    world.data.npc_data.insert_for_test(guard_template());
    world.data.npc_data.insert_for_test(clan_template(ORC_A_ID, &["ORC"]));
    world.data.npc_data.insert_for_test(clan_template(ORC_B_ID, &["ORC"]));
    world.data.npc_data.insert_for_test(clan_template(LONER_ID, &["LIZARDMAN"]));
    (world, db, l)
}

fn hate_on(world: &World, npc: i32, target: i32) -> f64 {
    world
        .objects
        .get_component::<AggroList>(&npc)
        .and_then(|a| a.0.get(&target))
        .map(|i| i.hate)
        .unwrap_or(0.0)
}

fn set_reputation(world: &mut World, oid: i32, rep: i32) {
    world.objects.get_component_mut::<crate::model::Player>(&oid).unwrap().reputation = rep;
}

// ---------------------------------------------------------------------------
// Guard aggro.

#[test]
fn guard_aggros_a_pk_in_range() {
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, GUARD_OID, GUARD_ID, "Guard", 20, 300, 0, 0);
    set_reputation(&mut world, PLAYER, -500);

    advance_world(&mut world, 20);

    assert!(hate_on(&world, GUARD_OID, PLAYER) > 0.0, "a guard must aggro a PK standing 300 units away");
}

#[test]
fn guard_ignores_a_lawful_player_at_the_same_distance() {
    // The distinguishing case: same guard, same spot, clean reputation.
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, GUARD_OID, GUARD_ID, "Guard", 20, 300, 0, 0);
    set_reputation(&mut world, PLAYER, 0);

    advance_world(&mut world, 20);

    assert_eq!(hate_on(&world, GUARD_OID, PLAYER), 0.0, "a lawful player walks past a guard untouched");
}

#[test]
fn guard_ignores_a_pk_beyond_500_units() {
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    // 700 away: outside the hardcoded 500 guard range.
    add_test_npc(&mut world, GUARD_OID, GUARD_ID, "Guard", 20, 700, 0, 0);
    set_reputation(&mut world, PLAYER, -500);

    advance_world(&mut world, 20);

    assert_eq!(hate_on(&world, GUARD_OID, PLAYER), 0.0, "the guard range is 500, not the template aggroRange");
}

#[test]
fn a_plain_monster_does_not_inherit_the_pk_rule() {
    // Only `Guard` templates hunt PKs; a passive monster stays passive.
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, GUARD_OID, LONER_ID, "Monster", 10, 300, 0, 0);
    set_reputation(&mut world, PLAYER, -500);

    advance_world(&mut world, 20);

    assert_eq!(hate_on(&world, GUARD_OID, PLAYER), 0.0, "a non-Guard monster ignores reputation");
}

// ---------------------------------------------------------------------------
// Faction calls.

/// Put an engaged mob (`ORC_A`) next to a clan-mate and run one think.
/// `damage` seeds the aggro entry — the "was I actually attacked" gate.
fn engaged_pair(world: &mut World, mate_npc_id: i32, mate_x: i32, damage: f64) {
    add_test_npc(world, GUARD_OID, ORC_A_ID, "Monster", 10, 100, 0, 0);
    add_test_npc(world, MATE_OID, mate_npc_id, "Monster", 10, mate_x, 0, 0);
    world
        .objects
        .get_component_mut::<AggroList>(&GUARD_OID)
        .unwrap()
        .0
        .insert(PLAYER, AggroInfo { hate: 50.0, damage });
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&GUARD_OID) {
        ai.intention = NpcIntention::Attack;
        ai.global_aggro = 0;
        ai.attack_timeout_tick = u64::MAX;
    }
}

#[test]
fn attacked_mob_pulls_its_clan_mate_in() {
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engaged_pair(&mut world, ORC_B_ID, 200, 100.0);

    advance_world(&mut world, 20);

    assert!(hate_on(&world, MATE_OID, PLAYER) > 0.0, "an ORC clan-mate 100 units away should join the fight");
    assert_eq!(
        world.objects.get_component::<NpcAi>(&MATE_OID).unwrap().intention,
        NpcIntention::Attack,
        "the recruit switches to the attack loop"
    );
}

#[test]
fn a_different_faction_is_not_pulled_in() {
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engaged_pair(&mut world, LONER_ID, 200, 100.0);

    advance_world(&mut world, 20);

    assert_eq!(hate_on(&world, MATE_OID, PLAYER), 0.0, "LIZARDMAN doesn't answer an ORC's call");
}

#[test]
fn no_call_when_the_target_never_attacked_this_mob() {
    // Hate but zero damage — e.g. seeded by an aggro-range scan. Java checks
    // `getAttackByList`, so this must NOT pull the camp.
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engaged_pair(&mut world, ORC_B_ID, 200, 0.0);

    advance_world(&mut world, 20);

    assert_eq!(
        hate_on(&world, MATE_OID, PLAYER),
        0.0,
        "merely being noticed by a mob must not pull its whole faction"
    );
}

#[test]
fn clan_mate_beyond_the_help_range_is_not_pulled_in() {
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    // clanHelpRange 300 + collision 10 + 10 = 320; put the mate at 900.
    engaged_pair(&mut world, ORC_B_ID, 1000, 100.0);

    advance_world(&mut world, 20);

    assert_eq!(hate_on(&world, MATE_OID, PLAYER), 0.0, "out of clanHelpRange");
}

#[test]
fn an_ignored_npc_id_refuses_the_call() {
    let (mut world, _db, _l) = guard_world();
    {
        // ORC_B refuses calls from ORC_A specifically, despite sharing ORC.
        let mut t = world.data.npc_data.get(ORC_B_ID).unwrap().clone();
        t.ignore_clan_npc_ids.push(ORC_A_ID);
        world.data.npc_data.insert_for_test(t);
    }
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engaged_pair(&mut world, ORC_B_ID, 200, 100.0);

    advance_world(&mut world, 20);

    assert_eq!(hate_on(&world, MATE_OID, PLAYER), 0.0, "ignoreNpcId beats a shared clan");
}

#[test]
fn all_clan_matches_any_faction() {
    let (mut world, _db, _l) = guard_world();
    {
        let mut t = clan_template(ORC_B_ID, &["ALL"]);
        t.clan_help_range = 300;
        world.data.npc_data.insert_for_test(t);
    }
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engaged_pair(&mut world, ORC_B_ID, 200, 100.0);

    advance_world(&mut world, 20);

    assert!(hate_on(&world, MATE_OID, PLAYER) > 0.0, "an ALL-clan NPC answers any faction's call");
}

#[test]
fn a_clan_mate_already_fighting_is_left_alone() {
    // "Busy" has to be a *real* state: an Attack intention with no target
    // resolves straight back to Active on the next think, which would then
    // accept the call. So the mate is given its own live fight with the same
    // player, and the assertion is that the call adds no further hate on top.
    let (mut world, _db, _l) = guard_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    engaged_pair(&mut world, ORC_B_ID, 200, 100.0);
    world
        .objects
        .get_component_mut::<AggroList>(&MATE_OID)
        .unwrap()
        .0
        .insert(PLAYER, AggroInfo { hate: 5.0, damage: 10.0 });
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&MATE_OID) {
        ai.intention = NpcIntention::Attack;
        ai.attack_timeout_tick = u64::MAX;
    }

    advance_world(&mut world, 20);

    assert_eq!(
        hate_on(&world, MATE_OID, PLAYER),
        5.0,
        "a clan-mate already in the fight must not have the call's hate stacked on top"
    );
}
