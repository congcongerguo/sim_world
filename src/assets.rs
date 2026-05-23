use bevy::prelude::*;

use crate::map::{Map, MapTileImage, MAP_WIDTH, MAP_HEIGHT};

// ---------------------------------------------------------------------------
// Resource – one handle per generated pixel-art texture
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct GameAssets {
    // Terrain (15)
    pub terrain: [Handle<Image>; 15],
    // Vegetation (7)
    pub veg_deciduous_tree: Handle<Image>,
    pub veg_pine_tree: Handle<Image>,
    pub veg_palm_tree: Handle<Image>,
    pub veg_bush: Handle<Image>,
    pub veg_flower: Handle<Image>,
    pub veg_dead_bush: Handle<Image>,
    pub veg_cactus: Handle<Image>,
    // Resources (7)
    pub res_iron_ore: Handle<Image>,
    pub res_coal: Handle<Image>,
    pub res_copper_ore: Handle<Image>,
    pub res_gold_ore: Handle<Image>,
    pub res_clay: Handle<Image>,
    pub res_sand: Handle<Image>,
    pub res_stone: Handle<Image>,
    // Features (7)
    pub feat_rock_formation: Handle<Image>,
    pub feat_ruins: Handle<Image>,
    pub feat_ancient_tree: Handle<Image>,
    pub feat_hot_spring: Handle<Image>,
    pub feat_geyser: Handle<Image>,
    pub feat_meteor_crater: Handle<Image>,
    pub feat_fossil: Handle<Image>,
    // Buildings (5)
    pub bld_house: Handle<Image>,
    pub bld_stone_house: Handle<Image>,
    pub bld_watchtower: Handle<Image>,
    pub bld_workshop: Handle<Image>,
    pub bld_well: Handle<Image>,
    // Characters (4)
    pub char_male: Handle<Image>,
    pub char_female: Handle<Image>,
    pub char_child: Handle<Image>,
    pub char_guard: Handle<Image>,
    // Misc (7)
    pub misc_shop: Handle<Image>,
    pub misc_tombstone: Handle<Image>,
    pub misc_road: Handle<Image>,
    pub misc_farm_fallow: Handle<Image>,
    pub misc_farm_growing: Handle<Image>,
    pub misc_farm_weedy: Handle<Image>,
    pub misc_farm_ready: Handle<Image>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_game_assets);
        app.add_systems(Update, build_terrain_texture.after(load_game_assets));
    }
}

fn load_game_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let p = |s: &str| format!("pixel_prototypes/{}", s);

    commands.insert_resource(GameAssets {
        terrain: [
            asset_server.load(p("terrain/grass.png")),        // 0=Grass
            asset_server.load(p("terrain/water.png")),        // 1=Water
            asset_server.load(p("terrain/deep_water.png")),   // 2=DeepWater
            asset_server.load(p("terrain/sand.png")),         // 3=Sand
            asset_server.load(p("terrain/forest_floor.png")), // 4=Forest
            asset_server.load(p("terrain/swamp.png")),        // 5=Swamp
            asset_server.load(p("terrain/stone_ground.png")), // 6=Stone
            asset_server.load(p("terrain/dirt.png")),         // 7=Dirt
            asset_server.load(p("terrain/snow.png")),         // 8=Snow
            asset_server.load(p("terrain/lava.png")),         // 9=Lava
            asset_server.load(p("terrain/tundra.png")),       // 10=Tundra
            asset_server.load(p("terrain/ice.png")),          // 11=Ice
            asset_server.load(p("terrain/meadow.png")),       // 12=Meadow
            asset_server.load(p("terrain/desert.png")),       // 13=Desert
            asset_server.load(p("terrain/clay.png")),         // 14=Clay
        ],
        veg_deciduous_tree: asset_server.load(p("vegetation/deciduous_tree.png")),
        veg_pine_tree:      asset_server.load(p("vegetation/pine_tree.png")),
        veg_palm_tree:      asset_server.load(p("vegetation/palm_tree.png")),
        veg_bush:           asset_server.load(p("vegetation/bush.png")),
        veg_flower:         asset_server.load(p("vegetation/flower.png")),
        veg_dead_bush:      asset_server.load(p("vegetation/dead_bush.png")),
        veg_cactus:         asset_server.load(p("vegetation/cactus.png")),

        res_iron_ore:   asset_server.load(p("resources/iron_ore.png")),
        res_coal:       asset_server.load(p("resources/coal.png")),
        res_copper_ore: asset_server.load(p("resources/copper_ore.png")),
        res_gold_ore:   asset_server.load(p("resources/gold_ore.png")),
        res_clay:       asset_server.load(p("resources/clay_deposit.png")),
        res_sand:       asset_server.load(p("resources/sand_deposit.png")),
        res_stone:      asset_server.load(p("resources/stone_deposit.png")),

        feat_rock_formation: asset_server.load(p("features/rock_formation.png")),
        feat_ruins:          asset_server.load(p("features/ancient_ruins.png")),
        feat_ancient_tree:   asset_server.load(p("features/ancient_tree.png")),
        feat_hot_spring:     asset_server.load(p("features/hot_spring.png")),
        feat_geyser:         asset_server.load(p("features/geyser.png")),
        feat_meteor_crater:  asset_server.load(p("features/meteor_crater.png")),
        feat_fossil:         asset_server.load(p("features/fossil.png")),

        bld_house:       asset_server.load(p("buildings/house.png")),
        bld_stone_house: asset_server.load(p("buildings/stone_house.png")),
        bld_watchtower:  asset_server.load(p("buildings/watchtower.png")),
        bld_workshop:    asset_server.load(p("buildings/workshop.png")),
        bld_well:        asset_server.load(p("buildings/well.png")),

        char_male:   asset_server.load(p("characters/character_male.png")),
        char_female: asset_server.load(p("characters/character_female.png")),
        char_child:  asset_server.load(p("characters/character_child.png")),
        char_guard:  asset_server.load(p("characters/character_guard.png")),

        misc_shop:         asset_server.load(p("misc/shop.png")),
        misc_tombstone:    asset_server.load(p("misc/tombstone.png")),
        misc_road:         asset_server.load(p("misc/road_path.png")),
        misc_farm_fallow:  asset_server.load(p("misc/farm_fallow.png")),
        misc_farm_growing: asset_server.load(p("misc/farm_growing.png")),
        misc_farm_weedy:   asset_server.load(p("misc/farm_weedy.png")),
        misc_farm_ready:   asset_server.load(p("misc/farm_ready.png")),
    });
}



