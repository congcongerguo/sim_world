use bevy::prelude::*;

use crate::actions::ActionEvent;
use crate::farmland::{color_for_clearing, color_for_state, setup_farm_layout, CropState, FarmLayout, FarmTile, PendingFarmland};
use crate::map::{Map, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_time::TimeScale;
use crate::vegetation::{Vegetation, VegetationKind};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Sim-seconds per "day" for food consumption.
const DAY_SECONDS: f64 = 12.0;
/// Minimum sim-seconds between birth attempts.
const CHILD_BIRTH_INTERVAL: f64 = 40.0;
/// Sim-seconds for a child to grow into an adult.
const CHILD_GROWTH_DURATION: f64 = 60.0;
/// Max children per household.
const MAX_CHILDREN: usize = 3;
/// Sim-seconds after which an adult dies of old age.
const LIFESPAN: f64 = 350.0;

/// Graveyard top-left tile on the map.
const GRAVEYARD_X: usize = 3;
const GRAVEYARD_Y: usize = 110;
/// Graveyard extends GRAVEYARD_W columns and GRAVEYARD_H rows.
const GRAVEYARD_W: usize = 10;
const GRAVEYARD_H: usize = 15;

// Essentials & shop
/// Sim-seconds between essentials consumption ticks.
const ESSENTIALS_DEPLETION_INTERVAL: f64 = 50.0;
/// Food cost per shop visit.
const SHOP_COST_FOOD: u32 = 3;
/// Essentials gained per shop visit.
/// Starting essentials for each household.
const HOUSE_START_ESSENTIALS: u32 = 20;
/// Below this threshold, an adult will go shopping.
const ESSENTIALS_LOW_THRESHOLD: u32 = 5;

// Road wear
const ROAD_THRESHOLD_1: u32 = 15;
const ROAD_THRESHOLD_3: u32 = 150;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum AiState {
    Idle,
    MoveTo(f32, f32),
    Exploring { origin_x: f32, origin_y: f32, dir_x: f32, dir_y: f32 },
    GoingToShop,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LifeStage {
    Child,
    Adult,
}

/// An autonomous character in the simulation.
#[derive(Component)]
pub struct Character {
    pub speed: f32,
    state: AiState,
    timer: f64,
    /// Flag raised when the character arrives at a farm tile.
    action_tile: Option<(usize, usize)>,
    /// Which plot (0..2) this character manages.
    pub plot_id: usize,
    /// Which house (by House::id) this character lives in.
    pub house_id: usize,
    pub stage: LifeStage,
    /// Accumulated sim-seconds since birth.
    pub age: f64,
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
        app.add_systems(PostStartup, (
            spawn_characters.after(setup_farm_layout),
            spawn_houses.after(setup_farm_layout),
            spawn_shop.after(setup_farm_layout),
        ));
        app.add_systems(Update, (
            character_ai,
            process_actions,
            reproduction_system,
            child_growth_system,
            aging_system,
            inheritance_system,
            grave_system,
            essentials_depletion,
            road_render_system,
            daily_consumption,
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

/// Speed multiplier based on terrain type the character is standing on.
fn tile_speed_multiplier(map: &Map, tf: &Transform) -> f32 {
    let x = (tf.translation.x / TILE_SIZE).floor() as usize;
    let y = (tf.translation.y / TILE_SIZE).floor() as usize;
    if x >= MAP_WIDTH || y >= MAP_HEIGHT {
        return 1.0;
    }
    match map.tiles[y * MAP_WIDTH + x] {
        TileType::Grass | TileType::Meadow | TileType::Dirt => 1.0,
        TileType::Sand | TileType::Clay => 0.85,
        TileType::Desert => 0.75,
        TileType::Tundra => 0.6,
        TileType::Forest | TileType::Snow => 0.5,
        TileType::Stone => 0.4,
        TileType::Swamp => 0.3,
        TileType::Ice => 0.65,
        _ => 1.0, // Water / DeepWater / Lava — shouldn't be walked on
    }
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

fn spawn_character_sprite(parent: &mut ChildBuilder, body_color: Color) {
    parent.spawn((
        Sprite {
            color: body_color,
            custom_size: Some(Vec2::new(14.0, 20.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -4.0, 0.0),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    parent.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.85, 0.65),
            custom_size: Some(Vec2::new(12.0, 12.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 12.0, 0.0),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn spawn_child_sprite(parent: &mut ChildBuilder, body_color: Color) {
    parent.spawn((
        Sprite {
            color: body_color,
            custom_size: Some(Vec2::new(10.0, 14.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -2.0, 0.0),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    parent.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.85, 0.65),
            custom_size: Some(Vec2::new(8.0, 8.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 10.0, 0.0),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn spawn_house_building(parent: &mut ChildBuilder) {
    // Ground shadow (at bottom: negative Y)
    parent.spawn((
        Sprite {
            color: Color::srgb(0.30, 0.18, 0.08),
            custom_size: Some(Vec2::new(48.0, 8.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -18.0, 0.01),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    // Left slope (人字形 left side — peak at top-right, base at bottom-left)
    parent.spawn((
        Sprite {
            color: Color::srgb(0.55, 0.35, 0.15),
            custom_size: Some(Vec2::new(42.0, 8.0)),
            ..default()
        },
        Transform::from_xyz(-12.0, 0.0, 0.02)
            .with_rotation(Quat::from_rotation_z(0.98)),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    // Right slope (人字形 right side — peak at top-left, base at bottom-right)
    parent.spawn((
        Sprite {
            color: Color::srgb(0.55, 0.35, 0.15),
            custom_size: Some(Vec2::new(42.0, 8.0)),
            ..default()
        },
        Transform::from_xyz(12.0, 0.0, 0.02)
            .with_rotation(Quat::from_rotation_z(-0.98)),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    // Ridge cap (at peak: positive Y)
    parent.spawn((
        Sprite {
            color: Color::srgb(0.40, 0.22, 0.06),
            custom_size: Some(Vec2::new(5.0, 3.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 17.0, 0.03),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    // Dark opening (at bottom: negative Y)
    parent.spawn((
        Sprite {
            color: Color::srgb(0.08, 0.05, 0.02),
            custom_size: Some(Vec2::new(10.0, 14.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -17.0, 0.03),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

const CHAR_COLORS: &[(f32, f32, f32)] = &[
    (0.15, 0.55, 0.95), // blue
    (0.55, 0.20, 0.75), // purple
    (0.20, 0.70, 0.40), // green
];

fn spawn_characters(mut commands: Commands, layout: Res<FarmLayout>) {
    for i in 0..3 {
        let (tx, ty) = layout.chars[i];
        let (r, g, b) = CHAR_COLORS[i];
        let color = Color::srgb(r, g, b);

        // Spawn 2 adults per house (husband & wife)
        for offset_x in [0i8, 1] {
            let (x, y) = tile_center(
                (tx as i8 + offset_x) as usize,
                ty,
            );
            commands.spawn((
                Character {
                    speed: 100.0,
                    state: AiState::Idle,
                    timer: offset_x as f64, // stagger their initial timers
                    action_tile: None,
                    plot_id: i,
                    house_id: i,
                    stage: LifeStage::Adult,
                    age: rand::random::<f64>() * 100.0 + 50.0,
                },
                Transform::from_xyz(x, y, 2.0),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .with_children(|parent| {
                spawn_character_sprite(parent, color);
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

fn spawn_houses(mut commands: Commands, layout: Res<FarmLayout>) {
    for i in 0..3 {
        let (tile_x, tile_y) = layout.houses[i];
        let w = 2usize;
        let h = 2usize;

        let world_x = (tile_x as f32 + w as f32 / 2.0) * TILE_SIZE;
        let world_y = (tile_y as f32 + h as f32 / 2.0) * TILE_SIZE;

        commands.spawn((
            House {
                id: i,
                tile_x,
                tile_y,
                w,
                h,
                storage: 10,
                essentials: HOUSE_START_ESSENTIALS,
            },
            Sprite {
                color: Color::srgb(0.45, 0.28, 0.12),
                custom_size: Some(Vec2::new(40.0, 36.0)),
                ..default()
            },
            Transform::from_xyz(world_x, world_y, 1.3),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            spawn_house_building(parent);
        });
    }
}

/// Spawn the village shop near the first settlement.
fn spawn_shop(mut commands: Commands, layout: Res<FarmLayout>) {
    // Place shop a few tiles from house #1 (middle of the initial 3)
    let (hx, hy) = layout.houses[1];
    let shop_tile_x = hx + 4;
    let shop_tile_y = hy + 5;

    commands.insert_resource(ShopLocation {
        tile_x: shop_tile_x,
        tile_y: shop_tile_y,
    });

    let wx = (shop_tile_x as f32 + 1.0) * TILE_SIZE;
    let wy = (shop_tile_y as f32 + 1.0) * TILE_SIZE;

    commands.spawn((
        Shop,
        Sprite {
            color: Color::srgb(0.95, 0.88, 0.75),
            custom_size: Some(Vec2::new(48.0, 48.0)),
            ..default()
        },
        Transform::from_xyz(wx, wy, 1.3),
        GlobalTransform::default(),
        Visibility::default(),
    ))
    .with_children(|parent| {
        // Red roof
        parent.spawn((
            Sprite {
                color: Color::srgb(0.85, 0.12, 0.08),
                custom_size: Some(Vec2::new(52.0, 12.0)),
                ..default()
            },
            Transform::from_xyz(0.0, -20.0, 0.01),
            GlobalTransform::default(),
            Visibility::default(),
        ));
        // Yellow sign
        parent.spawn((
            Sprite {
                color: Color::srgb(0.92, 0.90, 0.30),
                custom_size: Some(Vec2::new(20.0, 8.0)),
                ..default()
            },
            Transform::from_xyz(0.0, 20.0, 0.01),
            GlobalTransform::default(),
            Visibility::default(),
        ));
    });
}

// ---------------------------------------------------------------------------
// AI: movement & decision
// ---------------------------------------------------------------------------

fn character_ai(
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut commands: Commands,
    mut chars: Query<(&mut Character, &mut Transform)>,
    farm_tiles: Query<(&FarmTile, &Transform), Without<Character>>,
    houses: Query<&House>,
    vegetation: Query<(&Vegetation, &Transform), Without<Character>>,
    map: Res<Map>,
    mut next_id: ResMut<NextSettlementId>,
    shop_location: Res<ShopLocation>,
    mut road_wear: ResMut<RoadWear>,
    mut pending_farmland: ResMut<PendingFarmland>,
) {
    if scale.speed == 0.0 {
        return;
    }

    // Build tree position mask once per frame
    let mut tree_mask = vec![false; MAP_WIDTH * MAP_HEIGHT];
    for (veg, vtf) in vegetation.iter() {
        if matches!(veg.kind, VegetationKind::DeciduousTree | VegetationKind::PineTree | VegetationKind::PalmTree) {
            let tx = (vtf.translation.x / TILE_SIZE) as usize;
            let ty = (vtf.translation.y / TILE_SIZE) as usize;
            if tx < MAP_WIDTH && ty < MAP_HEIGHT {
                tree_mask[ty * MAP_WIDTH + tx] = true;
            }
        }
    }

    // Existing occupied tiles for overlap checks during building
    let existing: Vec<(usize, usize)> = {
        let mut list: Vec<(usize, usize)> = farm_tiles.iter().map(|(ft, _)| (ft.tile_x, ft.tile_y)).collect();
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
        let dt = time.delta_secs_f64() * scale.speed;
        ch.timer -= dt;
        let pos = (tf.translation.x, tf.translation.y);

        if ch.action_tile.is_some() {
            continue;
        }

        match ch.state {
            AiState::Idle => {
                if ch.timer <= 0.0 {
                    if ch.stage == LifeStage::Child {
                        // Child: wander near home
                        if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                            let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                            let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                            let wx = hx + (rand::random::<f32>() - 0.5) * 80.0;
                            let wy = hy + (rand::random::<f32>() - 0.5) * 80.0;
                            ch.state = AiState::MoveTo(wx, wy);
                        }
                        ch.timer = 4.0;
                    } else {
                        // Adult: look for work in assigned plot
                        let farm_list: Vec<(&FarmTile, &Transform)> = farm_tiles
                            .iter()
                            .filter(|(ft, _)| ft.plot == ch.plot_id)
                            .collect();

                        let target = find_farm_tile(farm_list.iter(), pos, CropState::Ready)
                            .or_else(|| find_farm_tile(farm_list.iter(), pos, CropState::Weedy))
                            .or_else(|| find_farm_tile(farm_list.iter(), pos, CropState::Fallow));

                        if let Some((tx, ty)) = target {
                            let (wx, wy) = tile_center(tx, ty);
                            ch.state = AiState::MoveTo(wx, wy);
                        } else {
                            // Check for pending tiles to clear
                            let pending_target = pending_farmland.plots.get(&ch.plot_id)
                                .and_then(|tiles| tiles.first().copied());

                            if let Some((px, py)) = pending_target {
                                let (wx, wy) = tile_center(px, py);
                                ch.state = AiState::MoveTo(wx, wy);
                            } else {
                                // No farm work — check essentials first
                                let needs_essentials = houses.iter()
                                    .find(|h| h.id == ch.house_id)
                                    .map(|h| h.essentials <= ESSENTIALS_LOW_THRESHOLD && h.storage >= SHOP_COST_FOOD)
                                    .unwrap_or(false);

                                if needs_essentials {
                                    ch.state = AiState::GoingToShop;
                                    ch.timer = 2.0;
                                } else {
                                    // Check if we can explore (enough stored food)
                                    let can_explore = houses.iter()
                                        .find(|h| h.id == ch.house_id)
                                        .map(|h| h.storage >= 25)
                                        .unwrap_or(false);

                                    if can_explore {
                                        let angle = rand::random::<f32>() * std::f32::consts::TAU;
                                        ch.state = AiState::Exploring {
                                            origin_x: pos.0, origin_y: pos.1,
                                            dir_x: angle.cos(), dir_y: angle.sin(),
                                        };
                                        ch.timer = 2.0;
                                    } else {
                                        // Not enough food → go home
                                        if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                                            let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                                            let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                                            ch.state = AiState::MoveTo(hx, hy);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            AiState::MoveTo(wx, wy) => {
                let dx = wx - pos.0;
                let dy = wy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < 2.0 {
                    let (tx, ty) = current_tile(&tf);
                    let is_my_farm = farm_tiles.iter().any(|(ft, _)| {
                        ft.tile_x == tx && ft.tile_y == ty && ft.plot == ch.plot_id
                    });
                    let is_pending = pending_farmland.plots.get(&ch.plot_id)
                        .map(|tiles| tiles.contains(&(tx, ty)))
                        .unwrap_or(false);
                    if is_my_farm || is_pending {
                        ch.action_tile = Some((tx, ty));
                    } else {
                        ch.state = AiState::Idle;
                        ch.timer = 3.0;
                    }
                } else {
                    let tile_speed = tile_speed_multiplier(&map, &tf);
                    let speed = ch.speed * scale.speed as f32 * tile_speed;
                    tf.translation.x += dx / dist * speed * time.delta_secs();
                    tf.translation.y += dy / dist * speed * time.delta_secs();

                    // Road wear — track frequently-walked tiles
                    let (rtx, rty) = current_tile(&tf);
                    if rtx < MAP_WIDTH && rty < MAP_HEIGHT {
                        road_wear.wear[rty * MAP_WIDTH + rtx] += 1;
                    }
                }
            }

            AiState::GoingToShop => {
                let shop_tile_x = shop_location.tile_x;
                let shop_tile_y = shop_location.tile_y;
                let shop_wx = shop_tile_x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let shop_wy = shop_tile_y as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let dx = shop_wx - pos.0;
                let dy = shop_wy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < 2.0 {
                    // Arrived at shop — signal process_actions to handle trade
                    ch.action_tile = Some((shop_tile_x, shop_tile_y));
                } else {
                    let tile_speed = tile_speed_multiplier(&map, &tf);
                    let speed = ch.speed * scale.speed as f32 * tile_speed;
                    tf.translation.x += dx / dist * speed * time.delta_secs();
                    tf.translation.y += dy / dist * speed * time.delta_secs();

                    // Road wear
                    let (rtx, rty) = current_tile(&tf);
                    if rtx < MAP_WIDTH && rty < MAP_HEIGHT {
                        road_wear.wear[rty * MAP_WIDTH + rtx] += 1;
                    }
                }
            }

            AiState::Exploring { origin_x, origin_y, dir_x, dir_y } => {
                let tile_speed = tile_speed_multiplier(&map, &tf);
                let speed = ch.speed * scale.speed as f32 * tile_speed;
                tf.translation.x += dir_x * speed * time.delta_secs();
                tf.translation.y += dir_y * speed * time.delta_secs();

                // Road wear
                let (rtx, rty) = current_tile(&tf);
                if rtx < MAP_WIDTH && rty < MAP_HEIGHT {
                    road_wear.wear[rty * MAP_WIDTH + rtx] += 1;
                }

                let (cx, cy) = (tf.translation.x, tf.translation.y);
                let dist = ((cx - origin_x).powi(2) + (cy - origin_y).powi(2)).sqrt();

                // Walked too far — go home
                if dist > 400.0 {
                    if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                        let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                        let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                        ch.state = AiState::MoveTo(hx, hy);
                    } else {
                        ch.state = AiState::Idle;
                        ch.timer = 5.0;
                    }
                    continue;
                }

                // Periodically scan for trees and check food supply
                ch.timer -= dt;
                if ch.timer <= 0.0 {
                    ch.timer = 1.5;

                    // Check if home still has enough food for the expedition
                    let home_food = houses.iter()
                        .find(|h| h.id == ch.house_id)
                        .map(|h| h.storage)
                        .unwrap_or(0);
                    if home_food < 10 {
                        // Not enough food — abort and return home
                        if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                            let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                            let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                            ch.state = AiState::MoveTo(hx, hy);
                        } else {
                            ch.state = AiState::Idle;
                            ch.timer = 5.0;
                        }
                        continue;
                    }

                    let (tx, ty) = current_tile(&tf);
                    let mut tree_count = 0;
                    for dy in -3..=3isize {
                        for dx in -3..=3isize {
                            let nx = tx as isize + dx;
                            let ny = ty as isize + dy;
                            if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize
                                && tree_mask[ny as usize * MAP_WIDTH + nx as usize]
                            {
                                tree_count += 1;
                            }
                        }
                    }

                    if tree_count >= 3 {
                        // Found trees — try to build a new settlement
                        let (mx, my) = tile_center(tx, ty);
                        if let Some((plot, house_tile, char_tile)) =
                            find_expansion_site(&map, &tree_mask, &existing, mx, my, 10)
                        {
                            let sid = next_id.0;
                            next_id.0 += 1;
                            let (r, g, b) = CHAR_COLORS[ch.house_id % CHAR_COLORS.len()];
                            let color = Color::srgb(r, g, b);

                            info!(
                                "[EXPAND] Settlement #{} — explored by #{} at ({}, {}), {} tiles",
                                sid, ch.house_id, house_tile.0, house_tile.1, plot.len(),
                            );

                            // Spawn 2 starter tiles, rest as pending
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
                                            color: color_for_state(CropState::Fallow),
                                            custom_size: Some(Vec2::new(TILE_SIZE - 2.0, TILE_SIZE - 2.0)),
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

                            // Spawn house
                            let (hx, hy) = tile_center(house_tile.0, house_tile.1);
                            commands.spawn((
                                House {
                                    id: sid,
                                    tile_x: house_tile.0,
                                    tile_y: house_tile.1,
                                    w: 2,
                                    h: 2,
                                    storage: 5,
                                    essentials: HOUSE_START_ESSENTIALS / 2,
                                },
                                Sprite {
                                    color: Color::srgb(0.82, 0.71, 0.55),
                                    custom_size: Some(Vec2::new(40.0, 36.0)),
                                    ..default()
                                },
                                Transform::from_xyz(hx, hy, 1.3),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| spawn_house_building(parent));

                            // Spawn settler
                            let (cx, cy) = tile_center(char_tile.0, char_tile.1);
                            commands.spawn((
                                Character {
                                    speed: 100.0,
                                    state: AiState::Idle,
                                    timer: 1.0,
                                    action_tile: None,
                                    plot_id: sid,
                                    house_id: sid,
                                    stage: LifeStage::Adult,
                                    age: rand::random::<f64>() * 50.0 + 20.0,
                                },
                                Transform::from_xyz(cx, cy, 2.0),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| spawn_character_sprite(parent, color));

                            // Explorer returns home
                            if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                                let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                                let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                                ch.state = AiState::MoveTo(hx, hy);
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AI: perform farm actions
// ---------------------------------------------------------------------------

fn process_actions(
    mut commands: Commands,
    mut chars: Query<(&mut Character, &Transform)>,
    farm_tiles: Query<&FarmTile>,
    houses: Query<&House>,
    mut events: EventWriter<ActionEvent>,
    shop_location: Res<ShopLocation>,
    mut pending_farmland: ResMut<PendingFarmland>,
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
                ch.state = AiState::MoveTo(hx, hy);
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
                    color: color_for_clearing(0.0),
                    custom_size: Some(Vec2::new(TILE_SIZE - 2.0, TILE_SIZE - 2.0)),
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

        let plot = farm_tiles
            .iter()
            .find(|ft| ft.tile_x == tx && ft.tile_y == ty)
            .map(|ft| ft.plot);

        let Some(plot_id) = plot else {
            ch.state = AiState::Idle;
            ch.timer = 2.0;
            continue;
        };

        // Snapshot state before sending the event (event will mutate it)
        let was_ready = farm_tiles
            .iter()
            .any(|ft| ft.plot == plot_id && ft.state == CropState::Ready);

        // Action processor handles state transition + storage deposit
        events.send(ActionEvent::FarmInteract {
            plot_id,
            house_id: Some(ch.house_id),
        });

        // AI navigation after action
        if was_ready {
            let tile_count = farm_tiles
                .iter()
                .filter(|ft| ft.plot == plot_id)
                .count();
            info!(
                "[HARV] Plot #{} harvested by house #{} — {} tiles collected",
                plot_id, ch.house_id, tile_count,
            );

            let home_pos = houses
                .iter()
                .find(|h| h.id == ch.house_id)
                .map(|house| {
                    let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                    let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                    (hx, hy)
                });
            ch.state = if let Some((hx, hy)) = home_pos {
                AiState::MoveTo(hx, hy)
            } else {
                AiState::Idle
            };
            ch.timer = 2.0;
        } else {
            ch.state = AiState::Idle;
            ch.timer = 2.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Population
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct ReproductionTimer(pub f64);

fn reproduction_system(
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut timer: ResMut<ReproductionTimer>,
    chars: Query<&Character>,
    houses: Query<&House>,
    mut commands: Commands,
) {
    if scale.speed == 0.0 {
        return;
    }
    timer.0 += time.delta_secs_f64() * scale.speed;
    if timer.0 < CHILD_BIRTH_INTERVAL {
        return;
    }
    timer.0 -= CHILD_BIRTH_INTERVAL;

    for house in houses.iter() {
        // Couple present?
        let adults = chars
            .iter()
            .filter(|c| c.house_id == house.id && c.stage == LifeStage::Adult)
            .count();
        if adults < 2 {
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
        let (r, g, b) = CHAR_COLORS[house.id % CHAR_COLORS.len()];

        info!(
            "[BIRTH] House #{}: child born (now {} children, storage: {})",
            house.id, children + 1, house.storage,
        );

        commands.spawn((
            Character {
                speed: 80.0,
                state: AiState::Idle,
                timer: 2.0,
                action_tile: None,
                plot_id: house.id,
                house_id: house.id,
                stage: LifeStage::Child,
                age: 0.0,
            },
            Growing { age: 0.0 },
            Transform::from_xyz(hx + ox, hy + oy, 2.0),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            spawn_child_sprite(parent, Color::srgb(r, g, b));
        });
    }
}

fn aging_system(
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut commands: Commands,
    mut chars: Query<(Entity, &mut Character, Option<&mut Growing>, &Transform)>,
    mut death_events: ResMut<DeathEvents>,
) {
    if scale.speed == 0.0 {
        return;
    }
    let dt = time.delta_secs_f64() * scale.speed;
    for (entity, mut ch, mut growing, tf) in chars.iter_mut() {
        ch.age += dt;
        if let Some(ref mut g) = growing {
            g.age = ch.age;
        }
        if ch.stage == LifeStage::Adult && ch.age > LIFESPAN {
            info!(
                "[DEATH] Adult died at house #{} (age: {:.0})",
                ch.house_id, ch.age,
            );
            death_events.deaths.push((ch.house_id, ch.age, tf.translation.x, tf.translation.y));
            commands.entity(entity).despawn();
        }
    }
}

/// When an adult dies, their eldest child inherits the household.
fn inheritance_system(
    mut commands: Commands,
    chars: Query<(Entity, &Character, Option<&Growing>)>,
    death_events: Res<DeathEvents>,
) {
    for &(house_id, _age, _dx, _dy) in &death_events.deaths {
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
                "[INHERIT] House #{}: eldest child inherits (age: {:.0})",
                house_id, child_age,
            );
            commands.entity(child_entity).remove::<Growing>();
            commands.entity(child_entity).insert(Character {
                speed: 100.0,
                state: AiState::Idle,
                timer: 1.0,
                action_tile: None,
                plot_id: house_id,
                house_id,
                stage: LifeStage::Adult,
                age: child_age,
            });
        } else {
            info!("[INHERIT] House #{}: no heir — house vacant", house_id);
        }
    }
}

/// Place a skull at each character's death location.
fn grave_system(
    mut commands: Commands,
    mut death_events: ResMut<DeathEvents>,
) {
    for (house_id, age, dx, dy) in death_events.deaths.drain(..) {
        info!(
            "[SKULL] House #{} died at age {:.0} pos ({:.0}, {:.0})",
            house_id, age, dx, dy,
        );

        commands.spawn((
            Grave,
            GraveInfo { age, house_id, cause: "Old Age" },
            Sprite {
                color: Color::srgb(0.92, 0.88, 0.82),
                custom_size: Some(Vec2::new(10.0, 10.0)),
                ..default()
            },
            Transform::from_xyz(dx, dy, 1.5),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            // Left eye
            parent.spawn((
                Sprite {
                    color: Color::srgb(0.10, 0.10, 0.10),
                    custom_size: Some(Vec2::new(2.0, 2.5)),
                    ..default()
                },
                Transform::from_xyz(-2.5, 1.0, 0.01),
                GlobalTransform::default(),
                Visibility::default(),
            ));
            // Right eye
            parent.spawn((
                Sprite {
                    color: Color::srgb(0.10, 0.10, 0.10),
                    custom_size: Some(Vec2::new(2.0, 2.5)),
                    ..default()
                },
                Transform::from_xyz(2.5, 1.0, 0.01),
                GlobalTransform::default(),
                Visibility::default(),
            ));
            // Mouth
            parent.spawn((
                Sprite {
                    color: Color::srgb(0.10, 0.10, 0.10),
                    custom_size: Some(Vec2::new(4.0, 1.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, -2.5, 0.01),
                GlobalTransform::default(),
                Visibility::default(),
            ));
        });
    }
}

fn child_growth_system(
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut commands: Commands,
    mut chars: Query<(Entity, &mut Character, &mut Growing, &Transform)>,
    adults: Query<&Character, Without<Growing>>,
    farm_tiles: Query<&FarmTile>,
    houses: Query<&House>,
    vegetation: Query<(&Vegetation, &Transform), Without<Character>>,
    map: Res<Map>,
    mut next_id: ResMut<NextSettlementId>,
    mut pending_farmland: ResMut<PendingFarmland>,
) {
    if scale.speed == 0.0 {
        return;
    }
    let dt = time.delta_secs_f64() * scale.speed;

    let existing_plots: Vec<(usize, usize)> = {
        let mut list: Vec<(usize, usize)> = farm_tiles.iter().map(|ft| (ft.tile_x, ft.tile_y)).collect();
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

    // Build a mask of tiles that have trees (wood source for building)
    let mut tree_mask = vec![false; MAP_WIDTH * MAP_HEIGHT];
    for (veg, tf) in vegetation.iter() {
        let is_tree = matches!(veg.kind, VegetationKind::DeciduousTree | VegetationKind::PineTree | VegetationKind::PalmTree);
        if is_tree {
            let tx = (tf.translation.x / TILE_SIZE) as usize;
            let ty = (tf.translation.y / TILE_SIZE) as usize;
            if tx < MAP_WIDTH && ty < MAP_HEIGHT {
                tree_mask[ty * MAP_WIDTH + tx] = true;
            }
        }
    }

    // Count adults per house (using a Without<Growing> query for disjoint access)
    let mut adult_counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for ch in adults.iter() {
        if ch.stage == LifeStage::Adult {
            *adult_counts.entry(ch.house_id).or_insert(0) += 1;
        }
    }

    let mut to_grow: Vec<(Entity, f32, f32, usize, usize)> = Vec::new();

    for (entity, ch, mut growing, tf) in chars.iter_mut() {
        if ch.stage != LifeStage::Child {
            continue;
        }
        growing.age += dt;
        if growing.age >= CHILD_GROWTH_DURATION {
            to_grow.push((entity, tf.translation.x, tf.translation.y, ch.plot_id, ch.house_id));
        }
    }

    for (entity, x, y, _plot_id, house_id) in to_grow {
        commands.entity(entity).despawn();

        let adult_count = adult_counts.get(&house_id).copied().unwrap_or(0);
        let (r, g, b) = CHAR_COLORS[house_id % CHAR_COLORS.len()];
        let color = Color::srgb(r, g, b);

        // If parents are still alive, the grown child may move out.
        if adult_count >= 2 {
            // Find the parents' house position
            let home_pos = houses.iter().find(|h| h.id == house_id).map(|h| {
                ((h.tile_x as f32 + h.w as f32 / 2.0) * TILE_SIZE,
                 (h.tile_y as f32 + h.h as f32 / 2.0) * TILE_SIZE)
            });

            if let Some((hx, hy)) = home_pos {
                // Search for a suitable expansion site
                if let Some((plot, house_tile, char_tile)) =
                    find_expansion_site(&map, &tree_mask, &existing_plots, hx, hy, 10)
                {
                    let sid = next_id.0;
                    next_id.0 += 1;

                    info!(
                        "[EXPAND] Settlement #{} founded at ({}, {}) — {} tiles, parent house #{}",
                        sid, house_tile.0, house_tile.1, plot.len(), house_id,
                    );

                    // Spawn 2 starter tiles, rest as pending
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
                                    color: color_for_state(CropState::Fallow),
                                    custom_size: Some(Vec2::new(TILE_SIZE - 2.0, TILE_SIZE - 2.0)),
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

                    // Spawn house
                    let (hx, hy) = tile_center(house_tile.0, house_tile.1);
                    commands.spawn((
                        House {
                            id: sid,
                            tile_x: house_tile.0,
                            tile_y: house_tile.1,
                            w: 2,
                            h: 2,
                            storage: 5,
                            essentials: HOUSE_START_ESSENTIALS / 2,
                        },
                        Sprite {
                            color: Color::srgb(0.82, 0.71, 0.55),
                            custom_size: Some(Vec2::new(40.0, 36.0)),
                            ..default()
                        },
                        Transform::from_xyz(hx, hy, 1.3),
                        GlobalTransform::default(),
                        Visibility::default(),
                    ))
                    .with_children(|parent| spawn_house_building(parent));

                    // Spawn settler character
                    let (cx, cy) = tile_center(char_tile.0, char_tile.1);
                    commands.spawn((
                        Character {
                            speed: 100.0,
                            state: AiState::Idle,
                            timer: 1.0,
                            action_tile: None,
                            plot_id: sid,
                            house_id: sid,
                            stage: LifeStage::Adult,
                            age: rand::random::<f64>() * 50.0 + 20.0,
                        },
                        Transform::from_xyz(cx, cy, 2.0),
                        GlobalTransform::default(),
                        Visibility::default(),
                    ))
                    .with_children(|parent| spawn_character_sprite(parent, color));

                    continue;
                } else {
                    info!(
                        "[GROW] House #{}: child grew up — expansion FAILED (no land), staying at home",
                        house_id,
                    );
                }
            }
        } else {
            info!(
                "[GROW] House #{}: child grew up — staying at home (helper, now {} adults)",
                house_id, adult_count + 1,
            );
        }

        // Stay at home: grow up on the original plot
        commands.spawn((
            Character {
                speed: 100.0,
                state: AiState::Idle,
                timer: 1.0,
                action_tile: None,
                plot_id: house_id, // same plot as parents
                house_id,
                stage: LifeStage::Adult,
                age: CHILD_GROWTH_DURATION,
            },
            Transform::from_xyz(x, y, 2.0),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| spawn_character_sprite(parent, color));
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
) -> Option<SettlementSite> {
    let arable = [TileType::Grass, TileType::Meadow];
    let start_x = (near_x / TILE_SIZE) as isize;
    let start_y = (near_y / TILE_SIZE) as isize;

    // Build a "used" mask from existing plot tiles.
    // Also keep a separate "occupied" mask (with 1-tile buffer zone) for house
    // overlap checks — the `used` mask gets modified by flood-fill and includes
    // buffer tiles, while `occupied` keeps the pre-flood-fill state with spacing.
    let mut used = vec![false; MAP_WIDTH * MAP_HEIGHT];
    let mut occupied = vec![false; MAP_WIDTH * MAP_HEIGHT];
    for (ex, ey) in existing {
        used[ey * MAP_WIDTH + ex] = true;
        // 1-tile buffer zone around every existing tile (houses, farms, etc.)
        for bx in -1..=1isize {
            for by in -1..=1isize {
                let nx = *ex as isize + bx;
                let ny = *ey as isize + by;
                if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize {
                    occupied[ny as usize * MAP_WIDTH + nx as usize] = true;
                }
            }
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
                let char_pos = (house_pos.0 + 1, house_pos.1 + 1);

                // Verify house tiles (2×2) and char tile are not already occupied
                let house_tiles = [
                    (house_pos.0, house_pos.1),
                    (house_pos.0 + 1, house_pos.1),
                    (house_pos.0, house_pos.1 + 1),
                    (house_pos.0 + 1, house_pos.1 + 1),
                ];
                if house_tiles.iter().any(|&(x, y)| {
                    y >= MAP_HEIGHT || x >= MAP_WIDTH || occupied[y * MAP_WIDTH + x]
                }) {
                    continue;
                }
                if occupied[char_pos.1 * MAP_WIDTH + char_pos.0] {
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
/// Starts at 3 because the initial map has 3 settlements (IDs 0, 1, 2).
#[derive(Resource)]
pub struct NextSettlementId(pub usize);

impl Default for NextSettlementId {
    fn default() -> Self {
        Self(3)
    }
}

/// Death info: house_id, age, world_x, world_y (for inheritance + skull placement).
#[derive(Resource, Default)]
pub struct DeathEvents {
    pub deaths: Vec<(usize, f64, f32, f32)>,
}

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
#[derive(Resource)]
pub struct RoadWear {
    pub wear: Vec<u32>,
}

impl Default for RoadWear {
    fn default() -> Self {
        Self { wear: vec![0; MAP_WIDTH * MAP_HEIGHT] }
    }
}

/// Tracks which tiles already have a road overlay sprite.
#[derive(Resource)]
pub struct RoadRender {
    pub tiles: Vec<Option<Entity>>,
}

impl Default for RoadRender {
    fn default() -> Self {
        Self { tiles: vec![None; MAP_WIDTH * MAP_HEIGHT] }
    }
}

/// Sim-seconds counter for periodic essentials consumption.
#[derive(Resource, Default)]
pub struct EssentialsTimer(pub f64);

/// Households periodically consume essentials — when they run low an adult goes
/// to the shop to restock.
fn essentials_depletion(
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut timer: ResMut<EssentialsTimer>,
    mut houses: Query<&mut House>,
) {
    if scale.speed == 0.0 {
        return;
    }
    timer.0 += time.delta_secs_f64() * scale.speed;
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

/// Periodically check road wear and spawn/update semi-transparent path overlays
/// on frequently walked tiles.
fn road_render_system(
    mut commands: Commands,
    mut sprites: Query<&mut Sprite, With<RoadTile>>,
    road_wear: Res<RoadWear>,
    mut road_render: ResMut<RoadRender>,
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

        // Alpha scales with wear: barely visible → well-worn path
        let alpha = ((wear as f32 / ROAD_THRESHOLD_3 as f32) * 0.45).min(0.45).max(0.06);

        if let Some(entity) = entry {
            // Update existing sprite's colour
            if let Ok(mut sprite) = sprites.get_mut(*entity) {
                let new = Color::srgba(0.45, 0.30, 0.15, alpha);
                if sprite.color != new {
                    sprite.color = new;
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
                    color: Color::srgba(0.45, 0.30, 0.15, alpha),
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

fn daily_consumption(
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut timer: ResMut<MealTimer>,
    mut child_timer: ResMut<ChildMealTimer>,
    chars: Query<&Character>,
    mut houses: Query<&mut House>,
) {
    if scale.speed == 0.0 {
        return;
    }
    let dt = time.delta_secs_f64() * scale.speed;

    // Adults: eat daily
    timer.0 += dt;
    if timer.0 >= DAY_SECONDS {
        timer.0 -= DAY_SECONDS;
        for ch in chars.iter() {
            if ch.stage == LifeStage::Child {
                continue;
            }
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

    // Children: eat every 2 days
    child_timer.0 += dt;
    if child_timer.0 >= DAY_SECONDS * 2.0 {
        child_timer.0 -= DAY_SECONDS * 2.0;
        for ch in chars.iter() {
            if ch.stage != LifeStage::Child {
                continue;
            }
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
