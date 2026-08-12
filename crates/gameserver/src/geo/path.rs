//! Port of `geoengine/pathfinding/cellnodes` (`CellPathFinding` +
//! `CellNodeBuffer`/`CellNode`/`NodeLoc`) — best-first search over geo cells
//! with the cost-sorted-chain open list and the LOS postfilter, as a pure
//! function over `&GeoEngine`. Java's cross-thread buffer pool
//! (`PathFindBuffers`, locking, temp-buffer overflow) is collapsed to "pick
//! the smallest configured buffer size that fits, allocate fresh" — the
//! search runs on the single path-worker thread (`geo::worker`), so pooling
//! buys nothing; only the size ceiling (a too-far request finds no buffer and
//! fails, same as Java) is preserved. Debug-item drops and the stats counters
//! are not ported.

use super::GeoEngine;
use super::{NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};

/// `CellNodeBuffer.MAX_ITERATIONS`.
const MAX_ITERATIONS: usize = 3500;

/// The pathfinding tuning block of `GeoEngine.ini` (`Config.java` geoengine
/// section). Defaults match Java's.
#[derive(Debug, Clone)]
pub struct PathConfig {
    /// Buffer map sizes parsed from `PathFindBuffers` (counts dropped —
    /// see the module note), ascending.
    pub buffer_sizes: Vec<i32>,
    /// `LowWeight`: cost bias of a plain cardinal step.
    pub low_weight: f64,
    /// `MediumWeight`: step next to an obstacle.
    pub medium_weight: f64,
    /// `HighWeight`: step onto a constrained cell (missing exits / z jump).
    pub high_weight: f64,
    /// `AdvancedDiagonalStrategy`: expand diagonal neighbours too.
    pub advanced_diagonal_strategy: bool,
    /// `DiagonalWeight`: cost bias of a diagonal step.
    pub diagonal_weight: f64,
    /// `MaxPostfilterPasses`: 0 disables the LOS postfilter.
    pub max_postfilter_passes: i32,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            buffer_sizes: parse_buffer_sizes("100x6;128x6;192x6;256x4;320x4;384x4;500x2"),
            low_weight: 0.5,
            medium_weight: 2.0,
            high_weight: 3.0,
            advanced_diagonal_strategy: true,
            diagonal_weight: 0.707,
            max_postfilter_passes: 3,
        }
    }
}

/// Parse the `PathFindBuffers` value ("100x6;128x6;…") into the map sizes,
/// sorted ascending. Malformed entries are skipped.
pub fn parse_buffer_sizes(spec: &str) -> Vec<i32> {
    let mut sizes: Vec<i32> = spec
        .split(';')
        .filter_map(|buf| buf.split('x').next()?.trim().parse().ok())
        .collect();
    sizes.sort_unstable();
    sizes
}

/// `NodeLoc`: a geo cell with its per-direction passability and nearest
/// height captured at node-creation z.
#[derive(Debug, Clone, Copy)]
struct NodeLoc {
    geo_x: i32,
    geo_y: i32,
    nswe: u8,
    geo_height: i32,
}

impl NodeLoc {
    fn new(geo: &GeoEngine, x: i32, y: i32, z: i32) -> Self {
        let mut nswe = 0u8;
        for dir in [NSWE_NORTH, NSWE_EAST, NSWE_SOUTH, NSWE_WEST] {
            if geo.check_nearest_nswe(x, y, z, dir) {
                nswe |= dir;
            }
        }
        Self {
            geo_x: x,
            geo_y: y,
            nswe,
            geo_height: geo.get_nearest_z(x, y, z),
        }
    }

    fn can_go(&self, dir: u8) -> bool {
        (self.nswe & dir) != 0
    }

    fn can_go_none(&self) -> bool {
        self.nswe == 0
    }

    fn can_go_all(&self) -> bool {
        self.nswe == (NSWE_NORTH | NSWE_EAST | NSWE_SOUTH | NSWE_WEST)
    }
}

/// `CellNode`, arena-allocated: `parent` is an index into
/// `CellNodeBuffer.nodes` (-1 = none) instead of object references.
#[derive(Debug, Clone, Copy)]
struct CellNode {
    loc: NodeLoc,
    parent: i32,
    cost: f32,
}