// ---------------------------------------------------------------------------
// Terrain texture builder – procedurally generates the full 3200×3200 texture
// with per-tile variation and smooth biome transitions.
// ---------------------------------------------------------------------------

/// Size of each terrain tile in the output texture (pixels).
const TILE_PX: u32 = 32;

/// Transition-zone width in pixels (blend between biomes).
const BLEND_PX: u32 = 4;

// ---------------------------------------------------------------------------
// Terrain palettes (3–5 colours each, sRGB)
// ---------------------------------------------------------------------------

type Palette = &'static [(u8, u8, u8)];

const PAL_GRASS:     Palette = &[(86, 175, 92)];
const PAL_WATER:     Palette = &[(40, 135, 210)];
const PAL_DEEP:      Palette = &[(14, 50, 130)];
const PAL_SAND:      Palette = &[(242, 225, 190)];
const PAL_FOREST:    Palette = &[(28, 100, 38)];
const PAL_SWAMP:     Palette = &[(55, 80, 35)];
const PAL_STONE:     Palette = &[(100, 100, 100)];
const PAL_DIRT:      Palette = &[(115, 80, 68)];
const PAL_SNOW:      Palette = &[(240, 242, 245)];
const PAL_LAVA:      Palette = &[(215, 75, 15)];
const PAL_TUNDRA:    Palette = &[(115, 125, 95)];
const PAL_ICE:       Palette = &[(185, 225, 252)];
const PAL_MEADOW:    Palette = &[(80, 175, 72)];
const PAL_DESERT:    Palette = &[(225, 170, 95)];
const PAL_CLAY:      Palette = &[(150, 95, 68)];

const TERRAIN_PALETTES: [Palette; 15] = [
    PAL_GRASS, PAL_WATER, PAL_DEEP, PAL_SAND, PAL_FOREST,
    PAL_SWAMP, PAL_STONE, PAL_DIRT, PAL_SNOW, PAL_LAVA,
    PAL_TUNDRA, PAL_ICE, PAL_MEADOW, PAL_DESERT, PAL_CLAY,
];

// ---------------------------------------------------------------------------
// Value noise – smoothly interpolated on a coarse grid, naturally
// continuous across tile boundaries because it uses global pixel coords.
// ---------------------------------------------------------------------------

fn hash_u32(gx: u32, gy: u32, seed: u32) -> u32 {
    let mut h = gx.wrapping_mul(374761393) ^ gy.wrapping_mul(668265263) ^ seed.wrapping_mul(1274126177);
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    h
}

fn hash_f32(gx: u32, gy: u32, seed: u32) -> f32 {
    hash_u32(gx, gy, seed) as f32 / u32::MAX as f32
}

