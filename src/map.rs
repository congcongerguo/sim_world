use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;

use crate::element_config::{Interaction, TERRAIN_CONFIGS};

pub const TILE_SIZE: f32 = 32.0;
pub const MAP_WIDTH: usize = 100;
pub const MAP_HEIGHT: usize = 100;

// ---------------------------------------------------------------------------
// Terrain types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum TileType {
    #[default]
    Grass = 0,
    Water = 1,
    DeepWater = 2,
    Sand = 3,
    Forest = 4,
    Swamp = 5,
    Stone = 6,
    Dirt = 7,
    Snow = 8,
    Lava = 9,
    Tundra = 10,
    Ice = 11,
    Meadow = 12,
    Desert = 13,
    Clay = 14,
}

/// Marks the single terrain sprite entity.
#[derive(Component)]
pub struct MapSurface;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// The flattened 2-D tile grid.
#[derive(Resource)]
pub struct Map {
    pub tiles: Vec<TileType>,
    pub width: usize,
    pub height: usize,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            tiles: Vec::new(),
            width: 0,
            height: 0,
        }
    }
}

impl Map {
    pub fn set(&mut self, x: usize, y: usize, tile: TileType) {
        if x < self.width && y < self.height {
            self.tiles[y * self.width + x] = tile;
        }
    }
}