/// `CellNodeBuffer`: a `map_size`² window of cells centred on the from→to
/// midpoint, holding the search state for one `find_path` call.
struct CellNodeBuffer<'a> {
    geo: &'a GeoEngine,
    cfg: &'a PathConfig,
    map_size: i32,
    /// (ax * map_size + ay) → node index + 1; 0 = no node yet.
    grid: Vec<u32>,
    nodes: Vec<CellNode>,
    base_x: i32,
    base_y: i32,
    target_x: i32,
    target_y: i32,
    target_z: i32,
    current: usize,
    /// The open set, cheapest-first.
    ///
    /// Java keeps it as a cost-sorted **linked list** and walks it from the
    /// current node on every insert, which is O(open set) per node and O(n^2)
    /// over a search: profiling one 1000-unit Giran route showed 160,424 chain
    /// steps to place 2,311 nodes, and that walk was ~60% of the total time.
    /// A binary heap answers the same question in O(log n).
    ///
    /// This is not a behaviour change. The chain was already kept globally
    /// sorted, so `current.next` was always the cheapest unvisited node — the
    /// same node this pops — and ties break by insertion order in both (the
    /// chain walks past equal costs and appends; `HeapEntry` orders equal costs
    /// by ascending index). Verified by diffing the produced routes.
    open: std::collections::BinaryHeap<HeapEntry>,
}

/// Open-set entry ordered cheapest-first, ties by insertion order.
///
/// `BinaryHeap` is a max-heap, so both comparisons are reversed.
#[derive(PartialEq)]
struct HeapEntry {
    cost: f32,
    idx: usize,
}

