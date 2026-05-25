use bevy::prelude::*;

use crate::map::{Map, TileType, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

const TILE_COUNT: usize = 15;

// ---------------------------------------------------------------------------
// Generation parameters
// ---------------------------------------------------------------------------

/// Number of octaves for base terrain noise (large-scale features).
const BASE_OCTAVES: usize = 5;
/// Persistence = amplitude multiplier per octave. <1 means higher freq = lower amp.
const PERSISTENCE: f64 = 0.55;
/// Base frequency for the first octave.
const BASE_FREQ: usize = 20;

/// Ridge noise creates sharp mountain ridgelines.
const RIDGE_OCTAVES: usize = 3;
const RIDGE_FREQ: usize = 10;
const RIDGE_WEIGHT: f64 = 0.35; // how much ridge noise contributes

/// Thermal erosion — material slides downslope when gradient exceeds talus angle.
const EROSION_ITERATIONS: usize = 5;
const TALUS: f64 = 0.008; // slope threshold

// ---------------------------------------------------------------------------
// World data stored for other systems to read
// ---------------------------------------------------------------------------

/// Per-tile elevation (0..1), stored after generation.
#[derive(Resource)]
pub struct ElevationMap(pub Vec<f64>);

/// Per-tile moisture (0..1), stored after generation.
#[derive(Resource)]
pub struct MoistureMap(pub Vec<f64>);

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, generate_map);
    }
}

// ---------------------------------------------------------------------------
// Multi-octave noise with custom grid fill + per-sample transform
// ---------------------------------------------------------------------------

fn sample_noise<F, G>(
    rng: &mut SimRng,
    octaves: usize,
    freq0: usize,
    grid_init: F,
    sample_transform: G,
) -> Vec<f64>
where
    F: Fn(&mut SimRng) -> f64,
    G: Fn(f64) -> f64,
{
    let mut map = vec![0.0_f64; MAP_WIDTH * MAP_HEIGHT];
    let mut max_amp = 0.0;

    for octave in 0..octaves {
        let freq = freq0 << octave;
        let amp = PERSISTENCE.powi(octave as i32);
        max_amp += amp;

        let gw = MAP_WIDTH / freq + 3;
        let gh = MAP_HEIGHT / freq + 3;
        let mut grid = vec![0.0_f64; gw * gh];
        for cell in &mut grid {
            *cell = grid_init(rng);
        }

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let gx = x as f64 / freq as f64;
                let gy = y as f64 / freq as f64;
                let ix = gx as usize;
                let iy = gy as usize;
                let fx = gx - ix as f64;
                let fy = gy - iy as f64;

                let sx = fx * fx * (3.0 - 2.0 * fx);
                let sy = fy * fy * (3.0 - 2.0 * fy);

                let v00 = grid[iy * gw + ix];
                let v10 = grid[iy * gw + ix + 1];
                let v01 = grid[(iy + 1) * gw + ix];
                let v11 = grid[(iy + 1) * gw + ix + 1];

                let v0 = v00 + (v10 - v00) * sx;
                let v1 = v01 + (v11 - v01) * sx;
                let v = v0 + (v1 - v0) * sy;

                map[y * MAP_WIDTH + x] += sample_transform(v) * amp;
            }
        }
    }

    for v in &mut map {
        *v /= max_amp;
    }
    map
}

fn value_noise(rng: &mut SimRng, octaves: usize, freq0: usize) -> Vec<f64> {
    sample_noise(rng, octaves, freq0, |rng| rng.gen_f64(), |v| v)
}

fn ridge_noise(rng: &mut SimRng) -> Vec<f64> {
    sample_noise(
        rng,
        RIDGE_OCTAVES,
        RIDGE_FREQ,
        |rng| rng.gen_f64() * 2.0 - 1.0,
        |v| (1.0 - v.abs()).powi(2),
    )
}