/// Baked terrain data — populated once at startup from element_config,
/// never reads the config tables afterwards.
#[derive(Resource)]
pub struct TerrainData {
    pub names: [&'static str; 15],
    pub interactions: [Interaction; 15],
    /// Pre-computed RGBA bytes (sRGB) for each TileType.
    pub rgbs: [[u8; 4]; 15],
}

impl TerrainData {
    pub fn bake() -> Self {
        let mut names = [""; 15];
        let mut interactions = [Interaction::None; 15];
        let mut rgbs = [[0u8; 4]; 15];
        for cfg in TERRAIN_CONFIGS {
            let i = cfg.tile_type as u8 as usize;
            names[i] = cfg.name_en;
            interactions[i] = cfg.interaction;
            let srgb = cfg.color.to_srgba();
            rgbs[i] = [
                (srgb.red * 255.0) as u8,
                (srgb.green * 255.0) as u8,
                (srgb.blue * 255.0) as u8,
                (srgb.alpha * 255.0) as u8,
            ];
        }
        Self { names, interactions, rgbs }
    }
}

/// Holds the handle to the procedural terrain texture so we can update pixels on edit.
#[derive(Resource)]
pub struct MapTileImage(pub Handle<Image>);

/// The terrain type the player will place on the next left-click.
#[derive(Resource)]
pub struct SelectedTile(pub TileType);

impl Default for SelectedTile {
    fn default() -> Self {
        Self(TileType::Water)
    }
}

/// Tracks which grid cells are occupied by multi-tile entities (buildings).
/// Inserted during spawn_map, read by building spawn + UI.
#[derive(Resource)]
pub struct OccupancyGrid {
    pub cells: Vec<Option<Entity>>,
    width: usize,
    height: usize,
}

impl OccupancyGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![None; width * height],
            width,
            height,
        }
    }

    /// Check whether the rectangle [x, x+w) × [y, y+h) is entirely free.
    pub fn is_free(&self, x: usize, y: usize, w: usize, h: usize) -> bool {
        if x + w > self.width || y + h > self.height {
            return false;
        }
        for dy in 0..h {
            for dx in 0..w {
                if self.cells[(y + dy) * self.width + (x + dx)].is_some() {
                    return false;
                }
            }
        }
        true
    }

    /// Mark the rectangle as occupied by `entity`.
    pub fn occupy(&mut self, x: usize, y: usize, w: usize, h: usize, entity: Entity) {
        for dy in 0..h {
            for dx in 0..w {
                self.cells[(y + dy) * self.width + (x + dx)] = Some(entity);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TileContent spatial index – O(1) UI lookups, built during PostStartup spawn
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TileCategory {
    Resource,
    Vegetation,
    Feature,
    Building,
    Cave,
}

#[derive(Clone)]
pub struct TileEntry {
    pub name: &'static str,
    pub category: TileCategory,
    pub amount: u32,
    pub w: u32,
    pub h: u32,
}

/// Maps tile index → overlays on that tile.
#[derive(Resource, Default)]
pub struct TileContent {
    pub data: HashMap<usize, Vec<TileEntry>>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Map>();
        app.init_resource::<SelectedTile>();
        app.init_resource::<TileContent>();
        app.add_systems(Startup, spawn_map);
        app.add_systems(Update, tile_edit_input);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Pixels-per-tile for the terrain texture (each map tile = 32×32 px).
const TERRAIN_TILE_PX: u32 = 32;

fn spawn_map(
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut images: ResMut<Assets<Image>>,
) {
    map.width = MAP_WIDTH;
    map.height = MAP_HEIGHT;
    if map.tiles.is_empty() {
        map.tiles = vec![TileType::Grass; MAP_WIDTH * MAP_HEIGHT];
    }

    // Bake terrain data from config into a persistent Resource.
    // After this, all runtime code reads from TerrainData, never from config.
    let td = TerrainData::bake();

    // Build a placeholder terrain texture at full resolution (3200×3200 px).
    // Each 32×32 block is filled with the flat terrain colour.
    // An Update system (build_terrain_texture) will replace the pixel data
    // with tiled pixel-art once the generated tile images are loaded.
    let tex_w = MAP_WIDTH as u32 * TERRAIN_TILE_PX;
    let tex_h = MAP_HEIGHT as u32 * TERRAIN_TILE_PX;
    let mut pixel_data = vec![0u8; (tex_w * tex_h * 4) as usize];

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = y * MAP_WIDTH + x;
            let tile = map.tiles[idx];
            let rgba = td.rgbs[tile as u8 as usize];
            // Y-flip: texture row 0 = top = highest world Y
            let dst_y_base = (MAP_HEIGHT - 1 - y) as u32 * TERRAIN_TILE_PX;

            for ty in 0..TERRAIN_TILE_PX {
                for tx in 0..TERRAIN_TILE_PX {
                    let pi = ((dst_y_base + ty) * tex_w + x as u32 * TERRAIN_TILE_PX + tx) as usize * 4;
                    pixel_data[pi + 0] = rgba[0];
                    pixel_data[pi + 1] = rgba[1];
                    pixel_data[pi + 2] = rgba[2];
                    pixel_data[pi + 3] = rgba[3];
                }
            }
        }
    }

    // Persist baked data for runtime systems (UI, editing).
    commands.insert_resource(td);

    // Initialise occupancy grid (buildings will occupy cells in PostStartup).
    let occ = OccupancyGrid::new(MAP_WIDTH, MAP_HEIGHT);
    commands.insert_resource(occ);

    let mut image = Image::new(
        Extent3d {
            width: tex_w,
            height: tex_h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixel_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // Nearest-neighbour filtering so tiles stay crisp when zoomed in.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        mipmap_filter: ImageFilterMode::Nearest,
        label: None,
        lod_min_clamp: 0.0,
        lod_max_clamp: f32::MAX,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    });

    let handle = images.add(image);
    commands.insert_resource(MapTileImage(handle.clone()));

    commands.spawn((
        MapSurface,
        Sprite {
            image: handle,
            custom_size: Some(Vec2::new(
                MAP_WIDTH as f32 * TILE_SIZE,
                MAP_HEIGHT as f32 * TILE_SIZE,
            )),
            anchor: Anchor::BottomLeft,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

fn tile_edit_input(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut selected: ResMut<SelectedTile>,
    mut map: ResMut<Map>,
    map_image: Res<MapTileImage>,
    mut images: ResMut<Assets<Image>>,
    terrain_data: Res<TerrainData>,
) {
    // --- switch selected terrain type via number keys ---
    if keys.just_pressed(KeyCode::Digit1) {
        selected.0 = TileType::Grass;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        selected.0 = TileType::Water;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        selected.0 = TileType::Sand;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        selected.0 = TileType::Forest;
    }
    if keys.just_pressed(KeyCode::Digit5) {
        selected.0 = TileType::Stone;
    }
    if keys.just_pressed(KeyCode::Digit6) {
        selected.0 = TileType::Snow;
    }
    if keys.just_pressed(KeyCode::Digit7) {
        selected.0 = TileType::DeepWater;
    }
    if keys.just_pressed(KeyCode::Digit8) {
        selected.0 = TileType::Swamp;
    }
    if keys.just_pressed(KeyCode::Digit9) {
        selected.0 = TileType::Dirt;
    }
    if keys.just_pressed(KeyCode::Digit0) {
        selected.0 = TileType::Lava;
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        selected.0 = TileType::Tundra;
    }
    if keys.just_pressed(KeyCode::KeyW) {
        selected.0 = TileType::Ice;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        selected.0 = TileType::Meadow;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        selected.0 = TileType::Desert;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        selected.0 = TileType::Clay;
    }

    // --- place tile on right click ---
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((cam, cam_global)) = camera_q.get_single() else {
        return;
    };
    let Ok(world_pos) = cam.viewport_to_world_2d(cam_global, cursor) else {
        return;
    };

    let tile_x = (world_pos.x / TILE_SIZE).floor() as isize;
    let tile_y = (world_pos.y / TILE_SIZE).floor() as isize;

    if tile_x < 0
        || tile_y < 0
        || tile_x >= map.width as isize
        || tile_y >= map.height as isize
    {
        return;
    }

    let (tx, ty) = (tile_x as usize, tile_y as usize);
    let new_type = selected.0;
    map.set(tx, ty, new_type);

    // Update the terrain texture — write a 32×32 block.
    if let Some(image) = images.get_mut(&map_image.0) {
        let rgba = terrain_data.rgbs[new_type as u8 as usize];
        let data_y = MAP_HEIGHT - 1 - ty; // flip Y
        let tex_w = MAP_WIDTH as u32 * TERRAIN_TILE_PX;

        for py in 0..TERRAIN_TILE_PX {
            for px in 0..TERRAIN_TILE_PX {
                let pi = ((data_y as u32 * TERRAIN_TILE_PX + py) * tex_w
                    + tx as u32 * TERRAIN_TILE_PX + px) as usize * 4;
                image.data[pi] = rgba[0];
                image.data[pi + 1] = rgba[1];
                image.data[pi + 2] = rgba[2];
                image.data[pi + 3] = rgba[3];
            }
        }
    }
}
