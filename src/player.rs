use bevy::prelude::*;

use crate::actions::ActionEvent;
use crate::assets::GameAssets;
use crate::farmland::{color_for_clearing, setup_farm_layout, CropState, FarmLayout, FarmTile, PendingFarmland};
use crate::map::{Map, TileCategory, TileContent, TileEntry, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_time::{TimeScale, MONTH, YEAR};
use crate::vegetation::{Vegetation, VegetationKind};

// ---------------------------------------------------------------------------
// Constants — expressed in game days (1 tick ≈ 1 day)
// 1 month = 30 days, 1 year = 360 days
// ---------------------------------------------------------------------------

/// Days between adult meals.
const MEAL_INTERVAL: f64 = 12.0;
/// Minimum days between birth attempts (~1.3 months).
const CHILD_BIRTH_INTERVAL: f64 = 40.0;
/// Days for a child to grow into an adult (2 months).
const CHILD_GROWTH_DURATION: f64 = 2.0 * MONTH;
/// Max children per household.
const MAX_CHILDREN: usize = 3;
/// Max age for reproduction (~6.7 months).
const MAX_REPRODUCTION_AGE: f64 = 200.0;
/// Lifespan (~1 year).
const LIFESPAN: f64 = 1.0 * YEAR;
/// Days a house can have 0 food before an adult starves (2 months).
const STARVATION_THRESHOLD: f64 = 2.0 * MONTH;

/// Graveyard top-left tile on the map.
const GRAVEYARD_X: usize = 3;
const GRAVEYARD_Y: usize = 110;
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
const ROAD_THRESHOLD_3: u32 = 40;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
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
        app.add_systems(PostStartup, (
            spawn_characters.after(setup_farm_layout),
            spawn_houses.after(setup_farm_layout),
            spawn_shop.after(setup_farm_layout),
        ));
        app.add_systems(Update, (
            romance_system,
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
            custom_size: Some(Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 2.0)),
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

        // Spawn 2 adults per house (husband & wife)
        for offset_x in [0i8, 1] {
            let (x, y) = tile_center(
                (tx as i8 + offset_x) as usize,
                ty,
            );
            let gender = if offset_x == 0 { Gender::Male } else { Gender::Female };
            let tex = if offset_x == 0 {
                assets.char_male.clone()
            } else {
                assets.char_female.clone()
            };
            commands.spawn((
                Character {
                    speed: 100.0,
                    state: AiState::Idle,
                    timer: offset_x as f64,
                    action_tile: None,
                    plot_id: i,
                    house_id: i,
                    stage: LifeStage::Adult,
                    age: rand::random::<f64>() * 100.0 + 50.0,
                    gender,
                    personality: Personality::random(),
                    marital: MaritalStatus::Married,
                },
                Transform::from_xyz(x, y, 2.0),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .with_children(|parent| {
                spawn_character_sprite(parent, tex);
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
                storage: 20,
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

/// Spawn the village shop near the first settlement.
fn spawn_shop(
    mut commands: Commands,
    layout: Res<FarmLayout>,
    assets: Res<GameAssets>,
    mut tile_content: ResMut<TileContent>,
) {
    // Place shop a few tiles from the first house
    let (hx, hy) = layout.houses[0];
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
    farm_set: &std::collections::HashSet<(usize, usize)>,
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

    for (otx, oty) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let tx = ctx + otx;
        let ty = cty + oty;
        if tx < 0 || tx >= MAP_WIDTH as isize || ty < 0 || ty >= MAP_HEIGHT as isize {
            continue;
        }
        let (tux, tuy) = (tx as usize, ty as usize);
        // Farm tiles are off-limits for non-farm movement
        if farm_set.contains(&(tux, tuy)) {
            // Check if the character's destination is a farm tile (allow access)
            let dest_tx = (to.0 / TILE_SIZE) as usize;
            let dest_ty = (to.1 / TILE_SIZE) as usize;
            if !farm_set.contains(&(dest_tx, dest_ty)) {
                continue; // skip — character is just passing through
            }
            // Fall through: character is heading to a farm tile, allow normal scoring
        }
        let terrain_speed = match map.tiles[tuy * MAP_WIDTH + tux] {
            TileType::Grass | TileType::Meadow | TileType::Dirt => 1.0,
            TileType::Sand | TileType::Clay => 0.85,
            TileType::Desert => 0.75,
            TileType::Tundra => 0.6,
            TileType::Forest | TileType::Snow => 0.5,
            TileType::Stone => 0.4,
            TileType::Swamp => 0.3,
            TileType::Ice => 0.65,
            _ => 0.0,
        };
        let mem = path_memory.counts[tuy * MAP_WIDTH + tux] as f32;
        let weight = terrain_speed * (1.0 + mem * 0.1);
        nudge_x += otx as f32 * weight;
        nudge_y += oty as f32 * weight;
    }

    dir_x += nudge_x * 0.05;
    dir_y += nudge_y * 0.05;
    let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
    if len > 0.01 {
        (dir_x / len, dir_y / len)
    } else {
        (dx / dist, dy / dist)
    }
}

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
    mut path_memory: ResMut<PathMemory>,
    mut pending_farmland: ResMut<PendingFarmland>,
    assets: Res<GameAssets>,
    road_render: Res<RoadRender>,
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

    // Build farm tile position set — non-farm characters avoid walking here
    let farm_set: std::collections::HashSet<(usize, usize)> = farm_tiles.iter()
        .map(|(ft, _)| (ft.tile_x, ft.tile_y))
        .collect();

    // Road mask: tiles with road sprites cannot have houses built on them
    let road_mask: Vec<bool> = road_render.tiles.iter().map(|e| e.is_some()).collect();

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
                            let wx = hx + (rand::random::<f32>() - 0.5) * 200.0;
                            let wy = hy + (rand::random::<f32>() - 0.5) * 200.0;
                            ch.state = AiState::MoveTo(wx, wy, false);
                        }
                        ch.timer = 2.0;
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
                            ch.state = AiState::MoveTo(wx, wy, true);
                        } else {
                            // Check for pending tiles to clear
                            let pending_target = pending_farmland.plots.get(&ch.plot_id)
                                .and_then(|tiles| tiles.first().copied());

                            if let Some((px, py)) = pending_target {
                                let (wx, wy) = tile_center(px, py);
                                ch.state = AiState::MoveTo(wx, wy, true);
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
                                    // Single adults: go socialize near the shop instead of idling at home
                                    if ch.marital == MaritalStatus::Single && rand::random::<f32>() < 0.4 {
                                        let shop_cx = shop_location.tile_x as f32 * TILE_SIZE + TILE_SIZE;
                                        let shop_cy = shop_location.tile_y as f32 * TILE_SIZE + TILE_SIZE;
                                        let ox = (rand::random::<f32>() - 0.5) * 64.0;
                                        let oy = (rand::random::<f32>() - 0.5) * 64.0;
                                        ch.state = AiState::GoingToSocial(shop_cx + ox, shop_cy + oy);
                                        ch.timer = 3.0;
                                    } else {
                                        // Only married/widowed characters can explore and expand
                                        let can_lead = matches!(ch.marital, MaritalStatus::Married | MaritalStatus::Widowed);

                                        // Check if we can explore (enough stored food)
                                        let house_storage = houses.iter()
                                            .find(|h| h.id == ch.house_id)
                                            .map(|h| h.storage)
                                            .unwrap_or(0);
                                        let can_explore = can_lead && house_storage >= 8;

                                        info!(
                                            "[AI_DEBUG] House#{} char can_lead={} storage={} => can_explore={}",
                                            ch.house_id, can_lead, house_storage, can_explore,
                                        );

                                        if can_explore {
                                            let angle = rand::random::<f32>() * std::f32::consts::TAU;
                                            info!(
                                                "[EXPLORE] House #{} adult exploring at ({:.0}, {:.0}) dir ({:.2}, {:.2})",
                                                ch.house_id, pos.0, pos.1, angle.cos(), angle.sin(),
                                            );
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
                                                ch.state = AiState::MoveTo(hx, hy, true);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            AiState::MoveTo(wx, wy, purposeful) => {
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
                    // Terrain + path memory biased direction
                    let (bdx, bdy) = biased_dir((pos.0, pos.1), (wx, wy), &map, &path_memory, &farm_set);
                    let tile_speed = tile_speed_multiplier(&map, &tf);
                    let speed = ch.speed * scale.speed as f32 * tile_speed;
                    tf.translation.x += bdx * speed * time.delta_secs();
                    tf.translation.y += bdy * speed * time.delta_secs();

                    // Push away from farmland if not heading there for work
                    let (ntx, nty) = current_tile(&tf);
                    let dest_is_farm = farm_set.contains(&((wx / TILE_SIZE) as usize, (wy / TILE_SIZE) as usize));
                    if !dest_is_farm && farm_set.contains(&(ntx, nty)) {
                        // Undo the last step — slide back off the farm tile
                        tf.translation.x -= bdx * speed * time.delta_secs() * 2.0;
                        tf.translation.y -= bdy * speed * time.delta_secs() * 2.0;
                    }

                    // Road wear & path memory — only for purposeful movement
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
                let shop_wx = shop_tile_x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let shop_wy = shop_tile_y as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let dx = shop_wx - pos.0;
                let dy = shop_wy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < 2.0 {
                    // Arrived at shop — signal process_actions to handle trade
                    ch.action_tile = Some((shop_tile_x, shop_tile_y));
                } else {
                    let (bdx, bdy) = biased_dir((pos.0, pos.1), (shop_wx, shop_wy), &map, &path_memory, &farm_set);
                    let tile_speed = tile_speed_multiplier(&map, &tf);
                    let speed = ch.speed * scale.speed as f32 * tile_speed;
                    tf.translation.x += bdx * speed * time.delta_secs();
                    tf.translation.y += bdy * speed * time.delta_secs();

                    // Push away from farmland (shop is never on a farm tile)
                    let (ntx, nty) = current_tile(&tf);
                    if farm_set.contains(&(ntx, nty)) {
                        tf.translation.x -= bdx * speed * time.delta_secs() * 2.0;
                        tf.translation.y -= bdy * speed * time.delta_secs() * 2.0;
                    }

                    // Road wear & path memory — shop visits are always purposeful
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
                if dist < 2.0 {
                    ch.state = AiState::Socializing;
                    ch.timer = 8.0 + rand::random::<f64>() * 8.0;
                } else {
                    let tile_speed = tile_speed_multiplier(&map, &tf);
                    let speed = ch.speed * scale.speed as f32 * tile_speed;
                    tf.translation.x += dx / dist * speed * time.delta_secs();
                    tf.translation.y += dy / dist * speed * time.delta_secs();
                }
            }

            AiState::Socializing => {
                ch.timer -= dt;
                if ch.timer <= 0.0 {
                    // Done socializing — go home
                    if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                        let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                        let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                        ch.state = AiState::MoveTo(hx, hy, false);
                    } else {
                        ch.state = AiState::Idle;
                    }
                    ch.timer = 2.0;
                }
            }

            AiState::Exploring { origin_x, origin_y, dir_x, dir_y } => {
                let tile_speed = tile_speed_multiplier(&map, &tf);
                let speed = ch.speed * scale.speed as f32 * tile_speed;
                tf.translation.x += dir_x * speed * time.delta_secs();
                tf.translation.y += dir_y * speed * time.delta_secs();

                let (cx, cy) = (tf.translation.x, tf.translation.y);
                let dist = ((cx - origin_x).powi(2) + (cy - origin_y).powi(2)).sqrt();

                // Walked too far — go home
                if dist > 800.0 {
                    if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                        let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                        let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                        ch.state = AiState::MoveTo(hx, hy, false);
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
                    if home_food < 5 {
                        // Not enough food — abort and return home
                        if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                            let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                            let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                            ch.state = AiState::MoveTo(hx, hy, true);
                        } else {
                            ch.state = AiState::Idle;
                            ch.timer = 5.0;
                        }
                        continue;
                    }

                    let (tx, ty) = current_tile(&tf);
                    let mut tree_count = 0;
                    for dy in -5..=5isize {
                        for dx in -5..=5isize {
                            let nx = tx as isize + dx;
                            let ny = ty as isize + dy;
                            if nx >= 0 && nx < MAP_WIDTH as isize && ny >= 0 && ny < MAP_HEIGHT as isize
                                && tree_mask[ny as usize * MAP_WIDTH + nx as usize]
                            {
                                tree_count += 1;
                            }
                        }
                    }

                    if tree_count >= 2 {
                        // Found trees — try to build a new settlement
                        let (mx, my) = tile_center(tx, ty);
                        if let Some((plot, house_tile, char_tile)) =
                            find_expansion_site(&map, &tree_mask, &existing, mx, my, 10, &road_mask)
                        {
                            let sid = next_id.0;
                            next_id.0 += 1;

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
                                            image: assets.misc_farm_fallow.clone(),
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
                                Transform::from_xyz(hx, hy, 1.3),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| spawn_house_building(parent, assets.bld_house.clone()));

                            // Spawn settler couple (married)
                            let (cx, cy) = tile_center(char_tile.0, char_tile.1);
                            let couple_offset = 16.0;

                            // Male settler
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
                                    gender: Gender::Male,
                                    personality: Personality::random(),
                                    marital: MaritalStatus::Married,
                                },
                                Transform::from_xyz(cx - couple_offset, cy, 2.0),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| spawn_character_sprite(parent, assets.char_male.clone()));

                            // Female settler (partner)
                            commands.spawn((
                                Character {
                                    speed: 100.0,
                                    state: AiState::Idle,
                                    timer: 1.5,
                                    action_tile: None,
                                    plot_id: sid,
                                    house_id: sid,
                                    stage: LifeStage::Adult,
                                    age: rand::random::<f64>() * 50.0 + 20.0,
                                    gender: Gender::Female,
                                    personality: Personality::random(),
                                    marital: MaritalStatus::Married,
                                },
                                Transform::from_xyz(cx + couple_offset, cy, 2.0),
                                GlobalTransform::default(),
                                Visibility::default(),
                            ))
                            .with_children(|parent| spawn_character_sprite(parent, assets.char_female.clone()));

                            // Explorer returns home
                            if let Some(house) = houses.iter().find(|h| h.id == ch.house_id) {
                                let hx = (house.tile_x as f32 + house.w as f32 / 2.0) * TILE_SIZE;
                                let hy = (house.tile_y as f32 + house.h as f32 / 2.0) * TILE_SIZE;
                                ch.state = AiState::MoveTo(hx, hy, false);
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
                AiState::MoveTo(hx, hy, true)
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

/// Sim-seconds between romance checks.
const ROMANCE_INTERVAL: f64 = 5.0;
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
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut timer: ResMut<RomanceTimer>,
    mut chars: Query<(Entity, &mut Character)>,
) {
    if scale.speed == 0.0 {
        return;
    }
    let dt = time.delta_secs_f64() * scale.speed;
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
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut timer: ResMut<ReproductionTimer>,
    chars: Query<&Character>,
    houses: Query<&House>,
    mut commands: Commands,
    assets: Res<GameAssets>,
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
        // Married/widowed couple present? (both must be below reproduction age)
        let married_adults = chars
            .iter()
            .filter(|c| {
                c.house_id == house.id
                    && c.stage == LifeStage::Adult
                    && c.age < MAX_REPRODUCTION_AGE
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
                plot_id: house.id,
                house_id: house.id,
                stage: LifeStage::Child,
                age: 0.0,
                gender: baby_gender,
                personality: Personality::random(),
                marital: MaritalStatus::Single,
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
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut commands: Commands,
    mut chars: Query<(Entity, &mut Character, Option<&mut Growing>, &Transform, &mut Visibility)>,
    children_q: Query<&Children>,
    mut death_events: ResMut<DeathEvents>,
    mut houses: Query<&mut House>,
    mut starvation_timer: ResMut<StarvationTimer>,
) {
    if scale.speed == 0.0 {
        return;
    }
    // Clear previous frame's death tiles before recording new ones
    death_events.tiles.clear();
    let dt = time.delta_secs_f64() * scale.speed;
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
    for mut house in houses.iter_mut() {
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
                plot_id: house_id,
                house_id,
                stage: LifeStage::Adult,
                age: child_age,
                gender: child_gender,
                personality: child_personality,
                marital: MaritalStatus::Widowed, // head of household — can expand
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

        let (tile_x, tile_y, cx, cy) = if near_home {
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
    time: Res<Time>,
    scale: Res<TimeScale>,
    mut commands: Commands,
    mut chars: Query<(Entity, &mut Character, &mut Growing, &Transform)>,
    adults: Query<&Character, Without<Growing>>,
    children_q: Query<&Children>,
    assets: Res<GameAssets>,
) {
    if scale.speed == 0.0 {
        return;
    }
    let dt = time.delta_secs_f64() * scale.speed;

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
            plot_id: house_id,
            house_id,
            stage: LifeStage::Adult,
            age: CHILD_GROWTH_DURATION,
            gender,
            personality,
            marital: MaritalStatus::Single,
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
#[derive(Resource)]
pub struct RoadWear {
    pub wear: Vec<u32>,
}

impl Default for RoadWear {
    fn default() -> Self {
        Self { wear: vec![0; MAP_WIDTH * MAP_HEIGHT] }
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
    if timer.0 >= MEAL_INTERVAL {
        timer.0 -= MEAL_INTERVAL;
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
    if child_timer.0 >= MEAL_INTERVAL * 2.0 {
        child_timer.0 -= MEAL_INTERVAL * 2.0;
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
