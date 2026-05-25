//! HPA* (Hierarchical Pathfinding A*) for 100×100 to 1024×1024+ maps.
//!
//! Design choices to keep per-frame cost low (<1ms even on large maps):
//! - Flat A* within a cluster is fast (≤256 nodes).
//! - Single-source Dijkstra computes costs to ALL entrances at once — never
//!   run per-entrance A*.
//! - Consecutive border entrances are merged into one node.
//! - Cluster data is built lazily and cached; dirtied only on terrain change.
//! - Path requests are amortised: at most 5 per FixedUpdate tick.
//!
//! Cost function preserves desire-path emergence via PathMemory + RoadWear.

use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

use bevy::prelude::*;

use crate::generation::ElevationMap;
use crate::map::{Map, TileType};
use crate::player::{PathMemory, RoadWear, ROAD_THRESHOLD_3};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CLUSTER_SIZE: usize = 16;
const MAX_REQUESTS_PER_FRAME: usize = 5;
const ABSTRACT_NODE_LIMIT: usize = 4000;
const NO_PATH_TTL: u64 = 100;
const DIAG_COST: f32 = 1.41421356;

// ---------------------------------------------------------------------------
// TileKey
// ---------------------------------------------------------------------------

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
struct TileKey(u32);

impl TileKey {
    fn new(x: usize, y: usize) -> Self { Self(((y as u32) << 16) | (x as u32)) }
    fn x(self) -> usize { (self.0 & 0xFFFF) as usize }
    fn y(self) -> usize { (self.0 >> 16) as usize }
    fn tuple(self) -> (usize, usize) { (self.x(), self.y()) }
}

// ---------------------------------------------------------------------------
// A* node
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AStarNode {
    tile: (usize, usize),
    g: f32,
    f: f32,
}

impl PartialEq for AStarNode { fn eq(&self, o: &Self) -> bool { self.f == o.f } }
impl Eq for AStarNode {}
impl PartialOrd for AStarNode { fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { o.f.partial_cmp(&self.f) } }
impl Ord for AStarNode { fn cmp(&self, o: &Self) -> std::cmp::Ordering { o.f.total_cmp(&self.f) } }

// ---------------------------------------------------------------------------
// Cost function — mirrors tile_speed_multiplier in player.rs (inverted)
// ---------------------------------------------------------------------------

fn tile_cost(tile: (usize, usize), map: &Map, elevation: &[f64], path_memory: &PathMemory, road_wear: &[u32]) -> f32 {
    let w = map.width;
    let idx = tile.1 * w + tile.0;

    let base_speed: f32 = match map.tiles[idx] {
        TileType::Grass | TileType::Meadow | TileType::Dirt => 1.0,
        TileType::Sand | TileType::Clay => 0.7,
        TileType::Desert => 0.5,
        TileType::Tundra => 0.25,
        TileType::Forest | TileType::Ice | TileType::Snow | TileType::Stone => 0.20,
        TileType::Swamp => 0.15,
        TileType::Water | TileType::DeepWater | TileType::Lava => return f32::INFINITY,
    };

    let elev = elevation[idx] as f32;
    let elev_factor = 1.0 - (elev - 0.5).max(0.0) * 1.0;

    let mut max_diff = 0.0f32;
    let cx = tile.0 as isize;
    let cy = tile.1 as isize;
    for (dx, dy) in &[(0,1),(1,1),(1,0),(1,-1),(0,-1),(-1,-1),(-1,0),(-1,1)] {
        let nx = cx + dx;
        let ny = cy + dy;
        if nx >= 0 && nx < w as isize && ny >= 0 && ny < (elevation.len() / w) as isize {
            let diff = (elev - elevation[ny as usize * w + nx as usize] as f32).abs();
            if diff > max_diff { max_diff = diff; }
        }
    }
    let steep_factor = 1.0 - max_diff * 6.0;

    let wear = road_wear[idx];
    let road_bonus = 1.0 + (wear as f32 / ROAD_THRESHOLD_3 as f32).min(1.0) * 2.0;
    let mem = path_memory.counts[idx] as f32;
    let memory_bonus = 1.0 + (mem as f32).sqrt() * 2.0;

    let effective_speed = base_speed * road_bonus * elev_factor * steep_factor.max(0.02) * memory_bonus;
    1.0 / effective_speed.max(0.001)
}

