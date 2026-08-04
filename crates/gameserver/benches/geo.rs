//! Geodata query benchmarks over the **real** `dist` geodata.
//!
//! These are the server's hottest CPU consumers away from the tick loop: LOS
//! and move validation run per attack, per cast and per aggro scan, and
//! pathfinding runs whenever a blocked move is handed to the path worker.
//! Giran town square is used because it is dense multilayer geodata, i.e. the
//! expensive branch rather than the flat-block fast path.
use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use gameserver::geo::GeoEngine;
use gameserver::geo::path::{PathConfig, find_path};

const DIST_GEO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/geodata");

/// Giran town square and a point ~1000 units west across it.
const AX: i32 = 82698;
const AY: i32 = 148638;
const BX: i32 = 81622;
const BY: i32 = 148672;

fn engine() -> Option<GeoEngine> {
    let g = GeoEngine::load(Path::new(DIST_GEO));
    g.has_geo(AX, AY).then_some(g)
}

fn bench(c: &mut Criterion) {
    let Some(g) = engine() else {
        eprintln!("dist geodata not present; skipping geo benches");
        return;
    };
    let az = g.get_height(AX, AY, -3473);
    let bz = g.get_height(BX, BY, -3464);

    // Single-cell primitives: what the walkers call per step.
    c.bench_function("get_height", |b| {
        b.iter(|| black_box(g.get_height(black_box(AX), black_box(AY), black_box(az))))
    });
    c.bench_function("get_nearest_z", |b| {
        let gx = g.get_geo_x(AX);
        let gy = g.get_geo_y(AY);
        b.iter(|| black_box(g.get_nearest_z(black_box(gx), black_box(gy), black_box(az))))
    });

    // Whole-line walks: the real call shape.
    c.bench_function("can_see_target_1000u", |b| {
        b.iter(|| {
            black_box(g.can_see_target(
                black_box(AX),
                black_box(AY),
                black_box(az),
                black_box(BX),
                black_box(BY),
                black_box(bz),
            ))
        })
    });
    c.bench_function("can_move_to_target_1000u", |b| {
        b.iter(|| {
            black_box(g.can_move_to_target(
                black_box(AX),
                black_box(AY),
                black_box(az),
                black_box(BX),
                black_box(BY),
                black_box(bz),
            ))
        })
    });

    // A sweep over many distinct cells, so the measurement is not one hot
    // cache line answering every query.
    c.bench_function("nearest_z_sweep_64x64", |b| {
        let gx0 = g.get_geo_x(AX);
        let gy0 = g.get_geo_y(AY);
        b.iter(|| {
            let mut acc = 0i64;
            for dx in 0..64 {
                for dy in 0..64 {
                    acc += g.get_nearest_z(gx0 + dx, gy0 + dy, az) as i64;
                }
            }
            black_box(acc)
        })
    });

    let cfg = PathConfig::default();
    c.bench_function("find_path_giran", |b| {
        b.iter(|| black_box(find_path(&g, &cfg, (AX, AY, az), (BX, BY, bz), true)))
    });

    // Same search with the LOS post-filter off, to price that stage.
    let mut cfg_nofilter = PathConfig::default();
    cfg_nofilter.max_postfilter_passes = 0;
    c.bench_function("find_path_giran_no_postfilter", |b| {
        b.iter(|| {
            black_box(find_path(
                &g,
                &cfg_nofilter,
                (AX, AY, az),
                (BX, BY, bz),
                true,
            ))
        })
    });

    // What one search pays just to get a zeroed node grid: the 256x256 buffer
    // this route allocates. Java pools these ("100x6;128x6;..." — the counts
    // are a pool size); this port allocates one per call.
    c.bench_function("grid_alloc_256x256", |b| {
        b.iter(|| black_box(vec![0u32; 256 * 256]))
    });
}

criterion_group!(geo, bench);
criterion_main!(geo);