// ---------------------------------------------------------------------------
// Island falloff — shapes the heightmap into an island by gradually
// reducing elevation toward the edges.  Uses a mix of soft (smooth
// interior) and hard (water at edges) falloff so mountains and detail
// are preserved in the middle third while coastlines are natural.
// ---------------------------------------------------------------------------

fn island_falloff(h: &mut [f64]) {
    let inner = 0.30; // central 30 % is unaffected
    let outer = 0.95; // beyond 95 % from center → zero

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let dx = x as f64 / MAP_WIDTH as f64 - 0.5;
            let dy = y as f64 / MAP_HEIGHT as f64 - 0.5;
            let dist = (dx * dx + dy * dy).sqrt() * 2.0;

            let t = ((dist - inner) / (outer - inner)).clamp(0.0, 1.0);
            // Cubic ease-out: smooth at first, then drops fast near the edge
            let fade = 1.0 - t * t * (3.0 - 2.0 * t);
            h[y * MAP_WIDTH + x] *= fade;
        }
    }
}

// ---------------------------------------------------------------------------
// Thermal erosion — smooth unnaturally steep slopes
// ---------------------------------------------------------------------------

fn thermal_erosion(h: &mut [f64]) {
    for _iter in 0..EROSION_ITERATIONS {
        for y in 1..MAP_HEIGHT - 1 {
            for x in 1..MAP_WIDTH - 1 {
                let idx = y * MAP_WIDTH + x;
                let center = h[idx];

                let mut max_diff = 0.0;
                let mut max_ni = 0;

                for (ddx, ddy) in &[(0, -1), (1, 0), (0, 1), (-1, 0)] {
                    let nx = x as isize + ddx;
                    let ny = y as isize + ddy;
                    if nx < 0 || nx >= MAP_WIDTH as isize || ny < 0 || ny >= MAP_HEIGHT as isize {
                        continue;
                    }
                    let ni = ny as usize * MAP_WIDTH + nx as usize;
                    let diff = center - h[ni];
                    if diff > max_diff {
                        max_diff = diff;
                        max_ni = ni;
                    }
                }

                if max_diff > TALUS {
                    let transfer = (max_diff - TALUS) * 0.5;
                    h[idx] -= transfer;
                    h[max_ni] += transfer;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Normalise to [0, 1]
// ---------------------------------------------------------------------------

fn normalize(h: &mut [f64]) {
    let mut min_h = f64::MAX;
    let mut max_h = f64::MIN;
    for &v in &*h {
        if v < min_h {
            min_h = v;
        }
        if v > max_h {
            max_h = v;
        }
    }
    let range = max_h - min_h;
    if range > 0.0 {
        for v in h {
            *v = (*v - min_h) / range;
        }
    }
}

// ---------------------------------------------------------------------------
// Full elevation pipeline
// ---------------------------------------------------------------------------

fn generate_elevation(rng: &mut SimRng) -> Vec<f64> {
    // 1. Base terrain — gentle hills and valleys
    let mut elev = value_noise(rng, BASE_OCTAVES, BASE_FREQ);

    // 2. Ridge noise for mountain ranges
    let ridges = ridge_noise(rng);
    for (e, r) in elev.iter_mut().zip(ridges.iter()) {
        *e = *e * (1.0 - RIDGE_WEIGHT) + r * RIDGE_WEIGHT;
    }

    // 3. Island falloff — shapes landmass with natural coastline
    island_falloff(&mut elev);

    // 4. Thermal erosion — natural slope smoothing
    thermal_erosion(&mut elev);

    // 5. Final normalise
    normalize(&mut elev);
    elev
}

// ---------------------------------------------------------------------------
// Improved moisture map — include orographic (rain-shadow) effect
// ---------------------------------------------------------------------------

fn generate_moisture(rng: &mut SimRng, elevation: &[f64]) -> Vec<f64> {
    let mut moisture = value_noise(rng, 4, 16);

    // Orographic effect: higher elevation = less moisture (rain shadow),
    // but with random variation so not all peaks are equally dry.
    for (m, e) in moisture.iter_mut().zip(elevation.iter()) {
        let elev_factor = 1.0 - e * 0.5; // 0.5x at peak, 1.0x at sea level
        *m *= elev_factor;
    }

    normalize(&mut moisture);
    moisture
}

// ---------------------------------------------------------------------------
// Biome lookup: elevation × moisture → TileType
// Snow appears at a clear snow line (~0.85+ elevation).
// ---------------------------------------------------------------------------

fn biome(elevation: f64, moisture: f64) -> TileType {
    if elevation < 0.18 {
        TileType::DeepWater
    } else if elevation < 0.28 {
        TileType::Water
    } else if elevation < 0.36 {
        // coastline — all sand
        TileType::Sand
    } else if elevation < 0.48 {
        // Lowlands — warm and flat
        if moisture < 0.15 {
            TileType::Desert
        } else if moisture < 0.30 {
            TileType::Sand
        } else if moisture < 0.45 {
            TileType::Grass
        } else if moisture < 0.75 {
            TileType::Forest
        } else {
            TileType::Swamp
        }
    } else if elevation < 0.65 {
        // Midlands — temperate
        if moisture < 0.15 {
            TileType::Desert
        } else if moisture < 0.25 {
            TileType::Clay
        } else if moisture < 0.45 {
            TileType::Grass
        } else if moisture < 0.60 {
            TileType::Meadow
        } else if moisture < 0.80 {
            TileType::Forest
        } else {
            TileType::Swamp
        }
    } else if elevation < 0.80 {
        // Highlands — steeper slopes, cooler
        if moisture < 0.2 {
            TileType::Dirt
        } else if moisture < 0.4 {
            TileType::Clay
        } else if moisture < 0.6 {
            TileType::Stone
        } else {
            TileType::Forest
        }
    } else if elevation < 0.88 {
        // Subalpine — rocky slopes below snow line
        if moisture < 0.3 {
            TileType::Tundra
        } else {
            TileType::Stone
        }
    } else if elevation < 0.94 {
        // Alpine snow line
        if moisture < 0.25 {
            TileType::Tundra
        } else {
            TileType::Snow
        }
    } else {
        // Peaks — ice, snow, and rare volcanic lava
        if moisture < 0.15 {
            TileType::Lava // dry volcanic peak
        } else if moisture < 0.4 {
            TileType::Snow
        } else if moisture < 0.7 {
            TileType::Snow
        } else {
            TileType::Ice
        }
    }
}

// ---------------------------------------------------------------------------
// Majority filter — each tile becomes the most common type in its area
// ---------------------------------------------------------------------------

fn majority_filter(map: &mut Map, radius: usize) {
    let old = map.tiles.clone();
    let r = radius as isize;

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let mut counts = [0_usize; TILE_COUNT];
            let mut total = 0;

            for dy in -r..=r {
                for dx in -r..=r {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                        counts[old[ny as usize * MAP_WIDTH + nx as usize] as usize] += 1;
                        total += 1;
                    }
                }
            }

            let current = map.tiles[y * MAP_WIDTH + x] as usize;
            let (winner, winner_n) = counts
                .iter()
                .enumerate()
                .max_by_key(|&(_, &c)| c)
                .unwrap();

            if *winner_n > total / 2 && winner != current {
                map.tiles[y * MAP_WIDTH + x] = tile_from_usize(winner);
            }
        }
    }
}

fn tile_from_usize(n: usize) -> TileType {
    match n {
        0 => TileType::Grass,
        1 => TileType::Water,
        2 => TileType::DeepWater,
        3 => TileType::Sand,
        4 => TileType::Forest,
        5 => TileType::Swamp,
        6 => TileType::Stone,
        7 => TileType::Dirt,
        8 => TileType::Snow,
        9 => TileType::Lava,
        10 => TileType::Tundra,
        11 => TileType::Ice,
        12 => TileType::Meadow,
        13 => TileType::Desert,
        14 => TileType::Clay,
        _ => TileType::Grass,
    }
}

// ---------------------------------------------------------------------------
// Cellular automata cleanup
// ---------------------------------------------------------------------------

fn neighbors(tiles: &[TileType], x: usize, y: usize, kinds: &[TileType], radius: usize) -> usize {
    let mut count = 0;
    let r = radius as isize;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                if kinds.contains(&tiles[ny as usize * MAP_WIDTH + nx as usize]) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn cellular_smooth(map: &mut Map) {
    for _ in 0..2 {
        let old = map.tiles.clone();

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let idx = y * MAP_WIDTH + x;
                match old[idx] {
                    TileType::Water => {
                        if neighbors(&old, x, y, &[TileType::Water, TileType::DeepWater], 1) < 2 {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                    TileType::Forest => {
                        if neighbors(&old, x, y, &[TileType::Forest, TileType::Swamp], 1) < 2 {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                    TileType::Swamp => {
                        let wet =
                            neighbors(&old, x, y, &[TileType::Water, TileType::DeepWater], 2);
                        let woods = neighbors(&old, x, y, &[TileType::Forest, TileType::Swamp], 1);
                        if wet == 0 && woods < 2 {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                    TileType::Sand => {
                        if neighbors(&old, x, y, &[TileType::Water, TileType::DeepWater], 2) == 0 {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                    TileType::Lava => {
                        if neighbors(&old, x, y, &[TileType::Lava], 1) < 2 {
                            map.tiles[idx] = TileType::Stone;
                        }
                    }
                    TileType::Tundra => {
                        if neighbors(&old, x, y, &[TileType::Tundra, TileType::Snow, TileType::Stone], 1) < 2 {
                            map.tiles[idx] = TileType::Stone;
                        }
                    }
                    TileType::Ice => {
                        if neighbors(&old, x, y, &[TileType::Ice, TileType::Snow], 1) < 2 {
                            map.tiles[idx] = TileType::Snow;
                        }
                    }
                    TileType::Meadow => {
                        if neighbors(&old, x, y, &[TileType::Meadow, TileType::Grass], 1) < 2 {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                    TileType::Desert => {
                        if neighbors(&old, x, y, &[TileType::Desert, TileType::Sand], 1) < 2 {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                    TileType::Clay => {
                        if neighbors(&old, x, y, &[TileType::Clay, TileType::Dirt], 1) < 2 {
                            map.tiles[idx] = TileType::Grass;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn generate_map(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut rng: ResMut<SimRng>,
) {
    map.width = MAP_WIDTH;
    map.height = MAP_HEIGHT;
    map.tiles = vec![TileType::Grass; MAP_WIDTH * MAP_HEIGHT];

    // 1. Improved elevation: 6-octave noise + ridge noise + erosion
    let elevation = generate_elevation(&mut rng);

    // 2. Moisture with orographic (rain-shadow) effect
    let moisture = generate_moisture(&mut rng, &elevation);

    // 3. Biome lookup (elevation × moisture → terrain)
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = y * MAP_WIDTH + x;
            map.tiles[idx] = biome(elevation[idx], moisture[idx]);
        }
    }

    // 4. Move elevation/moisture into resources (no clone needed after biome loop)
    commands.insert_resource(ElevationMap(elevation));
    commands.insert_resource(MoistureMap(moisture));

    // 5. Majority filter — merge scattered tiles into contiguous regions
    majority_filter(&mut map, 1);

    // 7. Cellular automata — clean up edge artifacts
    cellular_smooth(&mut map);

    info!(
        "[MAP] Generated {}×{} terrain ({:.1}% land, snow line at ~{:.0}% elevation)",
        MAP_WIDTH, MAP_HEIGHT,
        map.tiles.iter().filter(|&&t|
            !matches!(t, TileType::Water | TileType::DeepWater)
        ).count() as f64 / (MAP_WIDTH * MAP_HEIGHT) as f64 * 100.0,
        88.0,
    );
}
