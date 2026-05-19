use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;

pub const TILE_SIZE: f32 = 32.0;
pub const MAP_WIDTH: usize = 1024;
pub const MAP_HEIGHT: usize = 1024;

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

impl TileType {
    pub fn name(&self) -> &'static str {
        match self {
            TileType::Grass => "Grass",
            TileType::Water => "Water",
            TileType::DeepWater => "Deep Water",
            TileType::Sand => "Sand",
            TileType::Forest => "Forest",
            TileType::Swamp => "Swamp",
            TileType::Stone => "Stone",
            TileType::Dirt => "Dirt",
            TileType::Snow => "Snow",
            TileType::Lava => "Lava",
            TileType::Tundra => "Tundra",
            TileType::Ice => "Ice",
            TileType::Meadow => "Meadow",
            TileType::Desert => "Desert",
            TileType::Clay => "Clay",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            TileType::Grass => Color::srgb(0.30, 0.70, 0.20),
            TileType::Water => Color::srgb(0.20, 0.35, 0.80),
            TileType::DeepWater => Color::srgb(0.08, 0.15, 0.50),
            TileType::Sand => Color::srgb(0.76, 0.70, 0.50),
            TileType::Forest => Color::srgb(0.10, 0.50, 0.10),
            TileType::Swamp => Color::srgb(0.25, 0.45, 0.20),
            TileType::Stone => Color::srgb(0.50, 0.50, 0.50),
            TileType::Dirt => Color::srgb(0.55, 0.40, 0.25),
            TileType::Snow => Color::srgb(0.95, 0.95, 0.95),
            TileType::Lava => Color::srgb(0.80, 0.20, 0.05),
            TileType::Tundra => Color::srgb(0.60, 0.65, 0.55),
            TileType::Ice => Color::srgb(0.85, 0.90, 0.95),
            TileType::Meadow => Color::srgb(0.50, 0.80, 0.25),
            TileType::Desert => Color::srgb(0.85, 0.75, 0.40),
            TileType::Clay => Color::srgb(0.65, 0.45, 0.30),
        }
    }

    /// Return RGBA bytes for the tile color (sRGB space).
    fn rgba_bytes(&self) -> [u8; 4] {
        let c = self.color().to_srgba();
        [
            (c.red * 255.0) as u8,
            (c.green * 255.0) as u8,
            (c.blue * 255.0) as u8,
            (c.alpha * 255.0) as u8,
        ]
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

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
    pub fn get(&self, x: usize, y: usize) -> Option<&TileType> {
        if x < self.width && y < self.height {
            Some(&self.tiles[y * self.width + x])
        } else {
            None
        }
    }

    pub fn set(&mut self, x: usize, y: usize, tile: TileType) {
        if x < self.width && y < self.height {
            self.tiles[y * self.width + x] = tile;
        }
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

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Map>();
        app.init_resource::<SelectedTile>();
        app.add_systems(Startup, spawn_map);
        app.add_systems(Update, tile_edit_input);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

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

    // Build procedural texture from tile data.
    // Texture row 0 = top of image = top of sprite = highest world Y.
    // World tile (x, y=0) is at the bottom → texture row MAP_HEIGHT-1.
    let mut pixel_data = vec![0u8; MAP_WIDTH * MAP_HEIGHT * 4];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = y * MAP_WIDTH + x;
            let rgba = map.tiles[idx].rgba_bytes();
            let data_y = MAP_HEIGHT - 1 - y; // flip Y
            let pi = (data_y * MAP_WIDTH + x) * 4;
            pixel_data[pi] = rgba[0];
            pixel_data[pi + 1] = rgba[1];
            pixel_data[pi + 2] = rgba[2];
            pixel_data[pi + 3] = rgba[3];
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: MAP_WIDTH as u32,
            height: MAP_HEIGHT as u32,
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

    // --- place tile on left click ---
    if !mouse.just_pressed(MouseButton::Left) {
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

    // Update the terrain texture pixel(s).
    if let Some(image) = images.get_mut(&map_image.0) {
        let rgba = new_type.rgba_bytes();
        let data_y = MAP_HEIGHT - 1 - ty; // flip Y
        let pi = (data_y * MAP_WIDTH + tx) * 4;
        image.data[pi] = rgba[0];
        image.data[pi + 1] = rgba[1];
        image.data[pi + 2] = rgba[2];
        image.data[pi + 3] = rgba[3];
    }
}
