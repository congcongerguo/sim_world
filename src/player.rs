use bevy::prelude::*;

use crate::actions::ActionEvent;
use crate::assets::GameAssets;
use crate::farmland::{color_for_clearing, setup_farm_layout, CropState, FarmLayout, FarmTile, MIN_READY_TIME, PendingFarmland};
use crate::generation::ElevationMap;
use crate::map::{Map, TileCategory, TileContent, TileEntry, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_time::YEAR;
use crate::vegetation::TreeMask;

// ---------------------------------------------------------------------------
// Constants — expressed in game days (1 tick ≈ 1 day)
// 1 month = 30 days, 1 year = 360 days
// ---------------------------------------------------------------------------

/// Days between adult meals.
const MEAL_INTERVAL: f64 = 12.0;
/// Minimum days between birth attempts (~2.5 years).
const CHILD_BIRTH_INTERVAL: f64 = 2.5 * YEAR;
/// Days for a child to grow into an adult (18 years).
const CHILD_GROWTH_DURATION: f64 = 18.0 * YEAR;
/// Max children per household.
const MAX_CHILDREN: usize = 6;
/// Max age for reproduction (45 years).
const MAX_REPRODUCTION_AGE: f64 = 45.0 * YEAR;
/// Lifespan (~60 years).
const LIFESPAN: f64 = 60.0 * YEAR;
/// Days a house can have 0 food before an adult starves (1 year).
const STARVATION_THRESHOLD: f64 = 1.0 * YEAR;

/// Graveyard top-left tile on the map.
const GRAVEYARD_X: usize = 3;
const GRAVEYARD_Y: usize = 85;
/// Graveyard extends GRAVEYARD_W columns and GRAVEYARD_H rows.
const GRAVEYARD_W: usize = 10;
const GRAVEYARD_H: usize = 15;

// Essentials & shop
/// Days between essentials consumption ticks (~1.7 months).
const ESSENTIALS_DEPLETION_INTERVAL: f64 = 50.0;
/// Food cost per shop visit.
const SHOP_COST_FOOD: u32 = 3;
/// Essentials gained per shop visit.
/// Starting essentials for each household.
const HOUSE_START_ESSENTIALS: u32 = 20;
/// Below this threshold, an adult will go shopping.
const ESSENTIALS_LOW_THRESHOLD: u32 = 5;

// Road wear
const ROAD_THRESHOLD_1: u32 = 4;
const ROAD_THRESHOLD_3: u32 = 30;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Something a character has noticed within their sight range.
/// Populated by perception_system, consumed by decision_system.
#[derive(Clone, Debug)]
pub enum Percept {
    FarmTile { entity: Entity, tile_x: usize, tile_y: usize, state: CropState, plot: usize, growth: f64 },
    Shop,
    Character { entity: Entity, gender: Gender, marital: MaritalStatus, house_id: usize },
    Grave { house_id: usize, tile_x: usize, tile_y: usize },
    TreesNearby { count: usize, center_tx: usize, center_ty: usize },
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum AiState {
    Idle,
    MoveTo(f32, f32, bool), // x, y, purposeful (creates road wear)
    Exploring { origin_x: f32, origin_y: f32, dir_x: f32, dir_y: f32 },
    GoingToShop,
    GoingToSocial(f32, f32),
    Socializing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gender {
    Male,
    Female,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Personality {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}

impl Personality {
    pub fn random() -> Self {
        Self {
            openness: rand::random::<f32>(),
            conscientiousness: rand::random::<f32>(),
            extraversion: rand::random::<f32>(),
            agreeableness: rand::random::<f32>(),
            neuroticism: rand::random::<f32>(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LifeStage {
    Child,
    Adult,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MaritalStatus {
    Single,
    Married,
    Widowed,
}

/// An autonomous character in the simulation.
#[derive(Component)]
pub struct Character {
    pub speed: f32,
    state: AiState,
    timer: f64,
    /// Flag raised when the character arrives at a farm tile.
    action_tile: Option<(usize, usize)>,
    /// Which plot (settlement) this character manages.
    pub plot_id: usize,
    /// Which house (by House::id) this character lives in.
    pub house_id: usize,
    pub stage: LifeStage,
    /// Accumulated sim-seconds since birth.
    pub age: f64,
    pub gender: Gender,
    pub personality: Personality,
    pub marital: MaritalStatus,
    /// How far (in tiles) this character can see.
    /// Adults can see ~8 tiles; children only ~4.
    pub sight_range: f32,
    /// Personal food carried by this character.
    pub food: u32,
    /// Perceived entities within sight range, refreshed every tick.
    pub percepts: Vec<Percept>,
    /// Seconds spent on very slow terrain (<10% speed). Resets on fast terrain.
    stuck_timer: f64,
    /// Consecutive frames spent in water. Reset on land. When > threshold,
    /// the SWIM handler assumes the destination is unreachable, blacklists
    /// it, and forces the character into Exploring state.
    swim_streak: u32,
}

/// Tracks age of a child character (sim-seconds accumulated).
#[derive(Component)]
pub struct Growing {
    pub age: f64,
}

/// A tombstone in the graveyard.
#[derive(Component)]
pub struct Grave;

/// Information about a deceased character, shown on hover.
#[derive(Component)]
pub struct GraveInfo {
    pub age: f64,
    pub house_id: usize,
    pub gender: Gender,
    pub cause: &'static str,
}

/// The village shop where characters buy daily essentials.
#[derive(Component)]
pub struct Shop;

/// Road overlay sprite (semi-transparent path).
#[derive(Component)]
pub struct RoadTile;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MealTimer>();
        app.init_resource::<ChildMealTimer>();
        app.init_resource::<ReproductionTimer>();
        app.init_resource::<NextSettlementId>();
        app.init_resource::<DeathEvents>();
        app.init_resource::<RoadWear>();
        app.init_resource::<RoadRender>();
        app.init_resource::<EssentialsTimer>();
        app.init_resource::<RomanceTimer>();
        app.init_resource::<NextGraveSlot>();
        app.init_resource::<PathMemory>();
        app.init_resource::<StarvationTimer>();
        app.init_resource::<FoodDiagTimer>();
        app.add_systems(PostStartup, (
            spawn_characters.after(setup_farm_layout),
            spawn_houses.after(setup_farm_layout),
            spawn_shop.after(setup_farm_layout),
        ));
        app.add_systems(FixedUpdate, (
            // 1. 先处理上一帧积累的人物操作事件（收获、种植、交易）
            crate::actions::process_action_events,
            // 2. 作物生长（基于稳定后的地块状态）
            crate::farmland::update_crop_growth,
            // 3. 感知 → 决策 → 移动
            romance_system,
            perception_system,
            decision_system,
            movement_system,
            // 4. 到达目的地后执行操作
            action_system,
            // 5. 人口变化
            reproduction_system,
            child_growth_system,
            // 6. 先消耗食物，再判断是否饿死
            (aging_system, daily_consumption).chain(),
            // 7. 继承和坟墓
            inheritance_system,
            grave_system,
            // 8. 日用品消耗
            essentials_depletion,
            // 9. 日志
            food_diagnostics,
        ).chain());
        app.add_systems(Update, (
            road_render_system,
            shop_interaction,
        ));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tile_center(tx: usize, ty: usize) -> (f32, f32) {
    (tx as f32 * TILE_SIZE + TILE_SIZE / 2.0, ty as f32 * TILE_SIZE + TILE_SIZE / 2.0)
}

fn current_tile(tf: &Transform) -> (usize, usize) {
    let x = (tf.translation.x / TILE_SIZE).floor() as usize;
    let y = (tf.translation.y / TILE_SIZE).floor() as usize;
    (x, y)
}

fn dist_to(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Find the nearest passable (non-water, non-lava) tile within a scan radius,
/// searching outward in expanding squares.  Used by the water-swim logic so
/// characters can find the shore without teleporting.
fn nearest_shore(
    tx: usize, ty: usize, map: &Map, max_radius: usize,
) -> Option<(usize, usize)> {
    for r in 1..=max_radius {
        for dy in -(r as isize)..=r as isize {
            for dx in -(r as isize)..=r as isize {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = tx as isize + dx;
                let ny = ty as isize + dy;
                if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                    let tile = map.tiles[ny as usize * MAP_WIDTH + nx as usize];
                    if !matches!(tile, TileType::Water | TileType::DeepWater | TileType::Lava) {
                        return Some((nx as usize, ny as usize));
                    }
                }
            }
        }
    }
    None
}

/// Find the nearest tile with decent movement speed (>0.15 equivalent) within
/// a scan radius.  Used when a character gets stuck on very slow terrain so
/// they can escape to a walkable tile without teleporting.
fn nearest_passable(
    tx: usize, ty: usize, map: &Map, max_radius: usize,
) -> Option<(usize, usize)> {
    for r in 1..=max_radius {
        for dy in -(r as isize)..=r as isize {
            for dx in -(r as isize)..=r as isize {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = tx as isize + dx;
                let ny = ty as isize + dy;
                if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                    let tile = map.tiles[ny as usize * MAP_WIDTH + nx as usize];
                    if !matches!(
                        tile,
                        TileType::Water | TileType::DeepWater | TileType::Lava
                            | TileType::Stone | TileType::Snow | TileType::Swamp | TileType::Ice
                    ) {
                        return Some((nx as usize, ny as usize));
                    }
                }
            }
        }
    }
    // Fallback: any non-water tile (including Stone/Snow etc.) if nothing better
    nearest_shore(tx, ty, map, max_radius)
}

/// Speed multiplier based on terrain type, road wear, elevation, and local steepness.
/// Roads (well-trodden tiles) give significant speed bonus.
/// Rough terrain (mountains, snow, swamp), steep slopes, and high elevation
/// heavily penalize movement, forcing characters to form and follow
/// established paths along valleys.
fn tile_speed_multiplier(
    map: &Map,
    tf: &Transform,
    road_wear: &[u32],
    elevation: &[f64],
) -> f32 {
    let x = (tf.translation.x / TILE_SIZE).floor() as usize;
    let y = (tf.translation.y / TILE_SIZE).floor() as usize;
    if x >= MAP_WIDTH || y >= MAP_HEIGHT {
        return 1.0;
    }
    let base = match map.tiles[y * MAP_WIDTH + x] {
        // Flat, easy terrain — baseline speed
        TileType::Grass | TileType::Meadow | TileType::Dirt => 1.0,
        // Soft / loose — moderate penalty
        TileType::Sand | TileType::Clay => 0.7,
        TileType::Desert => 0.5,
        // Rough terrain — heavy penalty to discourage off-road travel
        TileType::Tundra => 0.25,
        TileType::Forest => 0.20,
        TileType::Ice => 0.20,
        // Difficult — slow but passable at reasonable speed
        TileType::Snow => 0.20,
        TileType::Swamp => 0.15,
        TileType::Stone => 0.20,
        // Impassable (lava) / slow swimming (water)
        TileType::Water | TileType::DeepWater => 0.15,
        TileType::Lava => 0.0,
    };
    if base == 0.0 {
        return 0.0;
    }
    // Elevation penalty: higher = slower (50% penalty at peak)
    let elev = elevation[y * MAP_WIDTH + x] as f32;
    let elev_factor = 1.0 - (elev - 0.5).max(0.0) * 1.0;

    // Steepness penalty: max elevation difference with 8 neighbors
    // Climbing steep slopes is very slow even if the base terrain is easy
    let mut max_diff = 0.0f32;
    let cx = x as isize;
    let cy = y as isize;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 { continue; }
            let nx = cx + dx;
            let ny = cy + dy;
            if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                let diff = (elev - elevation[ny as usize * MAP_WIDTH + nx as usize] as f32).abs();
                if diff > max_diff { max_diff = diff; }
            }
        }
    }
    let steepness_factor = 1.0 - max_diff * 6.0; // up to ~90% penalty on very steep slopes

    // Road speed bonus: well-established paths give up to 3.0x speed
    let wear = road_wear[y * MAP_WIDTH + x];
    let road_bonus = 1.0 + (wear as f32 / ROAD_THRESHOLD_3 as f32).min(1.0) * 2.0;
    base * road_bonus * elev_factor * steepness_factor
        .max(0.02) // absolute floor: never slower than 2% of base speed
}

fn find_farm_tile<'a>(
    tiles: impl Iterator<Item = &'a (&'a FarmTile, &'a Transform)>,
    near: (f32, f32),
    wanted: CropState,
) -> Option<(usize, usize)> {
    tiles
        .filter(|(t, _)| t.state == wanted)
        .min_by(|(_, a), (_, b)| {
            let da = dist_to(near, (a.translation.x, a.translation.y));
            let db = dist_to(near, (b.translation.x, b.translation.y));
            da.partial_cmp(&db).unwrap()
        })
        .map(|(t, _)| (t.tile_x, t.tile_y))
}

// ---------------------------------------------------------------------------
// Spawn helpers
// ---------------------------------------------------------------------------

fn spawn_character_sprite(parent: &mut ChildBuilder, texture: Handle<Image>) {
    parent.spawn((
        Sprite {
            image: texture,
            custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn spawn_child_sprite(parent: &mut ChildBuilder, texture: Handle<Image>) {
    parent.spawn((
        Sprite {
            image: texture,
            custom_size: Some(Vec2::new(28.0, 28.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn spawn_house_building(parent: &mut ChildBuilder, texture: Handle<Image>) {
    parent.spawn((
        Sprite {
            image: texture,
            custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.01),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn spawn_shop_building(parent: &mut ChildBuilder, texture: Handle<Image>) {
    parent.spawn((
        Sprite {
            image: texture,
            custom_size: Some(Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 2.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.01),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

fn spawn_characters(mut commands: Commands, layout: Res<FarmLayout>, assets: Res<GameAssets>) {
    for i in 0..layout.chars.len() {
        let (tx, ty) = layout.chars[i];

        // Spawn 3 adults per house (couple + sibling/elder)
        for offset_x in [0i8, 1, 2] {
            let (x, y) = tile_center(
                (tx as i8 + offset_x) as usize,
                ty,
            );
            let gender = match offset_x {
                0 => Gender::Male,
                1 => Gender::Female,
                _ => if rand::random::<f32>() < 0.5 { Gender::Male } else { Gender::Female },
            };
            let tex = match gender {
                Gender::Male => assets.char_male.clone(),
                Gender::Female => assets.char_female.clone(),
            };
            commands.spawn((
                Character {
                    speed: 100.0,
                    state: AiState::Idle,
                    timer: offset_x as f64,
                    action_tile: None,
                    percepts: Vec::new(),
                    stuck_timer: 0.0,
                    plot_id: i,
                    house_id: i,
                    stage: LifeStage::Adult,
                    age: (rand::random::<f64>() * 15.0 + 20.0) * YEAR,
                    gender,
                    personality: Personality::random(),
                    marital: if offset_x < 2 { MaritalStatus::Married } else { MaritalStatus::Single },
                    sight_range: 8.0,
                    food: 10,
                    swim_streak: 0,
                },
                Transform::from_xyz(x, y, 2.0),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .with_children(|parent| {
                spawn_character_sprite(parent, tex);
            });
        }

        // Spawn 2 random children per household (ages 5–17)
        for _ in 0..2 {
            let child_gender = if rand::random::<f32>() < 0.5 { Gender::Male } else { Gender::Female };
            let child_tex = match child_gender {
                Gender::Male => assets.char_child.clone(),
                Gender::Female => assets.char_child.clone(),
            };
            let child_age = (rand::random::<f64>() * 12.0 + 5.0) * YEAR; // 5–17 years
            let ox = (rand::random::<f32>() - 0.5) * 64.0;
            let oy = (rand::random::<f32>() - 0.5) * 64.0;
            let (cx, cy) = tile_center(tx, ty);
            commands.spawn((
                Character {
                    speed: 80.0,
                    state: AiState::Idle,
                    timer: rand::random::<f64>() * 3.0,
                    action_tile: None,
                    percepts: Vec::new(),
                    stuck_timer: 0.0,
                    plot_id: i,
                    house_id: i,
                    stage: LifeStage::Child,
                    age: child_age,
                    gender: child_gender,
                    personality: Personality::random(),
                    marital: MaritalStatus::Single,
                    sight_range: 4.0,
                    food: 10,
                    swim_streak: 0,
                },
                Growing { age: child_age },
                Transform::from_xyz(cx + ox, cy + oy, 2.0),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .with_children(|parent| {
                spawn_child_sprite(parent, child_tex);
            });
        }
    }
}

/// The test house.
#[derive(Component)]
pub struct House {
    pub id: usize,
    pub tile_x: usize,
    pub tile_y: usize,
    pub w: usize,
    pub h: usize,
    pub storage: u32,
    pub essentials: u32,
}

fn spawn_houses(
    mut commands: Commands,
    layout: Res<FarmLayout>,
    assets: Res<GameAssets>,
    mut tile_content: ResMut<TileContent>,
) {
    for i in 0..layout.houses.len() {
        let (tile_x, tile_y) = layout.houses[i];
        let w = 1usize;
        let h = 1usize;

        let world_x = (tile_x as f32 + w as f32 / 2.0) * TILE_SIZE;
        let world_y = (tile_y as f32 + h as f32 / 2.0) * TILE_SIZE;

        commands.spawn((
            House {
                id: i,
                tile_x,
                tile_y,
                w,
                h,
                storage: 200,
                essentials: HOUSE_START_ESSENTIALS,
            },
            Transform::from_xyz(world_x, world_y, 1.3),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            spawn_house_building(parent, assets.bld_house.clone());
        });

        // Register in TileContent for UI
        for dy in 0..h {
            for dx in 0..w {
                let idx = (tile_y + dy) * MAP_WIDTH + (tile_x + dx);
                tile_content.data.entry(idx).or_default().push(TileEntry {
                    name: "House",
                    category: TileCategory::Building,
                    amount: 0,
                    w: w as u32,
                    h: h as u32,
                });
            }
        }
    }
}

/// Scan for a 2×2 sand (coastal) tile near the given position.
fn find_coastal_plot(map: &Map, near_x: usize, near_y: usize) -> Option<(usize, usize)> {
    for radius in 0..40 {
        for dy in -(radius as isize)..=radius as isize {
            for dx in -(radius as isize)..=radius as isize {
                if dx.abs() < radius && dy.abs() < radius {
                    continue;
                }
                let nx = near_x as isize + dx;
                let ny = near_y as isize + dy;
                if nx < 0 || ny < 0 || nx >= MAP_WIDTH as isize - 1 || ny >= MAP_HEIGHT as isize - 1 {
                    continue;
                }
                let all_sand = (0..2).all(|dy2| {
                    (0..2).all(|dx2| {
                        let tx = (nx + dx2) as usize;
                        let ty = (ny + dy2) as usize;
                        tx < MAP_WIDTH && ty < MAP_HEIGHT && map.tiles[ty * MAP_WIDTH + tx] == TileType::Sand
                    })
                });
                if all_sand {
                    return Some((nx as usize, ny as usize));
                }
            }
        }
    }
    None
}

/// Spawn the village shop on a coastal tile near the first settlement.
fn spawn_shop(
    mut commands: Commands,
    layout: Res<FarmLayout>,
    assets: Res<GameAssets>,
    mut tile_content: ResMut<TileContent>,
    map: Res<Map>,
) {
    let (hx, hy) = layout.houses[0];

    // Try to find a coastal (sand) location near the first house
    let coastal = find_coastal_plot(&map, hx, hy);
    let (shop_tile_x, shop_tile_y) = coastal.unwrap_or((hx + 4, hy + 5));

    info!(
        "[SHOP] Placed at ({}, {}){}",
        shop_tile_x, shop_tile_y,
        if coastal.is_some() { " (coastal)" } else { " (fallback)" },
    );

    commands.insert_resource(ShopLocation {
        tile_x: shop_tile_x,
        tile_y: shop_tile_y,
    });

    let wx = (shop_tile_x as f32 + 1.0) * TILE_SIZE;
    let wy = (shop_tile_y as f32 + 1.0) * TILE_SIZE;

    commands.spawn((
        Shop,
        Transform::from_xyz(wx, wy, 1.3),
        GlobalTransform::default(),
        Visibility::default(),
    ))
    .with_children(|parent| {
        spawn_shop_building(parent, assets.misc_shop.clone());
    });

    // Register in TileContent for UI
    for dy in 0..2 {
        for dx in 0..2 {
            let idx = (shop_tile_y + dy) * MAP_WIDTH + (shop_tile_x + dx);
            tile_content.data.entry(idx).or_default().push(TileEntry {
                name: "Shop",
                category: TileCategory::Building,
                amount: 0,
                w: 2,
                h: 2,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// AI: movement & decision
// ---------------------------------------------------------------------------

/// Compute a direction vector biased toward known paths and easier terrain.
/// Farm tiles are avoided unless the character is heading to one for farm work.
fn biased_dir(
    from: (f32, f32),
    to: (f32, f32),
    map: &Map,
    path_memory: &PathMemory,
    elevation: &[f64],
    sight_range: f32,
) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 2.0 {
        return (0.0, 0.0);
    }
    let mut dir_x = dx / dist;
    let mut dir_y = dy / dist;

    let (ctx, cty) = (
        (from.0 / TILE_SIZE) as isize,
        (from.1 / TILE_SIZE) as isize,
    );
    let mut nudge_x = 0.0f32;
    let mut nudge_y = 0.0f32;

    let sight = (sight_range as isize).max(1).min(10);
    for ddy in -sight..=sight {
        for ddx in -sight..=sight {
            if ddx == 0 && ddy == 0 { continue; }
            let tx = ctx + ddx;
            let ty = cty + ddy;
            if tx < 0 || tx >= MAP_WIDTH as isize || ty < 0 || ty >= MAP_HEIGHT as isize {
                continue;
            }
            let (tux, tuy) = (tx as usize, ty as usize);

            // Distance weight: closer tiles influence more
            let tile_dist = ((ddx * ddx + ddy * ddy) as f32).sqrt();
            let dist_weight = (1.0 / (tile_dist + 1.0)).max(0.05);

            // Direction alignment: tiles in the travel direction get bonus
            let align = if tile_dist > 0.01 {
                (ddx as f32 / tile_dist * dir_x + ddy as f32 / tile_dist * dir_y).max(0.0)
            } else {
                0.0
            };
            let combined_weight = dist_weight * (0.4 + align * 0.6);

            let terrain_speed = match map.tiles[tuy * MAP_WIDTH + tux] {
                TileType::Water | TileType::DeepWater | TileType::Lava => {
                    let rep = if tile_dist <= 3.0 { 10.0 } else { 2.0 };
                    nudge_x -= (tx - ctx) as f32 * rep * combined_weight;
                    nudge_y -= (ty - cty) as f32 * rep * combined_weight;
                    continue;
                }
                TileType::Grass | TileType::Meadow | TileType::Dirt => 1.0,
                TileType::Sand | TileType::Clay => 0.7,
                TileType::Desert => 0.5,
                TileType::Tundra => 0.25,
                TileType::Forest => 0.20,
                TileType::Ice => 0.20,
                TileType::Snow => 0.20,
                TileType::Swamp => 0.15,
                TileType::Stone => 0.20,
            };
            let mem = path_memory.counts[tuy * MAP_WIDTH + tux] as f32;
            let tile_elev = elevation[tuy * MAP_WIDTH + tux] as f32;
            // Elevation penalty: prefer valleys (up to 75% penalty)
            let elev_penalty = 1.0 - (tile_elev - 0.5).max(0.0) * 1.5;
            // Steepness penalty: avoid steep slopes
            let mut steep_penalty = 1.0f32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let nx = tux as isize + dx;
                    let ny = tuy as isize + dy;
                    if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                        let diff = (tile_elev - elevation[ny as usize * MAP_WIDTH + nx as usize] as f32).abs();
                        if diff * 6.0 > (1.0 - steep_penalty) { steep_penalty = 1.0 - diff * 6.0; }
                    }
                }
            }
            let weight = terrain_speed * (1.0 + (mem as f32).sqrt() * 2.0) * elev_penalty * steep_penalty * combined_weight;
            nudge_x += (tx - ctx) as f32 * weight;
            nudge_y += (ty - cty) as f32 * weight;
        }
    }

    dir_x += nudge_x * 0.008;
    dir_y += nudge_y * 0.008;
    let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
    if len > 0.01 {
        (dir_x / len, dir_y / len)
    } else {
        (dx / dist, dy / dist)
    }
}

// ---------------------------------------------------------------------------
// Perception — each character scans their sight range for interesting entities
// ---------------------------------------------------------------------------

fn perception_system(
    mut chars: Query<(Entity, &mut Character, &Transform)>,
    farm_tiles: Query<(Entity, &FarmTile, &Transform)>,
    graves: Query<(Entity, &GraveInfo, &Transform)>,
    tree_mask: Res<TreeMask>,
    shop_location: Res<ShopLocation>,
    _map: Res<Map>,
) {
    let farm_data: Vec<_> = farm_tiles
        .iter()
        .map(|(e, ft, tf)| {
            (
                e,
                ft.tile_x,
                ft.tile_y,
                ft.state,
                ft.plot,
                ft.growth,
                Vec2::new(tf.translation.x, tf.translation.y),
            )
        })
        .collect();

    let grave_data: Vec<_> = graves
        .iter()
        .map(|(e, gi, tf)| {
            (e, gi.house_id, Vec2::new(tf.translation.x, tf.translation.y))
        })
        .collect();

    let shop_pos = Vec2::new(
        shop_location.tile_x as f32 * TILE_SIZE + TILE_SIZE / 2.0,
        shop_location.tile_y as f32 * TILE_SIZE + TILE_SIZE / 2.0,
    );

    // Snapshot all character data before we start writing (immutable pass)
    let char_snapshot: Vec<(
        Entity,
        Vec2,
        f32,
        Gender,
        MaritalStatus,
        usize,
    )> = chars
        .iter()
        .map(|(e, ch, tf)| {
            (
                e,
                Vec2::new(tf.translation.x, tf.translation.y),
                ch.sight_range,
                ch.gender,
                ch.marital,
                ch.house_id,
            )
        })
        .collect();

    // Mutable pass: write percepts for each character
    for (entity, mut ch, tf) in chars.iter_mut() {
        let pos = Vec2::new(tf.translation.x, tf.translation.y);

        // Find this character's info from snapshot
        let my_info = char_snapshot
            .iter()
            .find(|(e, _, _, _, _, _)| *e == entity);
        let Some(&(_, _, sight_range, _, _, _)) = my_info else {
            continue;
        };

        let sight_sq = (sight_range * TILE_SIZE).powi(2);
        let mut percepts = Vec::with_capacity(16);

        // Farm tiles within sight
        for &(f_entity, tx, ty, state, plot, growth, ref ft_pos) in &farm_data {
            if pos.distance_squared(*ft_pos) <= sight_sq {
                percepts.push(Percept::FarmTile {
                    entity: f_entity,
                    tile_x: tx,
                    tile_y: ty,
                    state,
                    plot,
                    growth,
                });
            }
        }

        // Shop within sight
        if pos.distance_squared(shop_pos) <= sight_sq {
            percepts.push(Percept::Shop);
        }

        // Other characters within sight (from snapshot)
        for &(other_e, other_pos, _, other_gender, other_marital, other_house) in &char_snapshot
        {
            if other_e == entity {
                continue;
            }
            if pos.distance_squared(other_pos) <= sight_sq {
                percepts.push(Percept::Character {
                    entity: other_e,
                    gender: other_gender,
                    marital: other_marital,
                    house_id: other_house,
                });
            }
        }

        // Graves within sight
        for &(_, house_id, ref gpos) in &grave_data {
            if pos.distance_squared(*gpos) <= sight_sq {
                let tile_x = (gpos.x / TILE_SIZE) as usize;
                let tile_y = (gpos.y / TILE_SIZE) as usize;
                percepts.push(Percept::Grave {
                    house_id,
                    tile_x,
                    tile_y,
                });
            }
        }

        // Trees in 5-tile radius
        let tile_x = (pos.x / TILE_SIZE) as usize;
        let tile_y = (pos.y / TILE_SIZE) as usize;
        let mut tree_count = 0usize;
        let mut sum_tx = 0usize;
        let mut sum_ty = 0usize;
        for dy in -5..=5isize {
            for dx in -5..=5isize {
                let nx = tile_x as isize + dx;
                let ny = tile_y as isize + dy;
                if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                    if tree_mask.0[ny as usize * MAP_WIDTH + nx as usize] {
                        tree_count += 1;
                        sum_tx += nx as usize;
                        sum_ty += ny as usize;
                    }
                }
            }
        }
        if tree_count >= 2 {
            percepts.push(Percept::TreesNearby {
                count: tree_count,
                center_tx: sum_tx / tree_count,
                center_ty: sum_ty / tree_count,
            });
        }

        ch.percepts = percepts;
    }
}

// ---------------------------------------------------------------------------
// Decision — idle characters pick a task based on their percepts
// ---------------------------------------------------------------------------

/// Returns true if a tile has 2+ water/ deep-water cardinal neighbours,
/// which makes it unreachable via biased_dir pathfinding (coastal tile).
fn is_coastal_tile(tx: usize, ty: usize, map: &Map) -> bool {
    let mut wc = 0i32;
    for (dx, dy) in &[(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
        let nx = tx as isize + dx;
        let ny = ty as isize + dy;
        if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
            match map.tiles[ny as usize * MAP_WIDTH + nx as usize] {
                TileType::Water | TileType::DeepWater => wc += 1,
                _ => {}
            }
        }
    }
    wc >= 2
}

fn decision_system(
    mut chars: Query<(&mut Character, &Transform)>,
    houses: Query<&House>,
    _graves: Query<&GraveInfo>,
    shop_location: Res<ShopLocation>,
    pending_farmland: Res<PendingFarmland>,
    map: Res<Map>,
    road_wear: Res<RoadWear>,
) {
    let (adult_counts, oldest_two): (
        std::collections::HashMap<usize, usize>,
        std::collections::HashMap<usize, (f64, f64)>,
    ) = {
        let mut ac = std::collections::HashMap::new();
        let mut ot = std::collections::HashMap::new();
        for (ch, _) in chars.iter() {
            if ch.stage == LifeStage::Adult {
                *ac.entry(ch.house_id).or_insert(0) += 1;
                let pair = ot.entry(ch.house_id).or_insert((0.0, 0.0));
                if ch.age > pair.0 {
                    pair.1 = pair.0;
                    pair.0 = ch.age;
                } else if ch.age > pair.1 {
                    pair.1 = ch.age;
                }
            }
        }
        (ac, ot)
    };

    for (mut ch, tf) in chars.iter_mut() {
        // Only idle characters make decisions
        if ch.action_tile.is_some() {
            continue;
        }
        if ch.state != AiState::Idle {
            continue;
        }
        if ch.timer > 0.0 {
            continue;
        }

        let pos = (tf.translation.x, tf.translation.y);

        // --- Children: wander near home ---
        if ch.stage == LifeStage::Child {
            if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                let wx = hx + (rand::random::<f32>() - 0.5) * 200.0;
                let wy = hy + (rand::random::<f32>() - 0.5) * 200.0;
                ch.state = AiState::MoveTo(wx, wy, false);
            }
            ch.timer = 2.0;
            continue;
        }

        // --- Adult decision priority ---
        let house_food = houses
            .iter()
            .find(|h| h.id == ch.house_id)
            .map(|h| h.storage)
            .unwrap_or(0);
        let house_ess = houses
            .iter()
            .find(|h| h.id == ch.house_id)
            .map(|h| h.essentials)
            .unwrap_or(0);

        // Priority 1: essentials low → go to shop
        if house_ess <= ESSENTIALS_LOW_THRESHOLD && house_food >= SHOP_COST_FOOD {
            ch.state = AiState::GoingToShop;
            ch.timer = 2.0;
            continue;
        }

        // Priority 2: low personal food (<3) and house has food → go home to eat
        if ch.food < 3 {
            if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                if house.storage > 0 {
                    let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                    let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                    if dist_to(pos, (hx, hy)) > HOME_FOOD_RANGE {
                        ch.state = AiState::MoveTo(hx, hy, true);
                        ch.timer = 10.0;
                        continue;
                    }
                }
            }
        }

        // Priority 3: emigration — married adult in crowded house with plenty food
        let adults_in_house = adult_counts.get(&ch.house_id).copied().unwrap_or(0);
        let second_oldest = oldest_two
            .get(&ch.house_id)
            .copied()
            .unwrap_or((0.0, 0.0))
            .1;
        if matches!(ch.marital, MaritalStatus::Married)
            && adults_in_house > 2
            && ch.age >= CHILD_GROWTH_DURATION
            && ch.age < MAX_REPRODUCTION_AGE
            && ch.age < second_oldest
            && house_food > 300
            && rand::random::<f32>() < 0.03
        {
            info!(
                "[EMIGRATE] House #{} adult (age {:.0}) leaving (adults: {})",
                ch.house_id, ch.age, adults_in_house,
            );
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            ch.state = AiState::Exploring {
                origin_x: pos.0,
                origin_y: pos.1,
                dir_x: angle.cos(),
                dir_y: angle.sin(),
            };
            ch.timer = 2.0;
            continue;
        }

        // Priority 4+: farm work — use PERCEPTS instead of global query
        let farm_percepts: Vec<&Percept> = ch
            .percepts
            .iter()
            .filter(|p| matches!(p, Percept::FarmTile { plot, .. } if *plot == ch.plot_id))
            .collect();

        let find_available =
            |wanted: CropState| -> Option<(usize, usize)> {
                let candidates: Vec<&&Percept> = farm_percepts
                    .iter()
                    .filter(|p| {
                        matches!(p, Percept::FarmTile { state, growth, .. }
                            if *state == wanted && (wanted != CropState::Ready || *growth >= MIN_READY_TIME))
                    })
                    .filter(|p| {
                        // Skip coastal tiles (2+ water neighbours) that biased_dir can't reach
                        !matches!(p, Percept::FarmTile { tile_x, tile_y, .. }
                            if is_coastal_tile(*tile_x, *tile_y, &map))
                    })
                    .filter(|p| {
                        // Skip recently failed destinations (blacklisted via RoadWear)
                        !matches!(p, Percept::FarmTile { tile_x, tile_y, .. }
                            if road_wear.is_failed(*tile_x, *tile_y))
                    })
                    .collect();
                if candidates.is_empty() {
                    return None;
                }
                if wanted == CropState::Ready {
                    // Nearest ready tile
                    candidates
                        .iter()
                        .min_by(|a, b| {
                            let da = match (a, b) {
                                (
                                    Percept::FarmTile { tile_x: ax, tile_y: ay, .. },
                                    Percept::FarmTile { tile_x: bx, tile_y: by, .. },
                                ) => {
                                    let da = dist_to(
                                        pos,
                                        (
                                            *ax as f32 * TILE_SIZE + TILE_SIZE / 2.0,
                                            *ay as f32 * TILE_SIZE + TILE_SIZE / 2.0,
                                        ),
                                    );
                                    let db = dist_to(
                                        pos,
                                        (
                                            *bx as f32 * TILE_SIZE + TILE_SIZE / 2.0,
                                            *by as f32 * TILE_SIZE + TILE_SIZE / 2.0,
                                        ),
                                    );
                                    da.partial_cmp(&db).unwrap()
                                }
                                _ => std::cmp::Ordering::Equal,
                            };
                            da
                        })
                        .and_then(|p| match p {
                            Percept::FarmTile {
                                tile_x, tile_y, ..
                            } => Some((*tile_x, *tile_y)),
                            _ => None,
                        })
                } else {
                    // Random
                    let idx =
                        (rand::random::<f32>() * candidates.len() as f32) as usize;
                    let p = candidates[idx];
                    match p {
                        Percept::FarmTile {
                            tile_x, tile_y, ..
                        } => Some((*tile_x, *tile_y)),
                        _ => None,
                    }
                }
            };

        // Check pending farmland for expansion
        let has_pending = pending_farmland
            .plots
            .get(&ch.plot_id)
            .map(|t| !t.is_empty())
            .unwrap_or(false);
        let fallow_count = farm_percepts
            .iter()
            .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Fallow))
            .count();
        let should_expand = has_pending
            && house_food > 200
            && fallow_count <= 2
            && rand::random::<f32>() < 0.15;

        // Priority: Ready > expand (if food high) > Fallow > Weedy > pending clear
        let target = find_available(CropState::Ready)
            .or_else(|| {
                if should_expand {
                    pending_farmland
                        .plots
                        .get(&ch.plot_id)
                        .and_then(|tiles| tiles.iter().copied().find(|(tx, ty)| !is_coastal_tile(*tx, *ty, &map)))
                } else {
                    None
                }
            })
            .or_else(|| find_available(CropState::Fallow))
            .or_else(|| find_available(CropState::Weedy))
            .or_else(|| {
                pending_farmland
                    .plots
                    .get(&ch.plot_id)
                    .and_then(|tiles| tiles.iter().copied().find(|(tx, ty)| !is_coastal_tile(*tx, *ty, &map)))
            });

        if let Some((tx, ty)) = target {
            let (wx, wy) = tile_center(tx, ty);
            ch.state = AiState::MoveTo(wx, wy, true);
            ch.timer = 10.0;
            continue;
        }

        // Diagnostic: log when Ready tiles exist but nothing assigned
        let ready_count = farm_percepts.iter()
            .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Ready))
            .count();
        if ready_count > 0 {
            info!(
                "[DIAG] House #{} on ({:.0},{:.0}) — {} ready, target=None, action={}, state={:?}, timer={:.3}",
                ch.house_id, pos.0, pos.1, ready_count,
                ch.action_tile.is_some(),
                ch.state,
                ch.timer,
            );
        }

        // No farm work — grave visit? (less likely when farm land needs attention)
        let has_family_grave = _graves.iter().any(|gi| gi.house_id == ch.house_id);
        if has_family_grave && rand::random::<f32>() < 0.05 {
            let grave_cx =
                (GRAVEYARD_X + GRAVEYARD_W / 2) as f32 * TILE_SIZE + TILE_SIZE / 2.0;
            let grave_cy =
                (GRAVEYARD_Y + GRAVEYARD_H / 2) as f32 * TILE_SIZE + TILE_SIZE / 2.0;
            let ox = (rand::random::<f32>() - 0.5) * 64.0;
            let oy = (rand::random::<f32>() - 0.5) * 64.0;
            ch.state = AiState::GoingToSocial(grave_cx + ox, grave_cy + oy);
            ch.timer = 3.0;
            continue;
        }

        // Farm work detected (but no specific tile targeted)? Retry, don't socialise/explore.
        let exploring_ready = farm_percepts
            .iter()
            .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Ready))
            .count();
        let exploring_fallow = farm_percepts
            .iter()
            .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Fallow))
            .count();
        let exploring_weedy = farm_percepts
            .iter()
            .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Weedy))
            .count();

        if exploring_ready > 0 || exploring_fallow > 0 || exploring_weedy > 0 {
            // Farm work exists but no tile available yet — pause then retry.
            // Check more frequently when Ready tiles are maturing.
            ch.timer = if exploring_ready > 0 { 0.25 } else { 1.0 };
            continue;
        }

        // No farm work at all — socialise or explore
        if ch.marital == MaritalStatus::Single && rand::random::<f32>() < 0.15 {
            let shop_cx =
                shop_location.tile_x as f32 * TILE_SIZE + TILE_SIZE;
            let shop_cy =
                shop_location.tile_y as f32 * TILE_SIZE + TILE_SIZE;
            let ox = (rand::random::<f32>() - 0.5) * 64.0;
            let oy = (rand::random::<f32>() - 0.5) * 64.0;
            ch.state = AiState::GoingToSocial(shop_cx + ox, shop_cy + oy);
            ch.timer = 3.0;
        } else {
            if house_food >= 10 {
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                ch.state = AiState::Exploring {
                    origin_x: pos.0,
                    origin_y: pos.1,
                    dir_x: angle.cos(),
                    dir_y: angle.sin(),
                };
                ch.timer = 2.0;
            } else if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                // Only go home if not already there — prevents no-op MoveTo loop
                if dist_to(pos, (hx, hy)) > TILE_SIZE {
                    ch.state = AiState::MoveTo(hx, hy, true);
                    ch.timer = 5.0;
                } else {
                    ch.state = AiState::Idle;
                    ch.timer = 5.0;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Movement — execute state-machine motion for all characters
// ---------------------------------------------------------------------------

fn movement_system(
    fixed_time: Res<Time<Fixed>>,
    mut commands: Commands,
    mut chars: Query<(&mut Character, &mut Transform)>,
    farm_tiles: Query<(&FarmTile, &Transform), Without<Character>>,
    houses: Query<&House>,
    map: Res<Map>,
    elevation: Res<ElevationMap>,
    mut next_id: ResMut<NextSettlementId>,
    shop_location: Res<ShopLocation>,
    mut road_wear: ResMut<RoadWear>,
    mut path_memory: ResMut<PathMemory>,
    mut pending_farmland: ResMut<PendingFarmland>,
    mut events: EventWriter<ActionEvent>,
    assets: Res<GameAssets>,
    tree_mask: Res<TreeMask>,
) {
    let farm_set: std::collections::HashSet<(usize, usize)> = farm_tiles
        .iter()
        .map(|(ft, _)| (ft.tile_x, ft.tile_y))
        .collect();

    let road_mask: Vec<bool> =
        road_wear.wear.iter().map(|&w| w >= ROAD_THRESHOLD_1).collect();

    let existing: Vec<(usize, usize)> = {
        let mut list: Vec<(usize, usize)> =
            farm_tiles.iter().map(|(ft, _)| (ft.tile_x, ft.tile_y)).collect();
        for house in houses.iter() {
            for dx in 0..house.w {
                for dy in 0..house.h {
                    list.push((house.tile_x + dx, house.tile_y + dy));
                }
            }
        }
        for tiles in pending_farmland.plots.values() {
            list.extend(tiles.iter().copied());
        }
        list
    };

    for (mut ch, mut tf) in chars.iter_mut() {
        let dt = fixed_time.delta_secs_f64();
        ch.timer -= dt;
        let pos = (tf.translation.x, tf.translation.y);

        // Water → swim to nearest shore.
        // Does NOT change ch.state — preserves the original destination so the
        // timeout handler (timer < -15.0) can blacklist it if the tile is
        // unreachable across water.  Movement is applied inline.
        let (wcx, wcy) = current_tile(&tf);
        if wcx < MAP_WIDTH && wcy < MAP_HEIGHT {
            if matches!(
                map.tiles[wcy * MAP_WIDTH + wcx],
                TileType::Water | TileType::DeepWater | TileType::Lava
            ) {
                ch.swim_streak += 1;

                // Safety valve: after 60 s of consecutive swimming, the
                // destination is unreachable.  Blacklist it and go explore.
                if ch.swim_streak > 1200 {
                    if let AiState::MoveTo(wx, wy, _) = ch.state {
                        let (dtx, dty) = ((wx / TILE_SIZE) as usize, (wy / TILE_SIZE) as usize);
                        road_wear.mark_failed(dtx, dty);
                        info!(
                            "[SWIM] #{} blacklisting ({},{}) after swim loop",
                            ch.house_id, dtx, dty,
                        );
                    }
                    let angle = rand::random::<f32>() * std::f32::consts::TAU;
                    ch.state = AiState::Exploring {
                        origin_x: pos.0, origin_y: pos.1,
                        dir_x: angle.cos(), dir_y: angle.sin(),
                    };
                    ch.timer = 15.0;
                    ch.swim_streak = 0;
                } else if let Some(shore) = nearest_shore(wcx, wcy, &map, 20) {
                    let (swx, swy) = tile_center(shore.0, shore.1);
                    warn!(
                        "[SWIM] #{} swim ({},{}) → shore ({},{})  streak={}",
                        ch.house_id, wcx, wcy, shore.0, shore.1, ch.swim_streak,
                    );
                    // Move toward shore without changing state or resetting timer
                    let dx = swx - tf.translation.x;
                    let dy = swy - tf.translation.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 1.0 {
                        let spd = ch.speed
                            * tile_speed_multiplier(&map, &tf, &road_wear.wear, &elevation.0)
                            * fixed_time.delta_secs();
                        tf.translation.x += (dx / dist) * spd;
                        tf.translation.y += (dy / dist) * spd;
                    } else {
                        // Close enough — snap to shore
                        tf.translation.x = swx;
                        tf.translation.y = swy;
                    }
                }
                continue;
            }
        }

        // Reset swim streak on dry land
        ch.swim_streak = 0;

        // Terrain stuck — react based on state when stuck for >10 s.
        let terrain_spd = tile_speed_multiplier(&map, &tf, &road_wear.wear, &elevation.0);
        if terrain_spd < 0.25 {
            ch.stuck_timer += dt;
            if ch.stuck_timer > 10.0 {
                match ch.state {
                    // MoveTo stuck → blacklist destination, escape to passable terrain
                    AiState::MoveTo(wx, wy, _) => {
                        let (dtx, dty) = ((wx / TILE_SIZE) as usize, (wy / TILE_SIZE) as usize);
                        road_wear.mark_failed(dtx, dty);
                        let (tx, ty) = current_tile(&tf);
                        if let Some((ex, ey)) = nearest_passable(tx, ty, &map, 15) {
                            let (ewx, ewy) = tile_center(ex, ey);
                            ch.state = AiState::MoveTo(ewx, ewy, false);
                            ch.stuck_timer = -10.0;
                            info!(
                                "[STUCK] #{} MoveTo blocked ({},{}) → passable ({},{})",
                                ch.house_id, tx, ty, ex, ey,
                            );
                        } else if let Some(h) = houses.iter().find(|hh| hh.id == ch.house_id) {
                            let hx = (h.tile_x as f32 + h.w as f32 / 2.0) * TILE_SIZE;
                            let hy = (h.tile_y as f32 + h.h as f32 / 2.0) * TILE_SIZE;
                            ch.state = AiState::MoveTo(hx, hy, false);
                            ch.stuck_timer = -10.0;
                        }
                    }
                    // Explorer stuck → steer downhill (lowest-elevation direction)
                    AiState::Exploring { .. } => {
                        let (tx, ty) = current_tile(&tf);
                        let mut best_dir = (0.0f32, 0.0f32);
                        let mut best_elev = f64::MAX;
                        for dy in -3..=3isize {
                            for dx in -3..=3isize {
                                if dx == 0 && dy == 0 { continue; }
                                let nx = tx as isize + dx;
                                let ny = ty as isize + dy;
                                if nx >= 0 && nx < MAP_WIDTH as isize
                                    && ny >= 0 && ny < MAP_HEIGHT as isize
                                {
                                    let e = elevation.0[ny as usize * MAP_WIDTH + nx as usize];
                                    if e < best_elev {
                                        best_elev = e;
                                        best_dir = (dx as f32, dy as f32);
                                    }
                                }
                            }
                        }
                        let len = (best_dir.0.powi(2) + best_dir.1.powi(2)).sqrt().max(0.001);
                        ch.state = AiState::Exploring {
                            origin_x: pos.0, origin_y: pos.1,
                            dir_x: best_dir.0 / len, dir_y: best_dir.1 / len,
                        };
                        info!(
                            "[STUCK] #{} explorer re-routed downhill from ({},{})",
                            ch.house_id, tx, ty,
                        );
                        ch.stuck_timer = -10.0;
                    }
                    // Idle / social / shop → escape to nearest passable tile
                    _ => {
                        let (tx, ty) = current_tile(&tf);
                        if let Some((ex, ey)) = nearest_passable(tx, ty, &map, 15) {
                            let (ewx, ewy) = tile_center(ex, ey);
                            ch.state = AiState::MoveTo(ewx, ewy, false);
                            ch.stuck_timer = -10.0;
                            info!(
                                "[STUCK] #{} escaping ({},{}) → passable ({},{})",
                                ch.house_id, tx, ty, ex, ey,
                            );
                        } else if let Some(h) = houses.iter().find(|hh| hh.id == ch.house_id) {
                            let hx = (h.tile_x as f32 + h.w as f32 / 2.0) * TILE_SIZE;
                            let hy = (h.tile_y as f32 + h.h as f32 / 2.0) * TILE_SIZE;
                            ch.state = AiState::MoveTo(hx, hy, false);
                            ch.stuck_timer = -10.0;
                        }
                    }
                }
            }
        } else {
            ch.stuck_timer = 0.0;
        }

        // Skip state machine if waiting for action processing
        // (water check and stuck check still run above)
        if ch.action_tile.is_some() {
            continue;
        }

        // MoveTo timeout: if character has been walking for >15 game-days
        // without arriving, give up and blacklist the destination.
        // This prevents biased_dir from trapping the character in oscillation
        // near difficult terrain or repeatedly targeting unreachable coastal tiles.
        if ch.timer < -15.0 {
            // Blacklist the destination tile so decision_system won't retry it
            if let AiState::MoveTo(wx, wy, _) = ch.state {
                let (dtx, dty) = ((wx / TILE_SIZE) as usize, (wy / TILE_SIZE) as usize);
                road_wear.mark_failed(dtx, dty);
            }
            if matches!(ch.state, AiState::MoveTo(..) | AiState::GoingToShop | AiState::GoingToSocial(..)) {
                info!(
                    "[TIMEOUT] House #{} giving up on {:?} (timer={:.1})",
                    ch.house_id, ch.state, ch.timer,
                );
                // Set to exploring so character moves to a new position,
                // getting out of the terrain that was blocking them.
                // After exploring, decision_system will re-evaluate.
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                ch.state = AiState::Exploring {
                    origin_x: pos.0, origin_y: pos.1,
                    dir_x: angle.cos(), dir_y: angle.sin(),
                };
                ch.timer = 10.0;
            }
        }

        match ch.state {
            AiState::Idle => {
                // handled by decision_system
            }

            AiState::MoveTo(wx, wy, purposeful) => {
                let dx = wx - pos.0;
                let dy = wy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();

                let tile_speed =
                    tile_speed_multiplier(&map, &tf, &road_wear.wear, &elevation.0);
                let step = ch.speed * tile_speed * fixed_time.delta_secs();

                if dist < 2.0 || dist <= step {
                    tf.translation.x = wx;
                    tf.translation.y = wy;

                    let (tx, ty) = current_tile(&tf);
                    let is_my_farm = farm_tiles.iter().any(|(ft, _)| {
                        ft.tile_x == tx && ft.tile_y == ty && ft.plot == ch.plot_id
                    });
                    let is_pending = pending_farmland
                        .plots
                        .get(&ch.plot_id)
                        .map(|tiles| tiles.contains(&(tx, ty)))
                        .unwrap_or(false);
                    if is_my_farm || is_pending {
                        ch.action_tile = Some((tx, ty));
                    } else {
                        ch.state = AiState::Idle;
                        // Longer rest if we just escaped from bad terrain
                        ch.timer = if ch.stuck_timer < 0.0 { 60.0 } else { 3.0 };
                        ch.stuck_timer = 0.0;
                    }
                } else {
                    let dest_tile_x = (wx / TILE_SIZE) as usize;
                    let dest_tile_y = (wy / TILE_SIZE) as usize;
                    let dest_is_farm = farm_set.contains(&(dest_tile_x, dest_tile_y));

                    let (bdx, bdy) = if dest_is_farm || tile_speed < 0.15 {
                        let len = (dx * dx + dy * dy).sqrt().max(0.001);
                        (dx / len, dy / len)
                    } else {
                        biased_dir(
                            (pos.0, pos.1),
                            (wx, wy),
                            &map,
                            &path_memory,
                            &elevation.0,
                            ch.sight_range,
                        )
                    };
                    tf.translation.x += bdx * step;
                    tf.translation.y += bdy * step;

                    if purposeful {
                        let (rtx, rty) = current_tile(&tf);
                        if rtx < MAP_WIDTH && rty < MAP_HEIGHT {
                            road_wear.wear[rty * MAP_WIDTH + rtx] += 2;
                            path_memory.counts[rty * MAP_WIDTH + rtx] += 1;
                        }
                    }
                }
            }

            AiState::GoingToShop => {
                let shop_tile_x = shop_location.tile_x;
                let shop_tile_y = shop_location.tile_y;
                let shop_wx =
                    shop_tile_x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let shop_wy =
                    shop_tile_y as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let dx = shop_wx - pos.0;
                let dy = shop_wy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();

                let tile_speed =
                    tile_speed_multiplier(&map, &tf, &road_wear.wear, &elevation.0);
                let step = ch.speed * tile_speed * fixed_time.delta_secs();

                if dist < 2.0 || dist <= step {
                    tf.translation.x = shop_wx;
                    tf.translation.y = shop_wy;
                    ch.action_tile = Some((shop_tile_x, shop_tile_y));
                } else {
                    let (bdx, bdy) = if tile_speed < 0.15 {
                        // Raw direction on slow terrain (avoid biased_dir cancel-out)
                        let len = (dx * dx + dy * dy).sqrt().max(0.001);
                        (dx / len, dy / len)
                    } else {
                        biased_dir(
                            (pos.0, pos.1),
                            (shop_wx, shop_wy),
                            &map,
                            &path_memory,
                            &elevation.0,
                            ch.sight_range,
                        )
                    };
                    tf.translation.x += bdx * step;
                    tf.translation.y += bdy * step;

                    let (rtx, rty) = current_tile(&tf);
                    if rtx < MAP_WIDTH && rty < MAP_HEIGHT {
                        road_wear.wear[rty * MAP_WIDTH + rtx] += 2;
                        path_memory.counts[rty * MAP_WIDTH + rtx] += 1;
                    }
                }
            }

            AiState::GoingToSocial(wx, wy) => {
                let dx = wx - pos.0;
                let dy = wy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                let tile_speed =
                    tile_speed_multiplier(&map, &tf, &road_wear.wear, &elevation.0);
                let step = ch.speed * tile_speed * fixed_time.delta_secs();
                if dist < 2.0 || dist <= step {
                    tf.translation.x = wx;
                    tf.translation.y = wy;
                    ch.state = AiState::Socializing;
                    ch.timer = 8.0 + rand::random::<f64>() * 8.0;
                } else {
                    let (bdx, bdy) = if tile_speed < 0.15 {
                        // Raw direction on slow terrain
                        let len = (dx * dx + dy * dy).sqrt().max(0.001);
                        (dx / len, dy / len)
                    } else {
                        biased_dir(
                            (pos.0, pos.1),
                            (wx, wy),
                            &map,
                            &path_memory,
                            &elevation.0,
                            ch.sight_range,
                        )
                    };
                    tf.translation.x += bdx * step;
                    tf.translation.y += bdy * step;
                }
            }

            AiState::Socializing => {
                ch.timer -= dt;
                if ch.timer <= 0.0 {
                    if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                        let hx =
                            (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                        let hy =
                            (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                        ch.state = AiState::MoveTo(hx, hy, false);
                    } else {
                        ch.state = AiState::Idle;
                    }
                    ch.timer = 2.0;
                }
            }

            AiState::Exploring {
                origin_x,
                origin_y,
                dir_x,
                dir_y,
            } => {
                let tile_speed =
                    tile_speed_multiplier(&map, &tf, &road_wear.wear, &elevation.0);
                let speed = ch.speed * tile_speed;
                tf.translation.x += dir_x * speed * fixed_time.delta_secs();
                tf.translation.y += dir_y * speed * fixed_time.delta_secs();

                // Hit water → abort
                let (wtx, wty) = current_tile(&tf);
                if wtx < MAP_WIDTH && wty < MAP_HEIGHT {
                    if matches!(
                        map.tiles[wty * MAP_WIDTH + wtx],
                        TileType::Water | TileType::DeepWater
                    ) {
                        if let Some(house) =
                            houses.iter().find(|h| h.id == ch.house_id)
                        {
                            let hx = (house.tile_x as f32 + house.w as f32 / 2.0)
                                * TILE_SIZE;
                            let hy = (house.tile_y as f32 + house.h as f32 / 2.0)
                                * TILE_SIZE;
                            ch.state = AiState::MoveTo(hx, hy, false);
                        } else {
                            ch.state = AiState::Idle;
                            ch.timer = 60.0;
                        }
                        continue;
                    }
                }

                // Look ahead for water (3 tiles)
                let look_ahead = 3;
                let mut water_ahead = false;
                for step in 1..=look_ahead {
                    let lx = (tf.translation.x + dir_x * step as f32 * TILE_SIZE)
                        / TILE_SIZE;
                    let ly = (tf.translation.y + dir_y * step as f32 * TILE_SIZE)
                        / TILE_SIZE;
                    let lxi = lx.floor() as isize;
                    let lyi = ly.floor() as isize;
                    if lxi >= 0
                        && lxi < MAP_WIDTH as isize
                        && lyi >= 0
                        && lyi < MAP_HEIGHT as isize
                    {
                        let li = lyi as usize * MAP_WIDTH + lxi as usize;
                        if matches!(
                            map.tiles[li],
                            TileType::Water | TileType::DeepWater
                        ) {
                            water_ahead = true;
                            break;
                        }
                    }
                }
                if water_ahead {
                    if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                        let hx = (house.tile_x as f32 + house.w as f32 / 2.0)
                            * TILE_SIZE;
                        let hy = (house.tile_y as f32 + house.h as f32 / 2.0)
                            * TILE_SIZE;
                        ch.state = AiState::MoveTo(hx, hy, false);
                    } else {
                        ch.state = AiState::Idle;
                        ch.timer = 60.0;
                    }
                    continue;
                }

                // Too far from origin → go home
                let (cx, cy) = (tf.translation.x, tf.translation.y);
                let dist =
                    ((cx - origin_x).powi(2) + (cy - origin_y).powi(2)).sqrt();
                if dist > 1200.0 {
                    if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                        let hx = (house.tile_x as f32 + house.w as f32 / 2.0)
                            * TILE_SIZE;
                        let hy = (house.tile_y as f32 + house.h as f32 / 2.0)
                            * TILE_SIZE;
                        ch.state = AiState::MoveTo(hx, hy, false);
                    } else {
                        ch.state = AiState::Idle;
                        ch.timer = 5.0;
                    }
                    continue;
                }

                // Periodic check: food supply + trees → found new settlement?
                ch.timer -= dt;
                if ch.timer <= 0.0 {
                    ch.timer = 1.5;

                    let home_food = houses
                        .iter()
                        .find(|h| h.id == ch.house_id)
                        .map(|h| h.storage)
                        .unwrap_or(0);
                    if home_food < 20 {
                        if let Some(house) =
                            houses.iter().find(|h| h.id == ch.house_id)
                        {
                            let hx = (house.tile_x as f32 + house.w as f32 / 2.0)
                                * TILE_SIZE;
                            let hy = (house.tile_y as f32 + house.h as f32 / 2.0)
                                * TILE_SIZE;
                            ch.state = AiState::MoveTo(hx, hy, true);
                        } else {
                            ch.state = AiState::Idle;
                            ch.timer = 5.0;
                        }
                        continue;
                    }

                    // Periodic check: go home to check for farm work
                    if rand::random::<f32>() < 0.15 {
                        if let Some(house) =
                            houses.iter().find(|h| h.id == ch.house_id)
                        {
                            let hx = (house.tile_x as f32 + house.w as f32 / 2.0)
                                * TILE_SIZE;
                            let hy = (house.tile_y as f32 + house.h as f32 / 2.0)
                                * TILE_SIZE;
                            ch.state = AiState::MoveTo(hx, hy, false);
                            continue;
                        }
                    }

                    // Check tree density from cached TreeMask
                    let (tx, ty) = current_tile(&tf);
                    let mut tree_count = 0;
                    for dy in -5..=5isize {
                        for dx in -5..=5isize {
                            let nx = tx as isize + dx;
                            let ny = ty as isize + dy;
                            if nx >= 0
                                && nx < MAP_WIDTH as isize
                                && ny >= 0
                                && ny < MAP_HEIGHT as isize
                                && tree_mask.0
                                    [ny as usize * MAP_WIDTH + nx as usize]
                            {
                                tree_count += 1;
                            }
                        }
                    }

                    if tree_count >= 4 {
                        let (mx, my) = tile_center(tx, ty);
                        if let Some((plot, house_tile, char_tile)) =
                            find_expansion_site(
                                &map,
                                &tree_mask.0,
                                &existing,
                                mx,
                                my,
                                10,
                                &road_mask,
                            )
                        {
                            let sid = next_id.0;
                            next_id.0 += 1;

                            let mut pending = Vec::new();
                            for (i, &(fx, fy)) in plot.iter().enumerate() {
                                if i < 2 {
                                    let wx = fx as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                                    let wy = fy as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                                    commands.spawn((
                                        FarmTile {
                                            plot: sid,
                                            tile_x: fx,
                                            tile_y: fy,
                                            state: CropState::Fallow,
                                            growth: 0.0,
                                        },
                                        Sprite {
                                            image: assets.misc_farm_fallow.clone(),
                                            custom_size: Some(Vec2::new(
                                                TILE_SIZE - 2.0,
                                                TILE_SIZE - 2.0,
                                            )),
                                            ..default()
                                        },
                                        Transform::from_xyz(wx, wy, 1.0),
                                        GlobalTransform::default(),
                                        Visibility::default(),
                                    ));
                                } else {
                                    pending.push((fx, fy));
                                }
                            }
                            if !pending.is_empty() {
                                pending_farmland.plots.insert(sid, pending);
                            }

                            let (hx, hy) =
                                tile_center(house_tile.0, house_tile.1);
                            commands.spawn((
                                House {
                                    id: sid,
                                    tile_x: house_tile.0,
                                    tile_y: house_tile.1,
                                    w: 1,
                                    h: 1,
                                    storage: 40,
                                    essentials: HOUSE_START_ESSENTIALS / 2,
                                },
                                Transform::from_xyz(hx, hy, 1.3),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| {
                                spawn_house_building(
                                    parent,
                                    assets.bld_house.clone(),
                                )
                            });

                            events.send(ActionEvent::Emigrate {
                                house_id: ch.house_id,
                                food_amount: 80,
                            });

                            let (cx, cy) =
                                tile_center(char_tile.0, char_tile.1);
                            let couple_offset = 16.0;

                            // Male settler
                            commands.spawn((
                                Character {
                                    speed: 100.0,
                                    state: AiState::Idle,
                                    timer: 1.0,
                                    action_tile: None,
                                    percepts: Vec::new(),
                                    stuck_timer: 0.0,
                                    plot_id: sid,
                                    house_id: sid,
                                    stage: LifeStage::Adult,
                                    age: (rand::random::<f64>() * 10.0 + 20.0)
                                        * YEAR,
                                    gender: Gender::Male,
                                    personality: Personality::random(),
                                    marital: MaritalStatus::Married,
                                    sight_range: 8.0,
                                    food: 10,
                                    swim_streak: 0,
                                },
                                Transform::from_xyz(cx - couple_offset, cy, 2.0),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| {
                                spawn_character_sprite(
                                    parent,
                                    assets.char_male.clone(),
                                )
                            });

                            // Female settler (spouse)
                            commands.spawn((
                                Character {
                                    speed: 100.0,
                                    state: AiState::Idle,
                                    timer: 1.5,
                                    action_tile: None,
                                    percepts: Vec::new(),
                                    stuck_timer: 0.0,
                                    plot_id: sid,
                                    house_id: sid,
                                    stage: LifeStage::Adult,
                                    age: (rand::random::<f64>() * 10.0 + 20.0)
                                        * YEAR,
                                    gender: Gender::Female,
                                    personality: Personality::random(),
                                    marital: MaritalStatus::Married,
                                    sight_range: 8.0,
                                    food: 10,
                                    swim_streak: 0,
                                },
                                Transform::from_xyz(cx + couple_offset, cy, 2.0),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| {
                                spawn_character_sprite(
                                    parent,
                                    assets.char_female.clone(),
                                )
                            });

                            // Explorer returns home
                            if let Some(house) =
                                houses.iter().find(|h| h.id == ch.house_id)
                            {
                                let hx = (house.tile_x as f32
                                    + house.w as f32 / 2.0)
                                    * TILE_SIZE;
                                let hy = (house.tile_y as f32
                                    + house.h as f32 / 2.0)
                                    * TILE_SIZE;
                                ch.state = AiState::MoveTo(hx, hy, false);
                            }
                        }
                    }
                }
            }
        }

        // Z elevation
        let (czx, czy) = current_tile(&tf);
        if czx < MAP_WIDTH && czy < MAP_HEIGHT {
            let elev_z = elevation.0[czy * MAP_WIDTH + czx] as f32;
            tf.translation.z = 2.0 + elev_z * 4.0;
        }
    }
}

// ---------------------------------------------------------------------------
// OLD character_ai removed — replaced by perception_system / decision_system / movement_system
// ---------------------------------------------------------------------------

// (old character_ai body removed)


// ---------------------------------------------------------------------------
// Action — handle arrival at farm/shop tiles
// ---------------------------------------------------------------------------

pub fn action_system(
    mut commands: Commands,
    mut chars: Query<(&mut Character, &Transform)>,
    farm_tiles: Query<&FarmTile>,
    houses: Query<&House>,
    mut events: EventWriter<ActionEvent>,
    shop_location: Res<ShopLocation>,
    mut pending_farmland: ResMut<PendingFarmland>,
    assets: Res<GameAssets>,
) {
    for (mut ch, _tf) in chars.iter_mut() {
        let Some((tx, ty)) = ch.action_tile.take() else {
            continue;
        };

        // Is this a shop visit?
        if tx == shop_location.tile_x && ty == shop_location.tile_y {
            let mut home_pos = None;
            if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                if house.storage >= SHOP_COST_FOOD {
                    // Can't modify house directly since we only have read access
                    // Use ActionEvent instead
                    home_pos = Some((
                        (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE,
                        (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE,
                    ));
                } else {
                    home_pos = Some((
                        (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE,
                        (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE,
                    ));
                }
            }
            // Emit shop event for the action handler to process
            events.send(ActionEvent::ShopTrade {
                house_id: ch.house_id,
            });
            if let Some((hx, hy)) = home_pos {
                ch.state = AiState::MoveTo(hx, hy, true);
            } else {
                ch.state = AiState::Idle;
            }
            ch.timer = 2.0;
            continue;
        }

        // Is this a pending (not yet spawned) tile?
        let is_pending = pending_farmland.plots.get(&ch.plot_id)
            .map(|tiles| tiles.contains(&(tx, ty)))
            .unwrap_or(false);

        if is_pending {
            // Spawn a Clearing-state farm tile so the character clears it
            let world_x = tx as f32 * TILE_SIZE + TILE_SIZE / 2.0;
            let world_y = ty as f32 * TILE_SIZE + TILE_SIZE / 2.0;
            commands.spawn((
                FarmTile {
                    plot: ch.plot_id,
                    tile_x: tx,
                    tile_y: ty,
                    state: CropState::Clearing,
                    growth: 0.0,
                },
                Sprite {
                    image: assets.misc_farm_fallow.clone(),
                    custom_size: Some(Vec2::new(TILE_SIZE - 2.0, TILE_SIZE - 2.0)),
                    color: color_for_clearing(0.0),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, 1.0),
                GlobalTransform::default(),
                Visibility::default(),
            ));

            // Remove this tile from pending
            if let Some(tiles) = pending_farmland.plots.get_mut(&ch.plot_id) {
                tiles.retain(|&p| p != (tx, ty));
            }

            ch.state = AiState::Idle;
            ch.timer = 2.0;
            continue;
        }

        // Verify this is a valid farm tile
        let _plot = farm_tiles
            .iter()
            .find(|ft| ft.tile_x == tx && ft.tile_y == ty)
            .map(|ft| ft.plot);

        let Some(tile_info) = farm_tiles
            .iter()
            .find(|ft| ft.tile_x == tx && ft.tile_y == ty)
        else {
            ch.state = AiState::Idle;
            ch.timer = 2.0;
            continue;
        };

        // Don't harvest Ready tiles that haven't reached MIN_READY_TIME yet.
        // (decision_system already filters these, but this guards against
        // edge cases where growth changed during the walk.)
        if tile_info.state == CropState::Ready && tile_info.growth < MIN_READY_TIME {
            info!(
                "[DIAG] House #{} arrived at Ready tile ({},{}) growth={:.2} — waiting",
                ch.house_id, tx, ty, tile_info.growth,
            );
            ch.state = AiState::Idle;
            ch.timer = 0.0;
            continue;
        }

        // Single-tile action: the handler will toggle this one tile
        events.send(ActionEvent::FarmInteract {
            tile_x: tx,
            tile_y: ty,
            house_id: Some(ch.house_id),
        });

        // Carried food from farm work
        ch.food = ch.food.saturating_add(1);

        // Immediately look for the next task
        ch.state = AiState::Idle;
        ch.timer = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Population
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct ReproductionTimer(pub f64);

/// Sim-seconds between romance checks (6 months).
const ROMANCE_INTERVAL: f64 = 0.5 * YEAR;
/// Personality compatibility threshold (0.0–1.0) for romance to succeed.
const ROMANCE_THRESHOLD: f32 = 0.25;
/// Base romance probability per tick for compatible pairs.
const ROMANCE_BASE_CHANCE: f64 = 0.6;

#[derive(Resource, Default)]
pub struct RomanceTimer(pub f64);

/// How compatible two personalities are (1.0 = perfect match).
fn personality_compatibility(a: &Personality, b: &Personality) -> f32 {
    let diffs = [
        (a.openness - b.openness).abs(),
        (a.conscientiousness - b.conscientiousness).abs(),
        (a.extraversion - b.extraversion).abs(),
        (a.agreeableness - b.agreeableness).abs(),
        (a.neuroticism - b.neuroticism).abs(),
    ];
    1.0 - diffs.iter().sum::<f32>() / diffs.len() as f32
}

/// Romance system: single adults find partners based on personality compatibility.
fn romance_system(
    fixed_time: Res<Time<Fixed>>,
    mut timer: ResMut<RomanceTimer>,
    mut chars: Query<(Entity, &mut Character)>,
) {
    let dt = fixed_time.delta_secs_f64();
    timer.0 += dt;
    if timer.0 < ROMANCE_INTERVAL {
        return;
    }
    timer.0 -= ROMANCE_INTERVAL;

    // Collect all single adults: (entity, house_id, gender, personality)
    let singles: Vec<(Entity, usize, Gender, Personality)> = chars
        .iter()
        .filter(|(_, ch)| ch.stage == LifeStage::Adult && ch.marital == MaritalStatus::Single)
        .map(|(e, ch)| (e, ch.house_id, ch.gender, ch.personality))
        .collect();

    // Track which entities to marry this tick
    let mut to_marry: Vec<Entity> = Vec::new();

    for (i, &(ei, hi, gi, ref pi)) in singles.iter().enumerate() {
        if to_marry.contains(&ei) {
            continue;
        }
        // Find best compatible partner — allow same house (grown children can pair)
        let mut best_compat = 0.0f32;
        let mut best_j = None;
        for (j, &(_ej, _hj, gj, ref pj)) in singles.iter().enumerate() {
            if i == j || gj == gi { continue; }
            let compat = personality_compatibility(pi, pj);
            if compat > best_compat {
                best_compat = compat;
                best_j = Some(j);
            }
        }

        if let Some(j) = best_j {
            if best_compat >= ROMANCE_THRESHOLD {
                let chance = ROMANCE_BASE_CHANCE * best_compat as f64;
                if rand::random::<f64>() < chance {
                    let ej = singles[j].0;
                    if !to_marry.contains(&ej) {
                        let hj = singles[j].1;
                        info!(
                            "[ROMANCE] Singles paired (compat: {:.2}, houses: {} & {}), starting new household",
                            best_compat, hi, hj,
                        );
                        to_marry.push(ei);
                        to_marry.push(ej);
                    }
                }
            }
        }
    }

    // Apply marriage
    for (entity, mut ch) in chars.iter_mut() {
        if to_marry.contains(&entity) {
            ch.marital = MaritalStatus::Married;
        }
    }
}

fn reproduction_system(
    fixed_time: Res<Time<Fixed>>,
    mut timer: ResMut<ReproductionTimer>,
    chars: Query<&Character>,
    houses: Query<&House>,
    mut commands: Commands,
    assets: Res<GameAssets>,
) {
    timer.0 += fixed_time.delta_secs_f64();
    if timer.0 < CHILD_BIRTH_INTERVAL {
        return;
    }
    timer.0 -= CHILD_BIRTH_INTERVAL;

    for house in houses.iter() {
        // Married/widowed couple present? (both must be 18–45 years old)
        let married_adults = chars
            .iter()
            .filter(|c| {
                c.house_id == house.id
                    && c.stage == LifeStage::Adult
                    && c.age >= CHILD_GROWTH_DURATION  // minimum 18 years
                    && c.age < MAX_REPRODUCTION_AGE    // maximum 45 years
                    && matches!(c.marital, MaritalStatus::Married | MaritalStatus::Widowed)
            })
            .count();
        if married_adults < 2 {
            continue;
        }

        // Room for more children?
        let children = chars
            .iter()
            .filter(|c| c.house_id == house.id && c.stage == LifeStage::Child)
            .count();
        if children >= MAX_CHILDREN {
            continue;
        }

        // Enough food buffer?
        if house.storage < 5 {
            continue;
        }

        // Spawn child near the house
        let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
        let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
        let ox = (rand::random::<f32>() - 0.5) * 64.0;
        let oy = (rand::random::<f32>() - 0.5) * 64.0;
        info!(
            "[BIRTH] House #{}: child born (now {} children, storage: {})",
            house.id, children + 1, house.storage,
        );

        let baby_gender = if rand::random::<f32>() < 0.5 { Gender::Male } else { Gender::Female };
        commands.spawn((
            Character {
                speed: 80.0,
                state: AiState::Idle,
                timer: 2.0,
                action_tile: None,
                percepts: Vec::new(),
                stuck_timer: 0.0,
                plot_id: house.id,
                house_id: house.id,
                stage: LifeStage::Child,
                age: 0.0,
                gender: baby_gender,
                personality: Personality::random(),
                marital: MaritalStatus::Single,
                sight_range: 4.0,
                food: 10,
                swim_streak: 0,
            },
            Growing { age: 0.0 },
            Transform::from_xyz(hx + ox, hy + oy, 2.0),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            spawn_child_sprite(parent, assets.char_child.clone());
        });
    }
}

fn aging_system(
    fixed_time: Res<Time<Fixed>>,
    mut commands: Commands,
    mut chars: Query<(Entity, &mut Character, Option<&mut Growing>, &Transform, &mut Visibility)>,
    children_q: Query<&Children>,
    mut death_events: ResMut<DeathEvents>,
    houses: Query<&House>,
    mut starvation_timer: ResMut<StarvationTimer>,
) {
    death_events.tiles.clear();
    let dt = fixed_time.delta_secs_f64();
    // --- Phase 1: Age-based deaths ---
    let mut to_die: Vec<Entity> = Vec::new();
    for (entity, mut ch, mut growing, tf, mut vis) in chars.iter_mut() {
        ch.age += dt;
        if let Some(ref mut g) = growing {
            g.age = ch.age;
        }
        if ch.stage == LifeStage::Adult && ch.age > LIFESPAN {
            let dx = tf.translation.x;
            let dy = tf.translation.y;
            let tile_x = (dx / TILE_SIZE).floor() as usize;
            let tile_y = (dy / TILE_SIZE).floor() as usize;
            info!(
                "[DEATH] Adult died at house #{} (age: {:.0}) tile ({}, {}) — Old Age",
                ch.house_id, ch.age, tile_x, tile_y,
            );
            *vis = Visibility::Hidden;
            death_events.deaths.push((ch.house_id, ch.age, dx, dy, ch.gender, "Old Age"));
            death_events.tiles.insert((tile_x, tile_y));
            to_die.push(entity);
        }
    }
    // Despawn age-dead entities (borrow, don't consume — starvation check still needs to_die)
    for &entity in &to_die {
        if let Ok(children) = children_q.get(entity) {
            for &child in children.iter() {
                if commands.get_entity(child).is_some() {
                    commands.entity(child).despawn();
                }
            }
        }
        if commands.get_entity(entity).is_some() {
            commands.entity(entity).despawn();
        }
    }
    // --- Phase 2: Starvation tracking ---
    for house in houses.iter() {
        if house.storage == 0 {
            *starvation_timer.0.entry(house.id).or_insert(0.0) += dt;
        } else {
            starvation_timer.0.remove(&house.id);
        }
    }
    // --- Phase 3: Starvation deaths ---
    let starve_kill: Vec<(usize, &'static str)> = starvation_timer.0.iter()
        .filter(|(_, &t)| t >= STARVATION_THRESHOLD)
        .map(|(&hid, _)| (hid, "Starvation"))
        .collect();
    if !starve_kill.is_empty() {
        // Reset timers so we don't re-kill every frame
        for &(hid, _) in &starve_kill {
            starvation_timer.0.remove(&hid);
        }
        let mut to_starve: Vec<Entity> = Vec::new();
        let mut starved_houses: Vec<usize> = Vec::new();
        for (entity, ch, _, tf, mut vis) in chars.iter_mut() {
            if ch.stage != LifeStage::Adult { continue; }
            if to_die.contains(&entity) { continue; } // already marked for age death
            if starved_houses.contains(&ch.house_id) { continue; }
            if !starve_kill.iter().any(|&(hid, _)| hid == ch.house_id) { continue; }
            let dx = tf.translation.x;
            let dy = tf.translation.y;
            let tile_x = (dx / TILE_SIZE).floor() as usize;
            let tile_y = (dy / TILE_SIZE).floor() as usize;
            let cause = starve_kill.iter().find(|&&(hid, _)| hid == ch.house_id).unwrap().1;
            info!(
                "[STARVE] Adult died at house #{} (age: {:.0}) tile ({}, {})",
                ch.house_id, ch.age, tile_x, tile_y,
            );
            *vis = Visibility::Hidden;
            death_events.deaths.push((ch.house_id, ch.age, dx, dy, ch.gender, cause));
            death_events.tiles.insert((tile_x, tile_y));
            to_starve.push(entity);
            starved_houses.push(ch.house_id);
        }
        for entity in to_starve {
            if let Ok(children) = children_q.get(entity) {
                for &child in children.iter() {
                    if commands.get_entity(child).is_some() {
                        commands.entity(child).despawn();
                    }
                }
            }
            if commands.get_entity(entity).is_some() {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// When an adult dies, widow their spouse and let the eldest child inherit.
fn inheritance_system(
    mut commands: Commands,
    mut chars: Query<(Entity, &mut Character, Option<&mut Growing>)>,
    death_events: Res<DeathEvents>,
) {
    // First pass: widowing surviving Married adults in the same house
    for &(house_id, _age, _dx, _dy, _gender, _cause) in &death_events.deaths {
        for (_entity, mut ch, _growing) in chars.iter_mut() {
            if ch.house_id == house_id
                && ch.stage == LifeStage::Adult
                && ch.marital == MaritalStatus::Married
            {
                ch.marital = MaritalStatus::Widowed;
                info!(
                    "[WIDOW] House #{} adult widowed — can still expand",
                    house_id,
                );
            }
        }
    }

    // Second pass: inheritance
    for &(house_id, _age, _dx, _dy, _gender, _cause) in &death_events.deaths {
        let mut eldest: Option<(Entity, f64)> = None; // (entity, age)
        for (entity, ch, growing) in chars.iter() {
            if ch.house_id == house_id && ch.stage == LifeStage::Child {
                if let Some(g) = growing {
                    if eldest.map_or(true, |(_, age)| g.age > age) {
                        eldest = Some((entity, g.age));
                    }
                }
            }
        }

        if let Some((child_entity, child_age)) = eldest {
            info!(
                "[INHERIT] House #{}: eldest child inherits (age: {:.0}) — must expand",
                house_id, child_age,
            );
            // Read gender/personality from the child before removing Growing
            let (child_gender, child_personality) = chars
                .iter()
                .find(|(e, _, _)| *e == child_entity)
                .map(|(_, ch, _)| (ch.gender, ch.personality))
                .unwrap_or((Gender::Male, Personality::random()));
            commands.entity(child_entity).remove::<Growing>();
            commands.entity(child_entity).insert(Character {
                speed: 100.0,
                state: AiState::Idle,
                timer: 1.0,
                action_tile: None,
                percepts: Vec::new(),
                stuck_timer: 0.0,
                plot_id: house_id,
                house_id,
                stage: LifeStage::Adult,
                age: child_age,
                gender: child_gender,
                personality: child_personality,
                marital: MaritalStatus::Widowed, // head of household — can expand
                sight_range: 8.0,
                food: 10,
                swim_streak: 0,
            });
        } else {
            info!("[INHERIT] House #{}: no heir — house vacant", house_id);
        }
    }
}

/// Tracks the next grid slot per house for tombstone placement.
#[derive(Resource, Default)]
pub struct NextGraveSlot(std::collections::HashMap<usize, usize>);

/// How close (world units) a death must be to the house for burial in the house grave plot.
const NEARBY_DIST: f32 = TILE_SIZE * 5.0;

/// Place a tombstone near the house if the character died nearby, or at the death
/// location if they died far from home.
fn grave_system(
    mut commands: Commands,
    mut death_events: ResMut<DeathEvents>,
    houses: Query<&House>,
    mut next_slot: ResMut<NextGraveSlot>,
    assets: Res<GameAssets>,
) {
    for (house_id, age, dx, dy, gender, cause) in death_events.deaths.drain(..) {
        let death_tile_x = (dx / TILE_SIZE).floor() as usize;
        let death_tile_y = (dy / TILE_SIZE).floor() as usize;

        // Check if the character died near their home
        let house_opt = houses.iter().find(|h| h.id == house_id);
        let near_home = house_opt
            .map(|h| {
                let h_cx = (h.tile_x + h.w / 2) as f32 * TILE_SIZE;
                let h_cy = (h.tile_y + h.h / 2) as f32 * TILE_SIZE;
                let d = ((dx - h_cx).powi(2) + (dy - h_cy).powi(2)).sqrt();
                d < NEARBY_DIST
            })
            .unwrap_or(false);

        let (_tile_x, _tile_y, cx, cy) = if near_home {
            // Grid placement below the house
            let h = house_opt.unwrap();
            const COLS: usize = 4;
            let slot = next_slot.0.entry(house_id).or_insert(0);
            let row = *slot / COLS;
            let col = *slot % COLS;
            *slot += 1;
            let gx = h.tile_x + col;
            let gy = h.tile_y.saturating_sub(1 + row);
            let (wc_x, wc_y) = tile_center(gx, gy);
            info!(
                "[SKULL] House #{} died at age {:.0} → grave ({}, {}) slot {}",
                house_id, age, gx, gy, slot,
            );
            (gx, gy, wc_x, wc_y)
        } else {
            // Remote death — place tombstone at the death location
            let (wc_x, wc_y) = tile_center(death_tile_x, death_tile_y);
            info!(
                "[SKULL] House #{} died at age {:.0} → remote grave ({}, {})",
                house_id, age, death_tile_x, death_tile_y,
            );
            (death_tile_x, death_tile_y, wc_x, wc_y)
        };

        commands.spawn((
            Grave,
            GraveInfo { age, house_id, gender, cause },
            Sprite {
                image: assets.misc_tombstone.clone(),
                custom_size: Some(Vec2::new(20.0, 20.0)),
                ..default()
            },
            Transform::from_xyz(cx, cy, 1.5),
            GlobalTransform::default(),
            Visibility::default(),
        ));
    }
}

fn child_growth_system(
    fixed_time: Res<Time<Fixed>>,
    mut commands: Commands,
    mut chars: Query<(Entity, &mut Character, &mut Growing, &Transform)>,
    adults: Query<&Character, Without<Growing>>,
    children_q: Query<&Children>,
    assets: Res<GameAssets>,
) {
    let dt = fixed_time.delta_secs_f64();

    // Count adults per house (using a Without<Growing> query for disjoint access)
    let mut adult_counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for ch in adults.iter() {
        if ch.stage == LifeStage::Adult {
            *adult_counts.entry(ch.house_id).or_insert(0) += 1;
        }
    }

    let mut to_grow: Vec<(Entity, f32, f32, usize, usize, Gender, Personality)> = Vec::new();

    for (entity, ch, mut growing, tf) in chars.iter_mut() {
        if ch.stage != LifeStage::Child {
            continue;
        }
        growing.age += dt;
        if growing.age >= CHILD_GROWTH_DURATION {
            to_grow.push((entity, tf.translation.x, tf.translation.y, ch.plot_id, ch.house_id, ch.gender, ch.personality));
        }
    }

    for (entity, _x, _y, _plot_id, house_id, gender, personality) in to_grow {
        let adult_count = adult_counts.get(&house_id).copied().unwrap_or(0);
        info!(
            "[GROW] House #{}: child grew up (now {} adults) — staying home as helper until married",
            house_id, adult_count + 1,
        );

        // Despawn old child sprites (cleanup to prevent ghost sprites)
        if let Ok(children) = children_q.get(entity) {
            for &child in children.iter() {
                commands.entity(child).despawn();
            }
        }

        // Convert in-place: remove Growing, update Character to adult
        commands.entity(entity).remove::<Growing>();
        commands.entity(entity).insert(Character {
            speed: 100.0,
            state: AiState::Idle,
            timer: 1.0,
            action_tile: None,
            percepts: Vec::new(),
            stuck_timer: 0.0,
            plot_id: house_id,
            house_id,
            stage: LifeStage::Adult,
            age: CHILD_GROWTH_DURATION,
            gender,
            personality,
            marital: MaritalStatus::Single,
            sight_range: 8.0,
            food: 10,
            swim_streak: 0,
        });

        // Spawn new adult sprite (gender-correct)
        let tex = match gender {
            Gender::Male => assets.char_male.clone(),
            Gender::Female => assets.char_female.clone(),
        };
        commands.entity(entity).with_children(|parent| {
            spawn_character_sprite(parent, tex);
        });
    }
}

// ---------------------------------------------------------------------------
// Expansion site search helpers
// ---------------------------------------------------------------------------

type SettlementSite = (Vec<(usize, usize)>, (usize, usize), (usize, usize));

fn find_expansion_site(
    map: &Map,
    tree_mask: &[bool],
    existing: &[(usize, usize)],
    near_x: f32,
    near_y: f32,
    target_size: usize,
    road_mask: &[bool],
) -> Option<SettlementSite> {
    let arable = [TileType::Grass, TileType::Meadow];
    let start_x = (near_x / TILE_SIZE) as isize;
    let start_y = (near_y / TILE_SIZE) as isize;

    // Build a "used" mask from existing plot tiles.
    // Also keep a separate "occupied" mask (with 2-tile buffer zone) for house
    // overlap checks — the `used` mask gets modified by flood-fill and includes
    // buffer tiles, while `occupied` keeps the pre-flood-fill state with spacing.
    let mut used = vec![false; MAP_WIDTH * MAP_HEIGHT];
    let mut occupied = vec![false; MAP_WIDTH * MAP_HEIGHT];
    for (ex, ey) in existing {
        used[ey * MAP_WIDTH + ex] = true;
        // 2-tile buffer zone around every existing tile (houses, farms, etc.)
        for bx in -2..=2isize {
            for by in -2..=2isize {
                let nx = *ex as isize + bx;
                let ny = *ey as isize + by;
                if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                    occupied[ny as usize * MAP_WIDTH + nx as usize] = true;
                }
            }
        }
    }

    // Road tiles are occupied — houses cannot be built on paths
    for (idx, has_road) in road_mask.iter().enumerate() {
        if *has_road {
            occupied[idx] = true;
        }
    }

    // Reserve graveyard area so expansions don't build on top of it
    for gx in GRAVEYARD_X..GRAVEYARD_X + GRAVEYARD_W {
        for gy in GRAVEYARD_Y..GRAVEYARD_Y + GRAVEYARD_H {
            if gx < MAP_WIDTH && gy < MAP_HEIGHT {
                occupied[gy * MAP_WIDTH + gx] = true;
            }
        }
    }

    // Spiral outward from the starting position
    for radius in 6..50 {
        for dy in -(radius as isize)..=radius as isize {
            for dx in -(radius as isize)..=radius as isize {
                if dx.abs() < radius as isize && dy.abs() < radius as isize {
                    continue;
                }
                let nx = start_x + dx;
                let ny = start_y + dy;
                if nx < 1 || nx >= MAP_WIDTH as isize - 1
                    || ny < 1 || ny >= MAP_HEIGHT as isize - 1
                {
                    continue;
                }
                let idx = ny as usize * MAP_WIDTH + nx as usize;
                if used[idx] || !arable.contains(&map.tiles[idx]) {
                    continue;
                }

                // Candidate – flood-fill a contiguous plot
                let mut plot = Vec::new();
                let mut queue = std::collections::VecDeque::new();
                queue.push_back((nx as usize, ny as usize));
                used[ny as usize * MAP_WIDTH + nx as usize] = true;

                while let Some((cx, cy)) = queue.pop_front() {
                    if plot.len() >= target_size {
                        break;
                    }
                    plot.push((cx, cy));
                    for (ddx, ddy) in &[(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                        let nnx = cx as isize + ddx;
                        let nny = cy as isize + ddy;
                        if nnx < 0 || nnx >= MAP_WIDTH as isize
                            || nny < 0 || nny >= MAP_HEIGHT as isize
                        {
                            continue;
                        }
                        let nidx = nny as usize * MAP_WIDTH + nnx as usize;
                        if used[nidx] {
                            continue;
                        }
                        used[nidx] = true;
                        if arable.contains(&map.tiles[nidx]) {
                            queue.push_back((nnx as usize, nny as usize));
                        }
                    }
                }

                if plot.len() < 6 {
                    continue; // too small
                }

                // House goes to the right of the plot
                let rightmost = plot.iter().map(|(x, _)| *x).max().unwrap();
                let col_ys: Vec<usize> = plot.iter()
                    .filter(|(x, _)| *x == rightmost)
                    .map(|(_, y)| *y)
                    .collect();
                let min_y = *col_ys.iter().min().unwrap_or(&0);
                let max_y = *col_ys.iter().max().unwrap_or(&0);
                let cy = (min_y + max_y) / 2;
                let house_pos = (rightmost + 2, cy.saturating_sub(1));
                let char_pos = (house_pos.0, house_pos.1 + 1);

                // Verify the 1×1 house tile and char tile are not already occupied
                if occupied[house_pos.1 * MAP_WIDTH + house_pos.0]
                    || occupied[char_pos.1 * MAP_WIDTH + char_pos.0]
                {
                    continue;
                }

                // Require at least 2 trees within 3 tiles of the house (wood source)
                let mut trees_nearby = 0;
                for dy in -3..=3isize {
                    for dx in -3..=3isize {
                        let nx = house_pos.0 as isize + dx;
                        let ny = house_pos.1 as isize + dy;
                        if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize
                            && tree_mask[ny as usize * MAP_WIDTH + nx as usize]
                        {
                            trees_nearby += 1;
                        }
                    }
                }
                if trees_nearby < 2 {
                    continue;
                }

                return Some((plot, house_pos, char_pos));
            }   // for dx
        }       // for dy
    }           // for radius
    None
}

// ---------------------------------------------------------------------------
// Resources for expansion & food
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct MealTimer(pub f64);

#[derive(Resource, Default)]
pub struct ChildMealTimer(pub f64);

/// Next available settlement ID (house_id + plot_id).
/// Starts at 1 because the initial map has 1 settlement (ID 0).
#[derive(Resource)]
pub struct NextSettlementId(pub usize);

impl Default for NextSettlementId {
    fn default() -> Self {
        Self(3)
    }
}

/// Death info: house_id, age, world_x, world_y, gender, cause (for inheritance + skull placement).
#[derive(Resource, Default)]
pub struct DeathEvents {
    pub deaths: Vec<(usize, f64, f32, f32, Gender, &'static str)>,
    /// Tiles where deaths occurred this frame — used synchronously by UI to hide dead chars.
    pub tiles: std::collections::HashSet<(usize, usize)>,
}

/// Tracks how long each house has been at 0 food.
#[derive(Resource, Default)]
pub struct StarvationTimer(pub std::collections::HashMap<usize, f64>);

/// Tile position of the village shop.
#[derive(Resource)]
pub struct ShopLocation {
    pub tile_x: usize,
    pub tile_y: usize,
}

impl Default for ShopLocation {
    fn default() -> Self {
        Self { tile_x: 55, tile_y: 70 }
    }
}

/// Per-tile foot-traffic wear counter (MAP_WIDTH × MAP_HEIGHT).
/// Also tracks recently failed destinations for pathfinding avoidance.
#[derive(Resource)]
pub struct RoadWear {
    pub wear: Vec<u32>,
    pub failed: Vec<bool>,
}

impl Default for RoadWear {
    fn default() -> Self {
        Self {
            wear: vec![0; MAP_WIDTH * MAP_HEIGHT],
            failed: vec![false; MAP_WIDTH * MAP_HEIGHT],
        }
    }
}

impl RoadWear {
    pub fn mark_failed(&mut self, tx: usize, ty: usize) {
        if tx < MAP_WIDTH && ty < MAP_HEIGHT {
            self.failed[ty * MAP_WIDTH + tx] = true;
        }
    }
    pub fn is_failed(&self, tx: usize, ty: usize) -> bool {
        if tx < MAP_WIDTH && ty < MAP_HEIGHT {
            self.failed[ty * MAP_WIDTH + tx]
        } else {
            false
        }
    }
    pub fn clear_failed(&mut self) {
        self.failed.fill(false);
    }
}

/// Tracks which tiles have a road overlay sprite.
#[derive(Resource)]
pub struct RoadRender {
    pub tiles: Vec<Option<Entity>>,
}

impl Default for RoadRender {
    fn default() -> Self {
        Self { tiles: vec![None; MAP_WIDTH * MAP_HEIGHT] }
    }
}

/// Global path memory — counts purposeful traversals per tile.
/// Used to bias character movement toward known routes.
#[derive(Resource)]
pub struct PathMemory {
    pub counts: Vec<u32>,
}

impl Default for PathMemory {
    fn default() -> Self {
        Self { counts: vec![0; MAP_WIDTH * MAP_HEIGHT] }
    }
}

/// Sim-seconds counter for periodic essentials consumption.
#[derive(Resource, Default)]
pub struct EssentialsTimer(pub f64);

/// Households periodically consume essentials — when they run low an adult goes
/// to the shop to restock.
fn essentials_depletion(
    fixed_time: Res<Time<Fixed>>,
    mut timer: ResMut<EssentialsTimer>,
    mut houses: Query<&mut House>,
) {
    timer.0 += fixed_time.delta_secs_f64();
    if timer.0 < ESSENTIALS_DEPLETION_INTERVAL {
        return;
    }
    timer.0 -= ESSENTIALS_DEPLETION_INTERVAL;

    for mut house in houses.iter_mut() {
        if house.essentials > 0 {
            house.essentials -= 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Road connectivity — compute which neighbors are roads and pick the
// correct sprite variant for smooth path rendering.
// ---------------------------------------------------------------------------

/// Bitmask: N=8, E=4, S=2, W=1
fn road_connectivity_mask(idx: usize, road_wear: &[u32]) -> u8 {
    let x = idx % MAP_WIDTH;
    let y = idx / MAP_WIDTH;
    let mut mask = 0u8;
    if y > 0 && road_wear[idx - MAP_WIDTH] >= ROAD_THRESHOLD_1 {
        mask |= 8;
    }
    if x < MAP_WIDTH - 1 && road_wear[idx + 1] >= ROAD_THRESHOLD_1 {
        mask |= 4;
    }
    if y < MAP_HEIGHT - 1 && road_wear[idx + MAP_WIDTH] >= ROAD_THRESHOLD_1 {
        mask |= 2;
    }
    if x > 0 && road_wear[idx - 1] >= ROAD_THRESHOLD_1 {
        mask |= 1;
    }
    mask
}

fn road_texture_index(mask: u8) -> usize {
    match mask {
        // Full crossing
        15 => 10, // cross  N+E+S+W
        // T-junctions
        14 => 6,  // TN    N+E+S
        13 => 7,  // TE    E+N+W
        11 => 9,  // TW    W+N+S
        7  => 8,  // TS    S+E+W
        // Turns / L-junctions
        12 => 2,  // NE
        9  => 3,  // NW
        6  => 4,  // SE
        3  => 5,  // SW
        // Straights
        10 | 8 | 2 => 1, // V  N+S  (or single N/S)
        5  | 4 | 1 => 0, // H  W+E  (or single E/W)
        _ => 0,
    }
}

/// Periodically check road wear and spawn/update semi-transparent path overlays
/// on frequently walked tiles. Uses neighbor-aware connectivity to select the
/// correct road variant sprite.
fn road_render_system(
    mut commands: Commands,
    mut sprites: Query<&mut Sprite, With<RoadTile>>,
    road_wear: Res<RoadWear>,
    mut road_render: ResMut<RoadRender>,
    assets: Res<GameAssets>,
    mut frame: Local<u32>,
) {
    *frame = frame.wrapping_add(1);
    // Throttle: check once every 30 frames (~every 0.5s at 60fps)
    if *frame % 30 != 0 {
        return;
    }

    for (idx, entry) in road_render.tiles.iter_mut().enumerate() {
        let wear = road_wear.wear[idx];
        if wear < ROAD_THRESHOLD_1 {
            continue;
        }

        // Determine road shape from neighbor connectivity
        let conn = road_connectivity_mask(idx, &road_wear.wear);
        let tex_idx = road_texture_index(conn);
        let texture = assets.misc_roads[tex_idx].clone();

        // Alpha scales with wear: barely visible → well-worn path
        let alpha = ((wear as f32 / ROAD_THRESHOLD_3 as f32) * 0.45).min(0.45).max(0.06);

        if let Some(entity) = entry {
            // Update existing sprite's texture + colour
            if let Ok(mut sprite) = sprites.get_mut(*entity) {
                if sprite.image != texture {
                    sprite.image = texture;
                }
                let new_color = Color::srgba(1.0, 1.0, 1.0, alpha);
                if sprite.color != new_color {
                    sprite.color = new_color;
                }
            }
        } else {
            // Spawn a new road overlay sprite
            let tx = idx % MAP_WIDTH;
            let ty = idx / MAP_WIDTH;
            let wx = tx as f32 * TILE_SIZE + TILE_SIZE / 2.0;
            let wy = ty as f32 * TILE_SIZE + TILE_SIZE / 2.0;

            let entity = commands.spawn((
                RoadTile,
                Sprite {
                    image: texture,
                    color: Color::srgba(1.0, 1.0, 1.0, alpha),
                    custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(wx, wy, 0.5),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .id();
            *entry = Some(entity);
        }
    }
}

/// Diagnostic timer for periodic food economy logging.
#[derive(Resource, Default)]
pub struct FoodDiagTimer(pub f64);

/// Manual shop interaction — press C while hovering over the shop to trade
/// food for daily essentials for the most-needy house.
fn shop_interaction(
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<crate::ui::HoveredTile>,
    shop_location: Res<ShopLocation>,
    houses: Query<&House>,
    mut events: EventWriter<crate::actions::ActionEvent>,
) {
    if !keys.just_pressed(KeyCode::KeyC) {
        return;
    }
    let Some((hx, hy)) = hovered.0 else { return };

    // Check if hovering over the 2x2 shop area
    if hx < shop_location.tile_x || hx >= shop_location.tile_x + 2
        || hy < shop_location.tile_y || hy >= shop_location.tile_y + 2
    {
        return;
    }

    // Find the house most in need (lowest essentials) that can afford a trade
    let target = houses.iter()
        .filter(|h| h.storage >= SHOP_COST_FOOD)
        .min_by_key(|h| h.essentials);

    if let Some(house) = target {
        events.send(crate::actions::ActionEvent::ShopTrade {
            house_id: house.id,
        });
        info!(
            "[SHOP] Manual trade for House #{} (food: {}, essentials: {})",
            house.id, house.storage, house.essentials,
        );
    } else {
        info!("[SHOP] No house can afford a trade");
    }
}

/// Log food economy summary every ~30 game days.
fn food_diagnostics(
    fixed_time: Res<Time<Fixed>>,
    sim: Res<crate::sim_time::SimTime>,
    mut diag_timer: ResMut<FoodDiagTimer>,
    chars: Query<&Character>,
    houses: Query<&House>,
    farm_tiles: Query<&FarmTile>,
    mut road_wear: ResMut<RoadWear>,
) {
    let dt = fixed_time.delta_secs_f64();
    diag_timer.0 += dt;
    if diag_timer.0 < 30.0 { return; }
    diag_timer.0 = 0.0;

    let (years, months, days) = sim.date();
    info!("=== FOOD DIAG Year {}.{:02}.{:02} ===", years, months, days);

    for house in houses.iter() {
        let adult_count = chars.iter().filter(|c| c.house_id == house.id && c.stage == LifeStage::Adult).count();
        let child_count = chars.iter().filter(|c| c.house_id == house.id && c.stage == LifeStage::Child).count();
        let plot_tiles = farm_tiles.iter().filter(|ft| ft.plot == house.id).count();
        let ready = farm_tiles.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Ready).count();
        let weedy = farm_tiles.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Weedy).count();
        let fallow = farm_tiles.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Fallow).count();
        let growing = farm_tiles.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Growing).count();
        info!(
            "  House #{}: food={} ess={} adults={} children={} tiles={}(R{} W{} F{} G{})",
            house.id, house.storage, house.essentials, adult_count, child_count,
            plot_tiles, ready, weedy, fallow, growing,
        );
        // Character state dump
        for ch in chars.iter().filter(|c| c.house_id == house.id) {
            let farm_percepts = ch.percepts.iter()
                .filter(|p| matches!(p, Percept::FarmTile { plot, .. } if *plot == ch.plot_id))
                .count();
            let ready_p = ch.percepts.iter()
                .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Ready))
                .count();
            let fallow_p = ch.percepts.iter()
                .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Fallow))
                .count();
            let weedy_p = ch.percepts.iter()
                .filter(|p| matches!(p, Percept::FarmTile { state, .. } if *state == CropState::Weedy))
                .count();
            info!(
                "    Char plot#{} state={:?} timer={:.3} action_tile={:?} food={} percepts(plt={} R={} F={} W={})",
                ch.plot_id, ch.state, ch.timer, ch.action_tile, ch.food,
                farm_percepts, ready_p, fallow_p, weedy_p,
            );
        }
    }
    // Clear failed destination blacklist so tiles can be retried
    road_wear.clear_failed();
}

/// Maximum distance (pixels) a character can be from their house to access
/// house food storage. Beyond this, they must rely on personal food only.
const HOME_FOOD_RANGE: f32 = 250.0; // ~6 tiles

fn daily_consumption(
    fixed_time: Res<Time<Fixed>>,
    mut timer: ResMut<MealTimer>,
    mut child_timer: ResMut<ChildMealTimer>,
    mut chars: Query<(&mut Character, &Transform)>,
    mut houses: Query<&mut House>,
) {
    let dt = fixed_time.delta_secs_f64();

    // Snapshot house positions for food-access distance checks
    let house_locs: std::collections::HashMap<usize, (f32, f32)> = houses
        .iter()
        .map(|h| {
            (
                h.id,
                (
                    (h.tile_x as f32 + h.w as f32 / 2.0) * TILE_SIZE,
                    (h.tile_y as f32 + h.h as f32 / 2.0) * TILE_SIZE,
                ),
            )
        })
        .collect();

    // Adults: eat daily
    timer.0 += dt;
    if timer.0 >= MEAL_INTERVAL {
        timer.0 -= MEAL_INTERVAL;
        for (mut ch, tf) in chars.iter_mut() {
            if ch.stage == LifeStage::Child {
                continue;
            }
            if ch.food > 0 {
                ch.food -= 1;
            } else {
                // Only access house storage if near home
                let near_home = house_locs.get(&ch.house_id).map_or(true, |&(hx, hy)| {
                    let dx = tf.translation.x - hx;
                    let dy = tf.translation.y - hy;
                    (dx * dx + dy * dy).sqrt() < HOME_FOOD_RANGE
                });
                if near_home {
                    if let Some(mut house) = houses.iter_mut().find(|h| h.id == ch.house_id) {
                        if house.storage > 0 {
                            house.storage -= 1;
                            if house.storage == 0 {
                                warn!("[FOOD] House #{} food DEPLETED after adult meal", house.id);
                            }
                        }
                    }
                }
            }
        }
    }

    // Children: eat every 2 days
    child_timer.0 += dt;
    if child_timer.0 >= MEAL_INTERVAL * 2.0 {
        child_timer.0 -= MEAL_INTERVAL * 2.0;
        for (mut ch, tf) in chars.iter_mut() {
            if ch.stage != LifeStage::Child {
                continue;
            }
            if ch.food > 0 {
                ch.food -= 1;
            } else {
                // Only access house storage if near home
                let near_home = house_locs.get(&ch.house_id).map_or(true, |&(hx, hy)| {
                    let dx = tf.translation.x - hx;
                    let dy = tf.translation.y - hy;
                    (dx * dx + dy * dy).sqrt() < HOME_FOOD_RANGE
                });
                if near_home {
                    if let Some(mut house) = houses.iter_mut().find(|h| h.id == ch.house_id) {
                        if house.storage > 0 {
                            house.storage -= 1;
                            if house.storage == 0 {
                                warn!("[FOOD] House #{} food DEPLETED after child meal", house.id);
                            }
                        }
                    }
                }
            }
        }
    }
}