impl Eq for HeapEntry {}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.idx.cmp(&self.idx))
    }
}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> CellNodeBuffer<'a> {
    fn new(geo: &'a GeoEngine, cfg: &'a PathConfig, map_size: i32) -> Self {
        Self {
            geo,
            cfg,
            map_size,
            grid: vec![0; (map_size * map_size) as usize],
            nodes: Vec::new(),
            base_x: 0,
            base_y: 0,
            target_x: 0,
            target_y: 0,
            target_z: 0,
            current: 0,
            open: std::collections::BinaryHeap::new(),
        }
    }

    /// `CellNodeBuffer.findPath` — returns the arena index of the target node.
    fn find_path(&mut self, x: i32, y: i32, z: i32, tx: i32, ty: i32, tz: i32) -> Option<usize> {
        // Middle of the line (x,y)-(tx,ty) lands in the center of the buffer.
        self.base_x = x + ((tx - x - self.map_size) / 2);
        self.base_y = y + ((ty - y - self.map_size) / 2);
        self.target_x = tx;
        self.target_y = ty;
        self.target_z = tz;
        self.current = self.get_node(x, y, z)?;
        let start_cost = self.cost(x, y, z, self.cfg.high_weight);
        self.nodes[self.current].cost = start_cost;

        for _ in 0..MAX_ITERATIONS {
            let loc = self.nodes[self.current].loc;
            if loc.geo_x == self.target_x
                && loc.geo_y == self.target_y
                && (loc.geo_height - self.target_z).abs() < 64
            {
                return Some(self.current); // Found.
            }

            self.get_neighbors();
            match self.open.pop() {
                None => return None, // No more ways.
                Some(e) => self.current = e.idx,
            }
        }
        None
    }

    /// `CellNodeBuffer.getNeighbors`.
    fn get_neighbors(&mut self) {
        let loc = self.nodes[self.current].loc;
        if loc.can_go_none() {
            return;
        }
        let (x, y, z) = (loc.geo_x, loc.geo_y, loc.geo_height);

        let node_e = if loc.can_go(NSWE_EAST) {
            self.add_node(x + 1, y, z, false)
        } else {
            None
        };
        let node_s = if loc.can_go(NSWE_SOUTH) {
            self.add_node(x, y + 1, z, false)
        } else {
            None
        };
        let node_w = if loc.can_go(NSWE_WEST) {
            self.add_node(x - 1, y, z, false)
        } else {
            None
        };
        let node_n = if loc.can_go(NSWE_NORTH) {
            self.add_node(x, y - 1, z, false)
        } else {
            None
        };

        if !self.cfg.advanced_diagonal_strategy {
            return;
        }
        // Snapshot the cardinal nodes' exits (fixed at node creation) so the
        // diagonal inserts below can borrow `self` mutably.
        let locs: Vec<Option<NodeLoc>> = [node_e, node_s, node_w, node_n]
            .iter()
            .map(|n| n.map(|i| self.nodes[i].loc))
            .collect();
        let can = |n: Option<NodeLoc>, dir: u8| n.is_some_and(|loc| loc.can_go(dir));
        let (node_e, node_s, node_w, node_n) = (locs[0], locs[1], locs[2], locs[3]);

        if can(node_e, NSWE_SOUTH) && can(node_s, NSWE_EAST) {
            self.add_node(x + 1, y + 1, z, true);
        }
        if can(node_w, NSWE_SOUTH) && can(node_s, NSWE_WEST) {
            self.add_node(x - 1, y + 1, z, true);
        }
        if can(node_e, NSWE_NORTH) && can(node_n, NSWE_EAST) {
            self.add_node(x + 1, y - 1, z, true);
        }
        if can(node_w, NSWE_NORTH) && can(node_n, NSWE_WEST) {
            self.add_node(x - 1, y - 1, z, true);
        }
    }

    /// `CellNodeBuffer.getNode`: cell → arena node, created on first visit.
    /// Nodes are keyed by (x, y) only — a revisit at a different z reuses the
    /// existing node, same as Java.
    fn get_node(&mut self, x: i32, y: i32, z: i32) -> Option<usize> {
        let ax = x - self.base_x;
        if ax < 0 || ax >= self.map_size {
            return None;
        }
        let ay = y - self.base_y;
        if ay < 0 || ay >= self.map_size {
            return None;
        }
        let slot = (ax * self.map_size + ay) as usize;
        match self.grid[slot] {
            0 => {
                let idx = self.nodes.len();
                self.nodes.push(CellNode {
                    loc: NodeLoc::new(self.geo, x, y, z),
                    parent: -1,
                    cost: -1000.0,
                });
                self.grid[slot] = (idx + 1) as u32;
                Some(idx)
            }
            n => Some((n - 1) as usize),
        }
    }

    /// `CellNodeBuffer.addNode`: weigh a neighbour and insert it into the
    /// open set (skipped if already weighed — cost ≥ 0).
    fn add_node(&mut self, x: i32, y: i32, z: i32, diagonal: bool) -> Option<usize> {
        let new_idx = self.get_node(x, y, z)?;
        if self.nodes[new_idx].cost >= 0.0 {
            return Some(new_idx);
        }

        let geo_z = self.nodes[new_idx].loc.geo_height;
        let step_z = (geo_z - self.nodes[self.current].loc.geo_height).abs();
        let mut weight = if diagonal {
            self.cfg.diagonal_weight
        } else {
            self.cfg.low_weight
        };

        if !self.nodes[new_idx].loc.can_go_all() || step_z > 16 {
            weight = self.cfg.high_weight;
        } else if self.is_high_weight(x + 1, y, geo_z)
            || self.is_high_weight(x - 1, y, geo_z)
            || self.is_high_weight(x, y + 1, geo_z)
            || self.is_high_weight(x, y - 1, geo_z)
        {
            weight = self.cfg.medium_weight;
        }

        self.nodes[new_idx].parent = self.current as i32;
        let new_cost = self.cost(x, y, geo_z, weight);
        self.nodes[new_idx].cost = new_cost;

        self.open.push(HeapEntry {
            cost: new_cost,
            idx: new_idx,
        });

        Some(new_idx)
    }

    fn is_high_weight(&mut self, x: i32, y: i32, z: i32) -> bool {
        match self.get_node(x, y, z) {
            None => true,
            Some(idx) => {
                let loc = self.nodes[idx].loc;
                !loc.can_go_all() || (loc.geo_height - z).abs() > 16
            }
        }
    }

    /// `CellNodeBuffer.getCost` — distance to target plus the step weight
    /// (only when farther than the weight itself, per the Java quirk).
    fn cost(&self, x: i32, y: i32, z: i32, weight: f64) -> f32 {
        let dx = (x - self.target_x) as f64;
        let dy = (y - self.target_y) as f64;
        let dz = (z - self.target_z) as f64;
        let mut result = (dx * dx + dy * dy + (dz * dz) / 256.0).sqrt();
        if result > weight {
            result += weight;
        }
        result.min(f32::MAX as f64) as f32
    }

    /// `CellPathFinding.constructPath`: walk the parent chain from the target
    /// back to the start, keeping only the points where the moving direction
    /// changes, in start→target order as (geo_x, geo_y, geo_height).
    fn construct_path(&self, target: usize) -> Vec<(i32, i32, i32)> {
        let mut path = Vec::new();
        let mut previous_direction = (i32::MIN, i32::MIN);
        let mut temp = target;
        while self.nodes[temp].parent != -1 {
            let parent = self.nodes[temp].parent as usize;
            // The step away from the parent — and, under the plain strategy,
            // the step away from the *grandparent* instead when those two hops
            // form one clean diagonal, which is how Java collapses a staircase
            // into a single route point.
            let mut direction = (
                self.nodes[temp].loc.geo_x - self.nodes[parent].loc.geo_x,
                self.nodes[temp].loc.geo_y - self.nodes[parent].loc.geo_y,
            );
            if !self.cfg.advanced_diagonal_strategy && self.nodes[parent].parent != -1 {
                let grandparent = self.nodes[parent].parent as usize;
                let tmp_x = self.nodes[temp].loc.geo_x - self.nodes[grandparent].loc.geo_x;
                let tmp_y = self.nodes[temp].loc.geo_y - self.nodes[grandparent].loc.geo_y;
                if tmp_x.abs() == tmp_y.abs() {
                    direction = (tmp_x, tmp_y);
                }
            }

            // Only add a new route point if the moving direction changes.
            if direction != previous_direction {
                previous_direction = direction;
                let loc = self.nodes[temp].loc;
                path.push((loc.geo_x, loc.geo_y, loc.geo_height));
            }

            temp = parent;
        }
        path.reverse();
        path
    }
}

