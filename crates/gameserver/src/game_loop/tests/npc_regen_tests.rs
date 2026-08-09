//! NPC HP/MP regeneration (G21 slice 6) — `CreatureStatus.doRegeneration` for
//! the non-player half.

use super::*;

use crate::model::components::Vitals;

const PLAYER: i32 = 2001;
const CID: u32 = 1;
const MOB_ID: i32 = 43000;
const RAID_ID: i32 = 43001;

fn regen_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    for (id, type_name) in [(MOB_ID, "Monster"), (RAID_ID, "RaidBoss")] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = type_name.into();
        t.name = format!("Regen {id}");
        t.level = 20;
        t.base_hp_max = 1000.0;
        t.base_mp_max = 500.0;
        // 8.5 is the most common hpRegen in the datapack (5467 templates).
        t.base_hp_reg = 8.5;
        t.base_mp_reg = 2.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db, l)
}

fn place(world: &mut World, npc_id: i32, cur_hp: f64) -> i32 {
    add_test_npc(world, NPC_OID, npc_id, "Monster", 20, 100, 0, 0);
    let v = world.objects.get_component_mut::<Vitals>(&NPC_OID).unwrap();
    // `add_test_npc` hard-codes a 100/50 pool; raise it *before* setting the
    // current values, or `cur_hp` lands above `max_hp` and the regen tick
    // (rightly) treats the mob as already full.
    v.max_hp = 1000;
    v.max_mp = 500;
    v.cur_hp = cur_hp;
    v.cur_mp = 500.0;
    NPC_OID
}

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}

fn mp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_mp
}

/// Run the NPC regen tick `n` times.
fn regen(world: &mut World, n: usize) {
    for _ in 0..n {
        crate::game_loop::regen::run_npc_regen_tick(world);
    }
}

// ---------------------------------------------------------------------------

#[test]
fn a_wounded_mob_heals_at_its_template_rate() {
    let (mut world, _db, _l) = regen_world();
    let oid = place(&mut world, MOB_ID, 500.0);

    regen(&mut world, 1);

    // NPC regen is the bare template value — no levelMod, no CON bonus, no
    // standing multiplier (all of those live inside Java's isPlayer() branch).
    assert_eq!(
        hp(&world, oid),
        508.5,
        "one tick of the template's 8.5 hpRegen"
    );
}

#[test]
fn regen_stops_at_full_hp() {
    let (mut world, _db, _l) = regen_world();
    let oid = place(&mut world, MOB_ID, 995.0);
    let max = world.objects.get_component::<Vitals>(&oid).unwrap().max_hp as f64;

    regen(&mut world, 5);

    assert_eq!(hp(&world, oid), max, "clamped at max, never above");
}

#[test]
fn a_dead_mob_does_not_regenerate() {
    let (mut world, _db, _l) = regen_world();
    let oid = place(&mut world, MOB_ID, 0.0);
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .dead = true;

    regen(&mut world, 5);

    assert_eq!(hp(&world, oid), 0.0, "a corpse stays at 0");
}

#[test]
fn mp_regenerates_too() {
    let (mut world, _db, _l) = regen_world();
    let oid = place(&mut world, MOB_ID, 500.0);
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .cur_mp = 100.0;

    regen(&mut world, 1);

    assert_eq!(
        mp(&world, oid),
        102.0,
        "one tick of the template's 2.0 mpRegen"
    );
}

#[test]
fn a_raid_boss_uses_the_raid_multiplier() {
    let (mut world, _db, _l) = regen_world();
    world.cfg.npc.raid_hp_regen_multiplier = 2.0; // dist ships 1.0; exercise the branch
    world.cfg.npc.hp_regen_multiplier = 1.0;
    let oid = place(&mut world, RAID_ID, 500.0);

    regen(&mut world, 1);

    assert_eq!(
        hp(&world, oid),
        517.0,
        "8.5 x the raid multiplier, not the ordinary one"
    );
}

#[test]
fn an_ordinary_mob_ignores_the_raid_multiplier() {
    let (mut world, _db, _l) = regen_world();
    world.cfg.npc.raid_hp_regen_multiplier = 2.0;
    world.cfg.npc.hp_regen_multiplier = 1.0;
    let oid = place(&mut world, MOB_ID, 500.0);

    regen(&mut world, 1);

    assert_eq!(
        hp(&world, oid),
        508.5,
        "a Monster uses the plain multiplier"
    );
}

#[test]
fn regen_continues_during_combat() {
    // Java's regen task never checks an in-combat flag — it only stops when
    // dead or full. A high-regen boss therefore makes a long fight a DPS race,
    // which is the intended retail behaviour, not an oversight.
    let (mut world, _db, _l) = regen_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let oid = place(&mut world, MOB_ID, 500.0);
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&oid)
        .unwrap()
        .0
        .insert(
            PLAYER,
            crate::model::npc::AggroInfo {
                hate: 100.0,
                damage: 100.0,
            },
        );
    if let Some(ai) = world
        .objects
        .get_component_mut::<crate::model::npc::NpcAi>(&oid)
    {
        ai.intention = crate::model::npc::NpcIntention::Attack;
    }

    regen(&mut world, 1);

    assert_eq!(hp(&world, oid), 508.5, "an engaged mob still regenerates");
}

#[test]
fn healing_broadcasts_the_hp_bar() {
    let (mut world, _db, _l) = regen_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let _oid = place(&mut world, MOB_ID, 500.0);
    let _ = drain(&mut out);

    regen(&mut world, 1);

    let packets = drain(&mut out);
    assert!(
        packets
            .iter()
            .any(|p| p.first() == Some(&crate::network::server_packets::opcodes::STATUS_UPDATE)),
        "nearby clients need the refreshed HP bar"
    );
}

#[test]
fn a_full_hp_mob_broadcasts_nothing() {
    // Otherwise every full-HP NPC in the world would emit a StatusUpdate every
    // 3 s — tens of thousands of packets for no visible change.
    let (mut world, _db, _l) = regen_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let oid = place(&mut world, MOB_ID, 500.0);
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .cur_hp = world.objects.get_component::<Vitals>(&oid).unwrap().max_hp as f64;
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .cur_mp = world.objects.get_component::<Vitals>(&oid).unwrap().max_mp as f64;
    let _ = drain(&mut out);

    regen(&mut world, 3);

    let packets = drain(&mut out);
    assert!(
        !packets
            .iter()
            .any(|p| p.first() == Some(&crate::network::server_packets::opcodes::STATUS_UPDATE)),
        "a mob at full HP/MP must be skipped entirely"
    );
}

/// The regen values come from the datapack, so check a real one.
#[test]
fn real_dist_templates_carry_regen() {
    let data = crate::data::NpcData::load_from(crate::data::DIST_GAME);
    let with_regen = data.all().filter(|t| t.base_hp_reg > 0.0).count();
    assert!(
        with_regen > 10_000,
        "expected >10k templates with a positive hpRegen, got {with_regen}"
    );
}
