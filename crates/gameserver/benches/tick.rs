//! Tick-system benchmarks over the real `dist` world.
//!
//! One world is built once — full datapack, full geodata, all ~35k boot
//! spawns — then populated with 100 players in Giran town and 300 more
//! standing at monster spawn points, and each per-tick system is measured in
//! isolation. `world.tick` is left static so movers/AI don't drift across
//! iterations: each call measures the system's per-invocation cost at a
//! fixed world state, which is the number the game loop's 100 ms budget
//! actually spends.
//!
//! Run: `cargo bench -p gameserver --features bench-api --bench tick`
use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use gameserver::game_loop::bench_api as api;

/// Giran town square — dense non-monster NPC population.
const TOWN: (i32, i32, i32) = (82698, 148638, -3473);

const TOWN_PLAYERS: usize = 100;
const FIELD_PLAYERS: usize = 300;
/// Object ids far below `FIRST_NPC_OBJECT_ID` (0x4000_0000) and above any
/// DB-persisted id a test datapack would use.
const FIRST_OID: i32 = 1_000_000;

fn bench(c: &mut Criterion) {
    let mut bw = api::dist_world();

    // 100 town players in a 10×10 grid over the square.
    let mut cid: u32 = 1;
    let mut oid: i32 = FIRST_OID;
    for i in 0..TOWN_PLAYERS as i32 {
        bw.add_player(
            cid,
            oid,
            TOWN.0 + (i % 10) * 40,
            TOWN.1 + (i / 10) * 40,
            TOWN.2,
        );
        cid += 1;
        oid += 1;
    }
    let town_oid = FIRST_OID; // one representative town player

    // 300 field players, each at a (spread-out) monster spawn point — the
    // worst realistic case for the region-activity gate: many awake cells.
    let spots = api::monster_positions(&mut bw.world, FIELD_PLAYERS, 97);
    assert!(
        spots.len() >= FIELD_PLAYERS / 2,
        "dist spawn data present? got only {} monster spots",
        spots.len()
    );
    for (x, y, z) in spots {
        bw.add_player(cid, oid, x + 30, y + 30, z);
        cid += 1;
        oid += 1;
    }
    bw.drain();

    let mut g = c.benchmark_group("tick");
    g.sample_size(30).measurement_time(Duration::from_secs(8));

    // -- every-tick systems, idle world: nobody moving, nobody in combat --
    g.bench_function("movement_tick_idle_400p", |b| {
        b.iter(|| api::movement_tick(black_box(&mut bw.world)));
    });
    bw.drain();

    // -- 1 s cadence systems --
    g.bench_function("stance_pvp_ticks_400p", |b| {
        b.iter(|| {
            api::stance_tick(black_box(&mut bw.world));
            api::pvp_flag_tick(black_box(&mut bw.world));
        });
    });
    g.bench_function("effect_zone_ticks_400p", |b| {
        b.iter(|| api::effect_zone_ticks(black_box(&mut bw.world)));
    });
    g.bench_function("item_audit_drain_400p", |b| {
        b.iter(|| api::drain_item_audit(black_box(&mut bw.world)));
    });

    // -- 3 s cadence systems --
    g.bench_function("regen_tick_full_hp_400p", |b| {
        b.iter(|| api::regen_tick(black_box(&mut bw.world)));
    });
    g.bench_function("npc_regen_tick_35k_full", |b| {
        b.iter(|| api::npc_regen_tick(black_box(&mut bw.world)));
    });
    g.bench_function("weight_sweep_400p", |b| {
        b.iter(|| api::weight_sweep(black_box(&mut bw.world)));
    });
    bw.drain();

    // -- pieces of the movement/broadcast path, isolated --
    g.bench_function("revalidate_zone_town_player", |b| {
        b.iter(|| api::revalidate_zone(black_box(&mut bw.world), town_oid));
    });
    let pkt: Vec<u8> = vec![0x2Du8, 0, 0, 0, 0, 1, 0, 0, 0]; // SocialAction-sized
    g.bench_function("broadcast_town_100p", |b| {
        b.iter(|| api::broadcast_including_self(black_box(&bw.world), town_oid, &pkt));
        bw.drain();
    });
    // A region-boundary crossing back and forth: the full update_region
    // fan-out (player/NPC/door/static/item deltas + offline-trader scan).
    let (bx, by, bz) = (TOWN.0, TOWN.1, TOWN.2);
    let mut flip = false;
    g.bench_function("region_crossing_town", |b| {
        b.iter(|| {
            flip = !flip;
            let x = if flip { bx + 2400 } else { bx };
            api::relocate(black_box(&mut bw.world), town_oid, x, by, bz);
        });
    });
    bw.drain();

    // -- npc_ai think, town-only wake vs. hunting grounds wake --
    // (mutates AI state: aggro lists fill, mobs start chasing; keep last)
    g.bench_function("npc_ai_tick_400p_mixed", |b| {
        b.iter(|| api::npc_ai_tick(black_box(&mut bw.world)));
        bw.drain();
    });

    // -- movers: 50 town players walking (static tick ⇒ stable per-call) --
    for i in 0..50u32 {
        let o = FIRST_OID + i as i32;
        api::begin_move(
            &mut bw.world,
            1 + i,
            o,
            (TOWN.0 + 3000, TOWN.1 + 3000, TOWN.2),
        );
    }
    bw.drain();
    g.bench_function("movement_tick_50_movers_400p", |b| {
        b.iter(|| api::movement_tick(black_box(&mut bw.world)));
        bw.drain();
    });

    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
