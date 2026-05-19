use bevy::prelude::*;

use crate::map::{Map, TileType, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

const TILE_COUNT: usize = 15;

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
// Multi-octave value noise
// ---------------------------------------------------------------------------

fn raw_heightmap(rng: &mut SimRng) -> Vec<f64> {
    let octaves = 3;
    let mut map = vec![0.0_f64; MAP_WIDTH * MAP_HEIGHT];

    for octave in 0..octaves {
        let freq = 16 << octave;
        let amp = 1.0 / (octave + 1) as f64;

        let gw = MAP_WIDTH / freq + 3;
        let gh = MAP_HEIGHT / freq + 3;
        let mut grid = vec![0.0_f64; gw * gh];
        for v in &mut grid {
            *v = rng.gen_f64();
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

                map[y * MAP_WIDTH + x] += v * amp;
            }
        }
    }

    map
}

fn island_falloff(h: &mut [f64]) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let dx = x as f64 / MAP_WIDTH as f64 - 0.5;
            let dy = y as f64 / MAP_HEIGHT as f64 - 0.5;
            let dist = (dx * dx + dy * dy).sqrt() * 2.0;
            h[y * MAP_WIDTH + x] *= (1.0 - dist * dist).max(0.0);
        }
    }
}

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
// Biome lookup: elevation × moisture → TileType
// ---------------------------------------------------------------------------

fn biome(elevation: f64, moisture: f64) -> TileType {
    if elevation < 0.22 {
        TileType::DeepWater
    } else if elevation < 0.30 {
        TileType::Water
    } else if elevation < 0.38 {
        // coastline
        if moisture < 0.3 {
            TileType::Sand
        } else if moisture < 0.7 {
            TileType::Sand
        } else {
            TileType::Grass
        }
    } else if elevation < 0.50 {
        // lowlands
        if moisture < 0.2 {
            TileType::Desert
        } else if moisture < 0.5 {
            TileType::Grass
        } else if moisture < 0.75 {
            TileType::Forest
        } else {
            TileType::Swamp
        }
    } else if elevation < 0.70 {
        // midlands
        if moisture < 0.2 {
            TileType::Desert
        } else if moisture < 0.4 {
            TileType::Grass
        } else if moisture < 0.6 {
            TileType::Meadow
        } else {
            TileType::Forest
        }
    } else if elevation < 0.85 {
        // highlands
        if moisture < 0.2 {
            TileType::Clay
        } else if moisture < 0.4 {
            TileType::Dirt
        } else if moisture < 0.7 {
            TileType::Stone
        } else {
            TileType::Forest
        }
    } else if elevation < 0.92 {
        // alpine
        if moisture < 0.3 {
            TileType::Tundra
        } else if moisture < 0.6 {
            TileType::Stone
        } else {
            TileType::Snow
        }
    } else {
        // peaks
        if moisture < 0.3 {
            TileType::Tundra
        } else if moisture < 0.6 {
            TileType::Snow
        } else {
            TileType::Ice
        }
    }
}

// ---------------------------------------------------------------------------
// Volcanic hotspot overlay
// ---------------------------------------------------------------------------

fn apply_volcanic(map: &mut Map, rng: &mut SimRng) {
    // Volcanoes use a wide, sparse noise layer with high threshold.
    // A separate 2-octave noise field; only the top 5 % become lava.
    let mut volc = raw_heightmap(rng);
    normalize(&mut volc);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            // Only place lava on land tiles
            if volc[y * MAP_WIDTH + x] > 0.92
                && map.tiles[y * MAP_WIDTH + x] != TileType::DeepWater
                && map.tiles[y * MAP_WIDTH + x] != TileType::Water
            {
                map.tiles[y * MAP_WIDTH + x] = TileType::Lava;
            }
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

    // 1. elevation layer (+ island falloff)
    let mut elevation = raw_heightmap(&mut rng);
    island_falloff(&mut elevation);
    normalize(&mut elevation);

    // 2. moisture layer
    let mut moisture = raw_heightmap(&mut rng);
    normalize(&mut moisture);

    // 3. store for later systems
    commands.insert_resource(ElevationMap(elevation.clone()));
    commands.insert_resource(MoistureMap(moisture.clone()));

    // 4. biome lookup  (elevation × moisture → terrain)
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = y * MAP_WIDTH + x;
            map.tiles[idx] = biome(elevation[idx], moisture[idx]);
        }
    }

    // 5. volcanic overlays
    apply_volcanic(&mut map, &mut rng);

    // 6. majority filter — merge scattered tiles into contiguous regions
    majority_filter(&mut map, 2);

    // 7. cellular automata — clean up edge artifacts
    cellular_smooth(&mut map);
}
