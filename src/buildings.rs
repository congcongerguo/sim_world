use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::assets::GameAssets;
use crate::element_config::{Interaction, BUILDING_CONFIGS};
use crate::map::{Map, OccupancyGrid, TileCategory, TileContent, TileEntry, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Building kinds
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BuildingKind {
    House,
    StoneHouse,
    Watchtower,
    Workshop,
    Well,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A multi-tile structure on the map.
/// Fields are baked from element_config at spawn time.
#[derive(Component)]
pub struct Building {
    pub kind: BuildingKind,
    pub name: &'static str,
    pub interaction: Interaction,
    pub anchor: IVec2,
    pub size: UVec2,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_buildings);
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn building_texture<'a>(kind: BuildingKind, assets: &'a GameAssets) -> &'a Handle<Image> {
    match kind {
        BuildingKind::House => &assets.bld_house,
        BuildingKind::StoneHouse => &assets.bld_stone_house,
        BuildingKind::Watchtower => &assets.bld_watchtower,
        BuildingKind::Workshop => &assets.bld_workshop,
        BuildingKind::Well => &assets.bld_well,
    }
}

fn spawn_buildings(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
    mut occ: ResMut<OccupancyGrid>,
    mut tile_content: ResMut<TileContent>,
    assets: Res<GameAssets>,
) {
    // Sort by area descending so larger buildings get first pick of locations.
    let mut cfgs: Vec<_> = BUILDING_CONFIGS.iter().collect();
    cfgs.sort_by(|a, b| (b.width * b.height).cmp(&(a.width * a.height)));

    for cfg in cfgs {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                // Quick terrain check on anchor tile
                let tile = map.tiles[y * MAP_WIDTH + x];
                if !cfg.spawn.terrain.contains(&tile) {
                    continue;
                }
                // Probability check (per anchor tile)
                if rng.gen_f64() >= cfg.spawn.chance {
                    continue;
                }
                // Out of bounds for the building footprint
                if x + cfg.width > MAP_WIDTH || y + cfg.height > MAP_HEIGHT {
                    continue;
                }
                // Full area terrain check + occupancy check
                let mut blocked = false;
                for dy in 0..cfg.height {
                    for dx in 0..cfg.width {
                        let cx = x + dx;
                        let cy = y + dy;
                        let ct = map.tiles[cy * MAP_WIDTH + cx];
                        if !cfg.spawn.terrain.contains(&ct)
                            || occ.cells[cy * MAP_WIDTH + cx].is_some()
                        {
                            blocked = true;
                            break;
                        }
                    }
                    if blocked {
                        break;
                    }
                }
                if blocked {
                    continue;
                }

                // Place the building
                let world_x = (x as f32 + cfg.width as f32 / 2.0) * TILE_SIZE;
                let world_y = (y as f32 + cfg.height as f32 / 2.0) * TILE_SIZE;

                let tex = building_texture(cfg.kind, &assets).clone();
                let entity = commands
                    .spawn((
                        Building {
                            kind: cfg.kind,
                            name: cfg.name_en,
                            interaction: cfg.interaction,
                            anchor: IVec2::new(x as i32, y as i32),
                            size: UVec2::new(cfg.width as u32, cfg.height as u32),
                        },
                        Sprite {
                            image: tex,
                            custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                            anchor: Anchor::Center,
                            ..default()
                        },
                        Transform::from_xyz(world_x, world_y, 1.3),
                        GlobalTransform::default(),
                        Visibility::default(),
                    ))
                    .id();

                occ.occupy(x, y, cfg.width, cfg.height, entity);

                // Index all occupied tiles for O(1) UI lookups
                for dy in 0..cfg.height {
                    for dx in 0..cfg.width {
                        let idx = (y + dy) * MAP_WIDTH + (x + dx);
                        tile_content.data.entry(idx).or_default().push(TileEntry {
                            name: cfg.name_en,
                            category: TileCategory::Building,
                            amount: 0,
                            w: cfg.width as u32,
                            h: cfg.height as u32,
                        });
                    }
                }
            }
        }
    }
}