/// Smoothstep for C1 continuity.
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise on a grid of `cell_size` pixels.
fn value_noise(px: u32, py: u32, cell_size: u32, seed: u32) -> f32 {
    let gx = px / cell_size;
    let gy = py / cell_size;
    let fx = (px % cell_size) as f32 / cell_size as f32;
    let fy = (py % cell_size) as f32 / cell_size as f32;
    let sx = smoothstep(fx);
    let sy = smoothstep(fy);

    let v00 = hash_f32(gx, gy, seed);
    let v10 = hash_f32(gx.wrapping_add(1), gy, seed);
    let v01 = hash_f32(gx, gy.wrapping_add(1), seed);
    let v11 = hash_f32(gx.wrapping_add(1), gy.wrapping_add(1), seed);

    let top = v00 + (v10 - v00) * sx;
    let bot = v01 + (v11 - v01) * sx;
    top + (bot - top) * sy
}

/// Combined noise for a pixel at global position (px, py)
/// for terrain type `tt`.  Uses a single smooth octave so tiles look
/// mostly solid with very gentle gradients – no leopard-print texture.
fn terrain_noise(px: u32, py: u32, tt: usize) -> f32 {
    let seed = tt as u32;
    value_noise(px, py, 32, seed)
}

fn pick_color(val: f32, palette: Palette) -> (u8, u8, u8) {
    let idx = ((val * palette.len() as f32).floor() as usize).min(palette.len() - 1);
    palette[idx]
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn lerp_color(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        lerp_u8(a.0, b.0, t),
        lerp_u8(a.1, b.1, t),
        lerp_u8(a.2, b.2, t),
    )
}

/// Blend factor at tile edges when neighbour has different terrain.
fn edge_blend(
    tx: u32, ty: u32,
    x: usize, y: usize,
    map: &Map,
) -> Option<(f32, Palette)> {
    let d_left   = tx;
    let d_right  = TILE_PX - 1 - tx;
    let d_top    = ty;
    let d_bottom = TILE_PX - 1 - ty;
    let min_d = d_left.min(d_right).min(d_top).min(d_bottom);

    if min_d >= BLEND_PX {
        return None;
    }

    let t = 1.0 - (min_d as f32 / BLEND_PX as f32);

    let (nx, ny) = if d_left == min_d && x > 0 {
        (x - 1, y)
    } else if d_right == min_d && x + 1 < MAP_WIDTH {
        (x + 1, y)
    } else if d_bottom == min_d && y > 0 {
        (x, y - 1)
    } else if d_top == min_d && y + 1 < MAP_HEIGHT {
        (x, y + 1)
    } else {
        return None;
    };

    let neighbor_tt = map.tiles[ny * MAP_WIDTH + nx] as usize;
    let this_tt = map.tiles[y * MAP_WIDTH + x] as usize;

    if neighbor_tt == this_tt {
        return None; // same biome, no blend
    }

    Some((t, TERRAIN_PALETTES[neighbor_tt]))
}

fn build_terrain_texture(
    mut images: ResMut<Assets<Image>>,
    map: Res<Map>,
    map_image: ResMut<MapTileImage>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }

    let out_w = MAP_WIDTH as u32 * TILE_PX;
    let out_h = MAP_HEIGHT as u32 * TILE_PX;
    let mut data = vec![0u8; (out_w * out_h * 4) as usize];

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let tt = map.tiles[y * MAP_WIDTH + x] as usize;
            let palette = TERRAIN_PALETTES[tt];

            // Y-flip: row 0 of map = bottom of texture
            let dst_y_base = (MAP_HEIGHT - 1 - y) as u32 * TILE_PX;

            for ty in 0..TILE_PX {
                for tx in 0..TILE_PX {
                    let px = x as u32 * TILE_PX + tx;
                    let global_py = (MAP_HEIGHT as u32 - 1 - y as u32) * TILE_PX + ty;

                    let noise = terrain_noise(px, global_py, tt);
                    let mut color = pick_color(noise, palette);

                    // Edge transition to neighbouring biome
                    if let Some((blend, neighbor_pal)) = edge_blend(tx, ty, x, y, &map) {
                        let n_noise = terrain_noise(px, global_py, tt);
                        let n_col = pick_color(n_noise, neighbor_pal);
                        color = lerp_color(color, n_col, blend);
                    }

                    let di = ((dst_y_base + ty) * out_w + x as u32 * TILE_PX + tx) as usize * 4;
                    data[di]     = color.0;
                    data[di + 1] = color.1;
                    data[di + 2] = color.2;
                    data[di + 3] = 255;
                }
            }
        }
    }

    if let Some(img) = images.get_mut(&map_image.0) {
        img.data = data;
        info!("[TERRAIN] Procedural texture built ({}x{}) with biome transitions", out_w, out_h);
    }
    *done = true;
}
