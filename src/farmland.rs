use bevy::prelude::*;

use crate::actions::ActionEvent;
use crate::assets::GameAssets;
use crate::map::{Map, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_time::{TimeScale, MONTH};

// ---------------------------------------------------------------------------
// Constants — expressed in game days (1 tick ≈ 1 day, 1 month = 30 days)
// ---------------------------------------------------------------------------

/// Crop growth time (~1 month).
const GROW_TIME: f64 = 1.0 * MONTH;
const WEED_CHANCE: f64 = 0.35;
/// Days to clear one tile of wild vegetation (15 days).
pub const CLEAR_TIME: f64 = 15.0;
/// Number of tiles automatically spawned per plot at game start.
const STARTER_TILES: usize = 12;
/// Target number of tiles per plot (organic shape, not a fixed rectangle).
const TARGET_PLOT_SIZE: usize = 15;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CropState {
    Fallow,
    Growing,
    Weedy,
    Ready,
    /// Tile is being cleared from wild vegetation — `growth` tracks progress.
    Clearing,
}

#[derive(Component)]
pub struct FarmTile {
    pub plot: usize,
    pub tile_x: usize,
    pub tile_y: usize,
    pub state: CropState,
    pub growth: f64,
}

// ---------------------------------------------------------------------------
// Resource – positions determined at startup by scanning the generated map
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct FarmLayout {
    /// Each plot's tile positions (irregular organic shape).
    pub plots: Vec<Vec<(usize, usize)>>,
    /// Top-left tile of each 2×2 house.
    pub houses: Vec<(usize, usize)>,
    /// Spawn tile for each character.
    pub chars: Vec<(usize, usize)>,
}

/// Tiles that have been reserved for a plot but not yet cleared by a character.
/// Keyed by plot ID (house_id / settlement_id).
#[derive(Resource, Default)]
pub struct PendingFarmland {
    pub plots: std::collections::HashMap<usize, Vec<(usize, usize)>>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct FarmlandPlugin;

impl Plugin for FarmlandPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, (setup_farm_layout, spawn_farmland).chain());
        app.add_systems(Update, (update_crop_growth, farm_interaction));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn color_for_state(state: CropState) -> Color {
    match state {
        CropState::Fallow => Color::srgb(0.55, 0.35, 0.20),
        CropState::Growing => Color::srgb(0.20, 0.65, 0.15),
        CropState::Weedy => Color::srgb(0.40, 0.55, 0.10),
        CropState::Ready => Color::srgb(0.85, 0.75, 0.15),
        CropState::Clearing => Color::srgb(0.55, 0.35, 0.20),
    }
}

/// Colour for a tile being cleared — smooth transition from grass-green to fallow brown.
pub fn color_for_clearing(progress: f64) -> Color {
    let t = progress.min(1.0) as f32;
    Color::srgb(
        0.45 + (0.55 - 0.45) * t,  // 0.45 → 0.55
        0.60 - (0.60 - 0.35) * t,  // 0.60 → 0.35
        0.25 - (0.25 - 0.20) * t,  // 0.25 → 0.20
    )
}

// ---------------------------------------------------------------------------
// Position finding – flood-fill irregular organic plots on grass / meadow
// ---------------------------------------------------------------------------

fn find_plot_positions(map: &Map, count: usize) -> Vec<Vec<(usize, usize)>> {
    let arable = [TileType::Grass, TileType::Meadow];
    let cx = MAP_WIDTH as f64 / 2.0;
    let cy = MAP_HEIGHT as f64 / 2.0;

    // All arable tiles sorted by distance from map centre
    let mut candidates: Vec<(usize, usize, f64)> = Vec::new();
    for y in 1..MAP_HEIGHT - 1 {
        for x in 1..MAP_WIDTH - 1 {
            if arable.contains(&map.tiles[y * MAP_WIDTH + x]) {
                let d = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt();
                candidates.push((x, y, d));
            }
        }
    }
    candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    let mut used = vec![false; MAP_WIDTH * MAP_HEIGHT];
    let mut plots = vec![Vec::new(); count];

    for pi in 0..count {
        let seed_pos = candidates.iter().position(|(x, y, _)| {
            if used[*y * MAP_WIDTH + *x] {
                return false;
            }
            // Keep minimum distance from already-placed plots
            plots[..pi].iter().all(|plot: &Vec<(usize, usize)>| {
                plot.iter().all(|(px, py)| {
                    let dx = *x as isize - *px as isize;
                    let dy = *y as isize - *py as isize;
                    dx.abs() >= 4 || dy.abs() >= 4
                })
            })
        });

        let plot = &mut plots[pi];
        if let Some(idx) = seed_pos {
            let (sx, sy, _) = candidates.remove(idx);
            used[sy * MAP_WIDTH + sx] = true;
            plot.push((sx, sy));

            // Flood-fill outward from seed to collect a contiguous cluster
            use std::collections::VecDeque;
            let mut queue = VecDeque::new();
            queue.push_back((sx, sy));

            while let Some((cx, cy)) = queue.pop_front() {
                if plot.len() >= TARGET_PLOT_SIZE {
                    break;
                }
                for (dx, dy) in &[(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                    let nx = cx as isize + dx;
                    let ny = cy as isize + dy;
                    if nx < 1 || nx >= MAP_WIDTH as isize - 1
                        || ny < 1 || ny >= MAP_HEIGHT as isize - 1
                    {
                        continue;
                    }
                    let nidx = ny as usize * MAP_WIDTH + nx as usize;
                    if !used[nidx] && arable.contains(&map.tiles[nidx]) {
                        used[nidx] = true;
                        let (nxu, nyu) = (nx as usize, ny as usize);
                        plot.push((nxu, nyu));
                        queue.push_back((nxu, nyu));
                    }
                }
            }
        }
    }

    plots
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

pub fn setup_farm_layout(map: Res<Map>, mut commands: Commands) {
    const INITIAL_COUNT: usize = 3;
    let mut plots = find_plot_positions(&map, INITIAL_COUNT);
    let mut houses = Vec::with_capacity(INITIAL_COUNT);
    let mut chars = Vec::with_capacity(INITIAL_COUNT);

    for i in 0..INITIAL_COUNT {
        let tiles = &plots[i];
        if tiles.is_empty() {
            // Fallback: safe open area
            houses.push((37, 42 + i * 4));
            chars.push((39, 43 + i * 4));
            continue;
        }

        // Rightmost tile column → house goes just to the right
        let rightmost = tiles.iter().map(|(x, _)| *x).max().unwrap();
        let col_ys: Vec<usize> = tiles.iter()
            .filter(|(x, _)| *x == rightmost)
            .map(|(_, y)| *y)
            .collect();
        let min_y = *col_ys.iter().min().unwrap_or(&0);
        let max_y = *col_ys.iter().max().unwrap_or(&0);
        let center_y = (min_y + max_y) / 2;

        let h = (rightmost + 1, center_y.saturating_sub(1));
        let c = (h.0 + 2, h.1 + 1);
        houses.push(h);
        chars.push(c);
    }

    // --- 为商店预留空间 ---
    // 商店在 spawn_shop 中放置在 houses[0] + (4, 5)，占地 2x2
    // 清除商店区域及周围 2 格缓冲内的农田，确保角色能走到商店
    if !houses.is_empty() {
        let (hx, hy) = houses[0];
        let shop_x = hx + 4;
        let shop_y = hy + 5;

        // 需要清除的区域: shop的 2x2 范围 + 周围 2 格缓冲
        let min_cx = (shop_x as isize - 2).max(0) as usize;
        let min_cy = (shop_y as isize - 2).max(0) as usize;
        let max_cx = (shop_x as isize + 3).min(MAP_WIDTH as isize - 1) as usize;
        let max_cy = (shop_y as isize + 3).min(MAP_HEIGHT as isize - 1) as usize;

        for plot in plots.iter_mut() {
            plot.retain(|(px, py)| {
                !(*px >= min_cx && *px <= max_cx && *py >= min_cy && *py <= max_cy)
            });
        }
    }

    commands.insert_resource(FarmLayout {
        plots,
        houses,
        chars,
    });
}

pub fn farm_texture<'a>(state: CropState, assets: &'a GameAssets) -> &'a Handle<Image> {
    match state {
        CropState::Fallow => &assets.misc_farm_fallow,
        CropState::Growing => &assets.misc_farm_growing,
        CropState::Weedy => &assets.misc_farm_weedy,
        CropState::Ready => &assets.misc_farm_ready,
        // Clearing uses the fallow (dirt) texture as base
        CropState::Clearing => &assets.misc_farm_fallow,
    }
}

fn spawn_farmland(mut commands: Commands, layout: Res<FarmLayout>, assets: Res<GameAssets>) {
    let mut pending = PendingFarmland::default();

    for (plot_idx, tiles) in layout.plots.iter().enumerate() {
        for (i, &(fx, fy)) in tiles.iter().enumerate() {
            if i < STARTER_TILES {
                // Spawn the first few tiles immediately
                let world_x = fx as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let world_y = fy as f32 * TILE_SIZE + TILE_SIZE / 2.0;

                commands.spawn((
                    FarmTile {
                        plot: plot_idx,
                        tile_x: fx,
                        tile_y: fy,
                        state: CropState::Fallow,
                        growth: 0.0,
                    },
                    Sprite {
                        image: assets.misc_farm_fallow.clone(),
                        custom_size: Some(Vec2::new(TILE_SIZE - 2.0, TILE_SIZE - 2.0)),
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.0),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));
            } else {
                // Queue remaining tiles for manual clearing
                pending.plots.entry(plot_idx).or_default().push((fx, fy));
            }
        }
    }

    commands.insert_resource(pending);
}

// ---------------------------------------------------------------------------
// Growth
// ---------------------------------------------------------------------------

fn update_crop_growth(
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut tiles: Query<(&mut FarmTile, &mut Sprite)>,
) {
    if scale.speed == 0.0 {
        return;
    }

    let dt = time.delta_secs_f64() * scale.speed;

    // Collect unique plots that are growing
    let mut plot_growth: Vec<(usize, f64)> = Vec::new();
    for (tile, _) in tiles.iter_mut() {
        if tile.state == CropState::Growing
            && !plot_growth.iter().any(|(p, _)| *p == tile.plot)
        {
            plot_growth.push((tile.plot, tile.growth));
        }
    }

    // Advance each plot and check for ripening
    use std::collections::HashMap;
    let mut updates: HashMap<usize, (f64, Option<CropState>)> = HashMap::new();
    for (plot, growth) in plot_growth.iter_mut() {
        *growth += dt;
        if *growth >= GROW_TIME {
            let weedy = rand::random::<f64>() < WEED_CHANCE;
            let state = if weedy {
                info!("[CROP] Plot #{} → WEEDY (will need weeding)", plot);
                CropState::Weedy
            } else {
                info!("[CROP] Plot #{} → READY (ready to harvest)", plot);
                CropState::Ready
            };
            updates.insert(*plot, (0.0, Some(state)));
        } else {
            updates.insert(*plot, (*growth, None));
        }
    }

    // Apply to all tiles
    for (mut tile, mut sprite) in tiles.iter_mut() {
        // Handle clearing tiles — gradual colour transition
        if tile.state == CropState::Clearing {
            tile.growth += dt;
            sprite.color = color_for_clearing(tile.growth / CLEAR_TIME);
            if tile.growth >= CLEAR_TIME {
                tile.state = CropState::Fallow;
                tile.growth = 0.0;
                sprite.color = Color::WHITE;
            }
            continue;
        }

        // Handle growing tiles
        if let Some((growth, new_state)) = updates.get(&tile.plot) {
            if tile.state == CropState::Growing {
                tile.growth = *growth;
                if let Some(state) = new_state {
                    tile.state = *state;
                    tile.growth = 0.0;
                    sprite.color = Color::WHITE;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Manual interaction (C key)
// ---------------------------------------------------------------------------

fn farm_interaction(
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<crate::ui::HoveredTile>,
    tiles: Query<&FarmTile>,
    mut events: EventWriter<ActionEvent>,
) {
    if !keys.just_pressed(KeyCode::KeyC) {
        return;
    }

    let Some((hx, hy)) = hovered.0 else {
        return;
    };

    // Find the plot of the hovered tile
    let plot = tiles
        .iter()
        .find(|t| t.tile_x == hx && t.tile_y == hy)
        .map(|t| t.plot);

    if let Some(plot_id) = plot {
        events.send(ActionEvent::FarmInteract {
            plot_id,
            house_id: None, // manual action – no house deposit
        });
    }
}