/// `CellPathFinding.findPath`: the full search — buffer sizing, cell search,
/// path construction, LOS postfilter — returning route points in world
/// coordinates (`NodeLoc.getX()/getY()/getZ()`), or `None` when either end
/// has no geodata, the target is farther than the largest buffer, or the
/// search exhausts. The `instance` parameter is dropped (no instances yet);
/// `playable` keeps its Java meaning (players get multiple postfilter
/// passes, AI gets one).
pub fn find_path(
    geo: &GeoEngine,
    cfg: &PathConfig,
    (x, y, z): (i32, i32, i32),
    (tx, ty, tz): (i32, i32, i32),
    playable: bool,
) -> Option<Vec<(i32, i32, i32)>> {
    let gx = geo.get_geo_x(x);
    let gy = geo.get_geo_y(y);
    if !geo.has_geo(x, y) {
        return None;
    }
    let gz = geo.get_height(x, y, z);
    let gtx = geo.get_geo_x(tx);
    let gty = geo.get_geo_y(ty);
    if !geo.has_geo(tx, ty) {
        return None;
    }
    let gtz = geo.get_height(tx, ty, tz);

    // Java `alloc`: smallest configured buffer that fits, none ⇒ no path.
    let size = 64 + 2 * (gx - gtx).abs().max((gy - gty).abs());
    let map_size = cfg.buffer_sizes.iter().copied().find(|&s| s >= size)?;

    let mut buffer = CellNodeBuffer::new(geo, cfg, map_size);
    let result = buffer.find_path(gx, gy, gz, gtx, gty, gtz)?;
    let mut path: Vec<(i32, i32, i32)> = buffer
        .construct_path(result)
        .into_iter()
        .map(|(nx, ny, nz)| (geo.get_world_x(nx), geo.get_world_y(ny), nz))
        .collect();

    if path.len() < 3 || cfg.max_postfilter_passes <= 0 {
        return Some(path);
    }

    // LOS postfilter: drop every middle point the mover could skip by
    // walking straight to the point after it. AI gets a single pass.
    let mut pass = 0;
    loop {
        pass += 1;
        let mut removed = false;
        let (mut cur_x, mut cur_y, mut cur_z) = (x, y, z);
        let mut i = 0;
        while i + 1 < path.len() {
            let (end_x, end_y, end_z) = path[i + 1];
            if geo.can_move_to_target(cur_x, cur_y, cur_z, end_x, end_y, end_z) {
                path.remove(i);
                removed = true;
            } else {
                (cur_x, cur_y, cur_z) = path[i];
                i += 1;
            }
        }
        if !(playable && removed && path.len() > 2 && pass < cfg.max_postfilter_passes) {
            break;
        }
    }

    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::region::{REGION_CELLS_X, REGION_CELLS_Y};
    use crate::geo::{synthetic_region, wall_column, wall_column_with_gap};

    const BASE_GEO_X: i32 = 11 * REGION_CELLS_X;
    const BASE_GEO_Y: i32 = 10 * REGION_CELLS_Y;

    /// Flat ground with a north-south wall at local cell x == 10, open only
    /// at local y ∈ [1000, 1004) — the classic walk-around case. The scene
    /// sits ~1000 cells from the region edges so the search cannot skirt the
    /// wall through unloaded void (see [`wall_column_with_gap`]).
    fn walled_engine() -> GeoEngine {
        let mut engine = GeoEngine::empty();
        engine.set_region(11, 10, synthetic_region(wall_column_with_gap(1000..1004)));
        engine
    }

    fn world_at(g: &GeoEngine, cx: i32, cy: i32) -> (i32, i32) {
        (
            g.get_world_x(BASE_GEO_X + cx),
            g.get_world_y(BASE_GEO_Y + cy),
        )
    }

    #[test]
    fn path_walks_around_wall_through_gap() {
        let g = walled_engine();
        let cfg = PathConfig::default();
        let (x0, y0) = world_at(&g, 5, 995);
        let (x1, y1) = world_at(&g, 15, 995);
        // Sanity: the straight line is blocked.
        assert!(!g.can_move_to_target(x0, y0, 0, x1, y1, 0));

        let path = find_path(&g, &cfg, (x0, y0, 0), (x1, y1, 0), true).expect("path via the gap");
        assert!(!path.is_empty());
        // The route must dip south into the gap rows.
        let gap_y = g.get_world_y(BASE_GEO_Y + 1000);
        assert!(
            path.iter().any(|&(_, py, _)| py >= gap_y),
            "path {path:?} should pass through the gap at y >= {gap_y}"
        );
        // Every consecutive leg of the postfiltered path is walkable.
        let (mut cx, mut cy, mut cz) = (x0, y0, 0);
        for &(px, py, pz) in &path {
            assert!(
                g.can_move_to_target(cx, cy, cz, px, py, pz),
                "leg ({cx},{cy}) -> ({px},{py}) not walkable in {path:?}"
            );
            (cx, cy, cz) = (px, py, pz);
        }
        // And the last point is the destination cell.
        let (lx, ly, _) = *path.last().unwrap();
        assert_eq!((lx, ly), (x1, y1));
    }

    #[test]
    fn sealed_wall_yields_no_path() {
        let mut engine = GeoEngine::empty();
        engine.set_region(11, 10, synthetic_region(wall_column));
        let cfg = PathConfig::default();
        let (x0, y0) = world_at(&engine, 5, 1000);
        let (x1, y1) = world_at(&engine, 15, 1000);
        assert_eq!(
            find_path(&engine, &cfg, (x0, y0, 0), (x1, y1, 0), true),
            None
        );
    }

    #[test]
    fn no_geodata_yields_no_path() {
        let g = GeoEngine::empty();
        let cfg = PathConfig::default();
        assert_eq!(find_path(&g, &cfg, (0, 0, 0), (1000, 0, 0), true), None);
    }

    #[test]
    fn too_far_for_any_buffer_yields_no_path() {
        let g = walled_engine();
        let cfg = PathConfig::default();
        let (x0, y0) = world_at(&g, 5, 995);
        // Largest default buffer is 500 cells: (500 - 64) / 2 = 218 cells
        // ≈ 3488 world units; ask for well past that inside the region.
        let (x1, y1) = world_at(&g, 5 + 300, 995);
        assert_eq!(find_path(&g, &cfg, (x0, y0, 0), (x1, y1, 0), true), None);
    }

    /// Same convention as `geo::tests::loads_real_dist_geodata` — run the
    /// pathfinder over the real Giran geodata.
    #[test]
    fn finds_path_on_real_dist_geodata() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/data/geodata"
        ));
        let g = GeoEngine::load(path);
        if !g.has_geo(82698, 148638) {
            eprintln!("dist geodata not present; skipping");
            return;
        }
        let cfg = PathConfig::default();
        let z = g.get_height(82698, 148638, -3473);
        let tz = g.get_height(81622, 148672, -3464);
        let route = find_path(&g, &cfg, (82698, 148638, z), (81622, 148672, tz), true)
            .expect("path across Giran town square");
        assert!(!route.is_empty());
        let (lx, ly, _) = *route.last().unwrap();
        // Destination cell (within one 16-unit geo cell of the request).
        assert!((lx - 81622).abs() <= 16 && (ly - 148672).abs() <= 16);
    }
}