fn edge_cost(from: (usize, usize), to: (usize, usize), map: &Map, elevation: &[f64], path_memory: &PathMemory, road_wear: &[u32]) -> f32 {
    let c = (tile_cost(from, map, elevation, path_memory, road_wear) + tile_cost(to, map, elevation, path_memory, road_wear)) / 2.0;
    if c.is_infinite() { return f32::INFINITY; }
    let is_diag = from.0 != to.0 && from.1 != to.1;
    if is_diag { c * DIAG_COST } else { c }
}

fn heuristic(a: (usize, usize), b: (usize, usize)) -> f32 {
    let dx = (a.0 as isize - b.0 as isize).unsigned_abs() as f32;
    let dy = (a.1 as isize - b.1 as isize).unsigned_abs() as f32;
    dx.min(dy) * DIAG_COST + (dx.max(dy) - dx.min(dy))
}

// ---------------------------------------------------------------------------
// 8-neighbour helper
// ---------------------------------------------------------------------------

fn neighbours(tile: (usize, usize), w: usize, h: usize) -> Vec<(usize, usize)> {
    let (x, y) = tile;
    let x = x as isize;
    let y = y as isize;
    let w = w as isize;
    let h = h as isize;
    let candidates: [(isize, isize); 8] = [
        (x, y+1), (x+1, y+1), (x+1, y), (x+1, y-1),
        (x, y-1), (x-1, y-1), (x-1, y), (x-1, y+1),
    ];
    let mut v = Vec::with_capacity(8);
    for (nx, ny) in &candidates {
        if *nx >= 0 && *nx < w && *ny >= 0 && *ny < h {
            v.push((*nx as usize, *ny as usize));
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Single-source Dijkstra — computes shortest-path costs from `start` to
// all tiles in `goals`.  Returns a Vec of (goal_idx, cost, came_from_map).
// Much more efficient than running A* per entrance.
// ---------------------------------------------------------------------------

fn dijkstra_to_goals(
    start: (usize, usize),
    goals: &[(usize, usize)],
    map: &Map,
    elevation: &[f64],
    path_memory: &PathMemory,
    road_wear: &[u32],
) -> HashMap<TileKey, (f32, TileKey)> {
    let w = map.width;
    let h = map.height;

    let goal_set: HashSet<TileKey> = goals.iter().map(|&g| TileKey::new(g.0, g.1)).collect();

    let mut dist: HashMap<TileKey, f32> = HashMap::new();
    let mut prev: HashMap<TileKey, (f32, TileKey)> = HashMap::new();
    let mut open = BinaryHeap::new();

    let sk = TileKey::new(start.0, start.1);
    dist.insert(sk, 0.0);
    open.push(AStarNode { tile: start, g: 0.0, f: 0.0 });

    let mut found = 0usize;
    let target_found = goal_set.len();

    while let Some(current) = open.pop() {
        let ck = TileKey::new(current.tile.0, current.tile.1);
        let cg = *dist.get(&ck).unwrap_or(&f32::INFINITY);
        if current.g > cg { continue; }

        if goal_set.contains(&ck) {
            prev.insert(ck, (cg, ck));
            found += 1;
            if found >= target_found { break; }
            continue;
        }

        for n in neighbours(current.tile, w, h).iter() {
            let cost = edge_cost(current.tile, *n, map, elevation, path_memory, road_wear);
            if cost.is_infinite() { continue; }
            let ng = cg + cost;
            let nk = TileKey::new(n.0, n.1);
            if ng < *dist.get(&nk).unwrap_or(&f32::INFINITY) {
                dist.insert(nk, ng);
                prev.insert(nk, (ng, ck));
                open.push(AStarNode { tile: *n, g: ng, f: ng });
            }
        }
    }

    prev
}

/// Reconstruct a path from start to goal using the came_from map from dijkstra.
fn reconstruct_path(start: (usize, usize), goal: (usize, usize), came_from: &HashMap<TileKey, (f32, TileKey)>) -> Option<Vec<(usize, usize)>> {
    let gk = TileKey::new(goal.0, goal.1);
    let sk = TileKey::new(start.0, start.1);
    if !came_from.contains_key(&gk) { return None; }
    if start == goal { return Some(vec![]); }

    let mut path = Vec::new();
    let mut cur = gk;
    loop {
        let (_, prev) = came_from.get(&cur)?;
        if *prev == cur { break; } // reached start
        path.push(cur.tuple());
        cur = *prev;
        if path.len() > 300 { return None; }
    }
    path.reverse();
    Some(path)
}

// ---------------------------------------------------------------------------
// Flat A* — for same-cluster pathfinding (small, fast)
// ---------------------------------------------------------------------------

fn astar_flat(
    start: (usize, usize),
    goal: (usize, usize),
    map: &Map,
    elevation: &[f64],
    path_memory: &PathMemory,
    road_wear: &[u32],
) -> Option<Vec<(usize, usize)>> {
    if start == goal { return Some(vec![]); }
    let w = map.width;
    let h = map.height;

    let mut open = BinaryHeap::new();
    let mut g_scores: HashMap<TileKey, f32> = HashMap::new();
    let mut came_from: HashMap<TileKey, TileKey> = HashMap::new();

    let sk = TileKey::new(start.0, start.1);
    g_scores.insert(sk, 0.0);
    open.push(AStarNode { tile: start, g: 0.0, f: heuristic(start, goal) });

    let mut nodes = 0u32;

    while let Some(current) = open.pop() {
        nodes += 1;
        if nodes > 2000 { return None; }

        let ck = TileKey::new(current.tile.0, current.tile.1);
        if current.tile == goal {
            let mut path = Vec::new();
            let mut k = ck;
            while k != sk {
                path.push(k.tuple());
                k = *came_from.get(&k)?;
            }
            path.reverse();
            return Some(path);
        }

        let cg = *g_scores.get(&ck).unwrap_or(&f32::INFINITY);
        if current.g > cg { continue; }

        for n in neighbours(current.tile, w, h).iter() {
            let cost = edge_cost(current.tile, *n, map, elevation, path_memory, road_wear);
            if cost.is_infinite() { continue; }
            let ng = cg + cost;
            let nk = TileKey::new(n.0, n.1);
            if ng < *g_scores.get(&nk).unwrap_or(&f32::INFINITY) {
                g_scores.insert(nk, ng);
                came_from.insert(nk, ck);
                open.push(AStarNode { tile: *n, g: ng, f: ng + heuristic(*n, goal) });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// HPA* types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Debug)]
struct ClusterId(usize, usize);

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Debug)]
struct EntranceId(u64);

impl EntranceId {
    fn new(cx: usize, cy: usize, local: usize) -> Self { Self(((cx as u64) << 48) | ((cy as u64) << 32) | (local as u64)) }
    fn cluster_cx(self) -> usize { ((self.0 >> 48) & 0xFFFF) as usize }
    fn cluster_cy(self) -> usize { ((self.0 >> 32) & 0xFFFF) as usize }
    fn local(self) -> usize { (self.0 & 0xFFFF_FFFF) as usize }
}

struct Entrance {
    id: EntranceId,
    world_tile: (usize, usize),
    neighbour: Option<EntranceId>,
}

struct ClusterData {
    id: ClusterId,
    origin_tile: (usize, usize),
    entrances: Vec<Entrance>,
    /// Intra-cluster edge costs: intra_costs[from_local] = [(to_local, cost)]
    intra_costs: Vec<Vec<(usize, f32)>>,
    dirty: bool,
}

// ---------------------------------------------------------------------------
// HpaGraph resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct HpaGraph {
    clusters: Vec<ClusterData>,
    num_cx: usize,
    num_cy: usize,
    cluster_size: usize,
}

impl HpaGraph {
    pub fn new(map_w: usize, map_h: usize) -> Self {
        let cx_count = map_w.div_ceil(CLUSTER_SIZE);
        let cy_count = map_h.div_ceil(CLUSTER_SIZE);
        let mut clusters = Vec::with_capacity(cx_count * cy_count);
        for cy in 0..cy_count {
            for cx in 0..cx_count {
                clusters.push(ClusterData {
                    id: ClusterId(cx, cy),
                    origin_tile: (cx * CLUSTER_SIZE, cy * CLUSTER_SIZE),
                    entrances: Vec::new(),
                    intra_costs: Vec::new(),
                    dirty: true,
                });
            }
        }
        Self { clusters, num_cx: cx_count, num_cy: cy_count, cluster_size: CLUSTER_SIZE }
    }

    fn cluster_index(&self, cx: usize, cy: usize) -> usize { cy * self.num_cx + cx }
    fn cluster_of_tile(&self, tx: usize, ty: usize) -> (usize, usize) { (tx / self.cluster_size, ty / self.cluster_size) }
    fn cluster_mut(&mut self, cx: usize, cy: usize) -> &mut ClusterData { let i = self.cluster_index(cx, cy); &mut self.clusters[i] }
    fn cluster(&self, cx: usize, cy: usize) -> &ClusterData { let i = self.cluster_index(cx, cy); &self.clusters[i] }

    fn entrance_pos(&self, eid: EntranceId) -> (usize, usize) {
        let cd = self.cluster(eid.cluster_cx(), eid.cluster_cy());
        cd.entrances[eid.local()].world_tile
    }
}

// ---------------------------------------------------------------------------
// Cluster rebuild — find entrances + compute intra-cluster costs
// ---------------------------------------------------------------------------

fn rebuild_cluster(
    cd: &mut ClusterData,
    map: &Map,
    elevation: &[f64],
    path_memory: &PathMemory,
    road_wear: &[u32],
) {
    let ox = cd.origin_tile.0;
    let oy = cd.origin_tile.1;
    let max_x = (ox + CLUSTER_SIZE).min(map.width);
    let max_y = (oy + CLUSTER_SIZE).min(map.height);

    let traversable = |tx: usize, ty: usize| -> bool {
        if tx >= map.width || ty >= map.height { return false; }
        let idx = ty * map.width + tx;
        !matches!(map.tiles[idx], TileType::Water | TileType::DeepWater | TileType::Lava)
    };

    // Collect entrance candidates per side, then merge consecutive tiles
    struct SideEntrance { world_tile: (usize, usize), neighbour: EntranceId }
    let mut side_entries: Vec<SideEntrance> = Vec::new();

    // Top border
    if oy > 0 {
        for x in ox..max_x {
            if traversable(x, oy) && traversable(x, oy.saturating_sub(1)) {
                side_entries.push(SideEntrance {
                    world_tile: (x, oy),
                    neighbour: EntranceId::new(cd.id.0, cd.id.1.saturating_sub(1), 0), // temp
                });
            }
        }
    }
    // Bottom border
    if oy + CLUSTER_SIZE < map.height {
        let by = max_y - 1;
        for x in ox..max_x {
            if traversable(x, by) && traversable(x, by + 1) {
                side_entries.push(SideEntrance {
                    world_tile: (x, by),
                    neighbour: EntranceId::new(cd.id.0, cd.id.1 + 1, 0),
                });
            }
        }
    }
    // Left border
    if ox > 0 {
        for y in oy..max_y {
            if traversable(ox, y) && traversable(ox.saturating_sub(1), y) {
                side_entries.push(SideEntrance {
                    world_tile: (ox, y),
                    neighbour: EntranceId::new(cd.id.0.saturating_sub(1), cd.id.1, 0),
                });
            }
        }
    }
    // Right border
    if ox + CLUSTER_SIZE < map.width {
        let rx = max_x - 1;
        for y in oy..max_y {
            if traversable(rx, y) && traversable(rx + 1, y) {
                side_entries.push(SideEntrance {
                    world_tile: (rx, y),
                    neighbour: EntranceId::new(cd.id.0 + 1, cd.id.1, 0),
                });
            }
        }
    }

    // Merge consecutive entrance tiles on the same border side
    let mut entrances: Vec<Entrance> = Vec::new();
    let mut local_idx = 0u32;

    // Group by which side they're on
    // We'll just take every Nth tile as an entrance to reduce count
    // (simple approach: keep entrances but de-duplicate by proximity)
    let mut seen = HashSet::new();
    for se in &side_entries {
        let wt = se.world_tile;
        // Skip if too close to previous entrance (merge)
        let too_close = entrances.iter().any(|e: &Entrance| {
            let dx = e.world_tile.0 as isize - wt.0 as isize;
            let dy = e.world_tile.1 as isize - wt.1 as isize;
            dx.abs() <= 2 && dy.abs() <= 2
        });
        if !too_close && !seen.contains(&TileKey::new(wt.0, wt.1)) {
            seen.insert(TileKey::new(wt.0, wt.1));
            entrances.push(Entrance {
                id: EntranceId::new(cd.id.0, cd.id.1, local_idx as usize),
                world_tile: wt,
                neighbour: Some(EntranceId::new(
                    se.neighbour.cluster_cx(),
                    se.neighbour.cluster_cy(),
                    0, // neighbour index will be matched during abstract search
                )),
            });
            local_idx += 1;
        }
    }

    // Compute intra-cluster costs: run Dijkstra from each entrance, capture
    // distances to all other entrances
    let n = entrances.len();
    let mut intra_costs: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];

    for i in 0..n {
        let goals: Vec<(usize, usize)> = entrances.iter().enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, e)| e.world_tile)
            .collect();
        if goals.is_empty() { continue; }

        let came_from = dijkstra_to_goals(entrances[i].world_tile, &goals, map, elevation, path_memory, road_wear);

        for (j, e) in entrances.iter().enumerate() {
            if j == i { continue; }
            let gk = TileKey::new(e.world_tile.0, e.world_tile.1);
            if let Some(&(cost, _)) = came_from.get(&gk) {
                intra_costs[i].push((j, cost));
            }
        }
    }

    cd.entrances = entrances;
    cd.intra_costs = intra_costs;
    cd.dirty = false;
}

// ---------------------------------------------------------------------------
// HPA* pathfinding query
// ---------------------------------------------------------------------------

pub fn find_path(
    src_tile: (usize, usize),
    dst_tile: (usize, usize),
    graph: &mut HpaGraph,
    map: &Map,
    elevation: &[f64],
    path_memory: &PathMemory,
    road_wear: &[u32],
    no_path_cache: &mut NoPathCache,
    frame: u64,
) -> Option<Vec<(usize, usize)>> {
    if src_tile == dst_tile { return Some(vec![]); }
    if src_tile.0 >= map.width || src_tile.1 >= map.height
        || dst_tile.0 >= map.width || dst_tile.1 >= map.height { return None; }

    let sk = TileKey::new(src_tile.0, src_tile.1);
    let dk = TileKey::new(dst_tile.0, dst_tile.1);
    if no_path_cache.contains(sk, dk, frame) { return None; }

    let (scx, scy) = graph.cluster_of_tile(src_tile.0, src_tile.1);
    let (dcx, dcy) = graph.cluster_of_tile(dst_tile.0, dst_tile.1);

    // Same cluster → flat A*
    if scx == dcx && scy == dcy {
        {
            let cd = graph.cluster_mut(scx, scy);
            if cd.dirty { rebuild_cluster(cd, map, elevation, path_memory, road_wear); }
        }
        let result = astar_flat(src_tile, dst_tile, map, elevation, path_memory, road_wear);
        if result.is_none() { no_path_cache.insert(sk, dk, frame); }
        return result;
    }

    // Ensure src & dst clusters are built
    for &(cx, cy) in &[(scx, scy), (dcx, dcy)] {
        let cd = graph.cluster_mut(cx, cy);
        if cd.dirty { rebuild_cluster(cd, map, elevation, path_memory, road_wear); }
    }

    let src_cd = graph.cluster(scx, scy);
    let dst_cd = graph.cluster(dcx, dcy);

    if src_cd.entrances.is_empty() || dst_cd.entrances.is_empty() {
        no_path_cache.insert(sk, dk, frame);
        return None;
    }

    // ---- Single Dijkstra from src to all entrances of src cluster ----
    let src_goals: Vec<(usize, usize)> = src_cd.entrances.iter().map(|e| e.world_tile).collect();
    let src_dijkstra = dijkstra_to_goals(src_tile, &src_goals, map, elevation, path_memory, road_wear);

    let mut src_to_ent: Vec<(EntranceId, f32)> = Vec::new();
    for ent in &src_cd.entrances {
        let gk = TileKey::new(ent.world_tile.0, ent.world_tile.1);
        if let Some(&(cost, _)) = src_dijkstra.get(&gk) {
            src_to_ent.push((ent.id, cost));
        }
    }
    if src_to_ent.is_empty() { no_path_cache.insert(sk, dk, frame); return None; }

    // ---- Single Dijkstra from each dst entrance to dst tile ----
    // We need costs from each dst entrance → dst. Run reverse Dijkstra from dst.
    let dst_goals: Vec<(usize, usize)> = dst_cd.entrances.iter().map(|e| e.world_tile).collect();
    let dst_dijkstra = dijkstra_to_goals(dst_tile, &dst_goals, map, elevation, path_memory, road_wear);

    let mut ent_to_dst: Vec<(EntranceId, f32)> = Vec::new();
    for ent in &dst_cd.entrances {
        let gk = TileKey::new(ent.world_tile.0, ent.world_tile.1);
        if let Some(&(cost, _)) = dst_dijkstra.get(&gk) {
            ent_to_dst.push((ent.id, cost));
        }
    }
    if ent_to_dst.is_empty() { no_path_cache.insert(sk, dk, frame); return None; }

    // ---- Abstract A* over entrance graph ----
    let dst_ent_set: HashSet<EntranceId> = ent_to_dst.iter().map(|(e, _)| *e).collect();

    let mut open: BinaryHeap<(i32, EntranceId)> = BinaryHeap::new();
    let mut g_abs: HashMap<EntranceId, f32> = HashMap::new();
    let mut came_from_abs: HashMap<EntranceId, EntranceId> = HashMap::new();

    for &(eid, cost) in &src_to_ent {
        let h = heuristic(graph.entrance_pos(eid), dst_tile);
        g_abs.insert(eid, cost);
        open.push((-(cost as i32 + h as i32), eid));
    }

    let mut abs_nodes = 0u32;
    let mut best_goal: Option<(EntranceId, f32)> = None;

    while let Some((_, current)) = open.pop() {
        abs_nodes += 1;
        if abs_nodes > ABSTRACT_NODE_LIMIT as u32 { break; }

        let cg = *g_abs.get(&current).unwrap_or(&f32::INFINITY);

        if dst_ent_set.contains(&current) {
            if let Some(&(_, to_dst)) = ent_to_dst.iter().find(|(e, _)| *e == current) {
                let total = cg + to_dst;
                if best_goal.map_or(true, |(_, b)| total < b) {
                    best_goal = Some((current, total));
                }
            }
            continue;
        }

        // Intra-cluster edges
        let ccx = current.cluster_cx();
        let ccy = current.cluster_cy();
        let cluster = graph.cluster(ccx, ccy);
        let local = current.local();
        if local < cluster.intra_costs.len() {
            for &(to_local, cost) in &cluster.intra_costs[local] {
                let to_eid = EntranceId::new(ccx, ccy, to_local);
                let tentative = cg + cost;
                if tentative < *g_abs.get(&to_eid).unwrap_or(&f32::INFINITY) {
                    g_abs.insert(to_eid, tentative);
                    came_from_abs.insert(to_eid, current);
                    let h = heuristic(graph.entrance_pos(to_eid), dst_tile);
                    open.push((-(tentative as i32 + h as i32), to_eid));
                }
            }
        }

        // Inter-cluster edge (neighbour)
        if local < cluster.entrances.len() {
            if let Some(neighbour) = cluster.entrances[local].neighbour {
                let ncx = neighbour.cluster_cx();
                let ncy = neighbour.cluster_cy();
                let n_local = neighbour.local();
                if ncx < graph.num_cx && ncy < graph.num_cy {
                    let n_cluster = graph.cluster(ncx, ncy);
                    if n_local < n_cluster.entrances.len() {
                        // Match: find entrance in neighbour cluster at the same border tile
                        let wt = cluster.entrances[local].world_tile;
                        for (nl, n_ent) in n_cluster.entrances.iter().enumerate() {
                            let nwt = n_ent.world_tile;
                            if (wt.0 as isize - nwt.0 as isize).abs() <= 1
                                && (wt.1 as isize - nwt.1 as isize).abs() <= 1
                            {
                                let cross_cost = if wt == nwt { 0.0 } else {
                                    edge_cost(wt, nwt, map, elevation, path_memory, road_wear)
                                };
                                if cross_cost.is_finite() {
                                    let n_eid = EntranceId::new(ncx, ncy, nl);
                                    let tentative = cg + cross_cost;
                                    if tentative < *g_abs.get(&n_eid).unwrap_or(&f32::INFINITY) {
                                        g_abs.insert(n_eid, tentative);
                                        came_from_abs.insert(n_eid, current);
                                        let h = heuristic(graph.entrance_pos(n_eid), dst_tile);
                                        open.push((-(tentative as i32 + h as i32), n_eid));
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let (goal_ent, _) = best_goal?;

    // Reconstruct abstract path
    let mut ent_path: Vec<EntranceId> = vec![goal_ent];
    let mut cur = goal_ent;
    while let Some(&prev) = came_from_abs.get(&cur) {
        ent_path.push(prev);
        cur = prev;
        if ent_path.len() > 500 { break; }
    }
    ent_path.reverse();

    // Refine: build full tile path
    let mut full_path: Vec<(usize, usize)> = Vec::new();

    // Src → first entrance
    let first_ent = ent_path.first()?;
    let first_wt = graph.entrance_pos(*first_ent);
    // Find the specific entrance in src cluster
    let src_cd = graph.cluster(first_ent.cluster_cx(), first_ent.cluster_cy());
    let first_wt_actual = if first_ent.local() < src_cd.entrances.len() {
        src_cd.entrances[first_ent.local()].world_tile
    } else { first_wt };
    if let Some(seg) = astar_flat(src_tile, first_wt_actual, map, elevation, path_memory, road_wear) {
        full_path.extend(seg);
    }

    // Walk abstract path
    for w in ent_path.windows(2) {
        let a = w[0]; let b = w[1];
        if a.cluster_cx() == b.cluster_cx() && a.cluster_cy() == b.cluster_cy() {
            let cd = graph.cluster(a.cluster_cx(), a.cluster_cy());
            let wt_a = cd.entrances[a.local()].world_tile;
            let wt_b = cd.entrances[b.local()].world_tile;
            if let Some(seg) = astar_flat(wt_a, wt_b, map, elevation, path_memory, road_wear) {
                full_path.extend(seg);
            }
        } else {
            // Border crossing
            let cd_a = graph.cluster(a.cluster_cx(), a.cluster_cy());
            let cd_b = graph.cluster(b.cluster_cx(), b.cluster_cy());
            let wt_a = cd_a.entrances[a.local()].world_tile;
            let wt_b = cd_b.entrances[b.local()].world_tile;
            if wt_a != wt_b { full_path.push(wt_b); }
        }
    }

    // Last entrance → dst
    let last_ent = ent_path.last()?;
    let last_cd = graph.cluster(last_ent.cluster_cx(), last_ent.cluster_cy());
    let last_wt = if last_ent.local() < last_cd.entrances.len() {
        last_cd.entrances[last_ent.local()].world_tile
    } else { graph.entrance_pos(*last_ent) };
    if let Some(seg) = astar_flat(last_wt, dst_tile, map, elevation, path_memory, road_wear) {
        full_path.extend(seg);
    }

    full_path.dedup();
    if full_path.is_empty() { no_path_cache.insert(sk, dk, frame); return None; }
    Some(full_path)
}

// ---------------------------------------------------------------------------
// Path request queue
// ---------------------------------------------------------------------------

pub struct PathRequest {
    pub entity: Entity,
    pub src_tile: (usize, usize),
    pub dst_tile: (usize, usize),
    pub world_target: (f32, f32),
    pub purposeful: bool,
}

#[derive(Resource, Default)]
pub struct PathRequestQueue {
    pub requests: Vec<PathRequest>,
}

// ---------------------------------------------------------------------------
// No-path cache
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct NoPathCache {
    entries: HashMap<(TileKey, TileKey), u64>,
}

impl Default for NoPathCache {
    fn default() -> Self { Self { entries: HashMap::new() } }
}

impl NoPathCache {
    fn contains(&self, src: TileKey, dst: TileKey, current_frame: u64) -> bool {
        self.entries.get(&(src, dst)).map_or(false, |&t| current_frame - t < NO_PATH_TTL)
    }
    fn insert(&mut self, src: TileKey, dst: TileKey, current_frame: u64) {
        self.entries.insert((src, dst), current_frame);
    }
}

// ---------------------------------------------------------------------------
// process_path_requests — called in FixedUpdate
// ---------------------------------------------------------------------------

pub fn process_path_requests(
    mut commands: Commands,
    mut queue: ResMut<PathRequestQueue>,
    mut chars: Query<(Entity, &mut super::player::Character)>,
    map: Res<Map>,
    elevation: Res<ElevationMap>,
    path_memory: Res<PathMemory>,
    road_wear: Res<RoadWear>,
    graph: Option<ResMut<HpaGraph>>,
    mut no_path_cache: ResMut<NoPathCache>,
    mut frame: Local<u64>,
) {
    *frame += 1;

    let mut graph = match graph {
        Some(g) => g,
        None => {
            commands.insert_resource(HpaGraph::new(map.width, map.height));
            return;
        }
    };

    let mut processed = 0;
    let mut remaining = Vec::new();

    for req in queue.requests.drain(..) {
        if processed >= MAX_REQUESTS_PER_FRAME {
            remaining.push(req);
            continue;
        }

        let result = find_path(
            req.src_tile, req.dst_tile,
            &mut graph, &map, &elevation.0, &path_memory, &road_wear.wear,
            &mut no_path_cache, *frame,
        );

        if let Ok((_e, mut ch)) = chars.get_mut(req.entity) {
            use super::player::AiState;
            match result {
                Some(found_path) => {
                    match &mut ch.state {
                        AiState::MoveTo { ref mut path, ref mut cursor, ref mut path_pending, .. } => {
                            *path = found_path;
                            *cursor = 0;
                            *path_pending = false;
                        }
                        AiState::GoingToShop | AiState::GoingToSocial(_, _) => {
                            ch.pending_path = found_path;
                            ch.path_cursor = 0;
                            ch.path_pending = false;
                        }
                        _ => {}
                    }
                }
                None => {
                    ch.state = AiState::Idle;
                    ch.timer = 10.0;
                    ch.path_pending = false;
                }
            }
        }
        processed += 1;
    }
    queue.requests.extend(remaining);
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct PathfindingPlugin;

impl Plugin for PathfindingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PathRequestQueue>();
        app.init_resource::<NoPathCache>();
    }
}
