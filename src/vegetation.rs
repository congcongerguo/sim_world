use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::element_config::{Interaction, VEGETATION_CONFIGS, VEG_SPAWN_RULES};
use crate::map::{Map, TileCategory, TileContent, TileEntry, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Vegetation kinds
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum VegetationKind {
    DeciduousTree,
    PineTree,
    PalmTree,
    Bush,
    Flower,
    DeadBush,
    Cactus,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A vegetation entity (tree, bush, flower) on the map.
/// Fields are baked from element_config at spawn time.
#[derive(Component)]
pub struct Vegetation {
    pub kind: VegetationKind,
    pub name: &'static str,
    pub interaction: Interaction,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_vegetation);
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn spawn_vegetation(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
    mut tile_content: ResMut<TileContent>,
) {
    for rule in VEG_SPAWN_RULES {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = map.tiles[y * MAP_WIDTH + x];
                if !rule.terrain.contains(&tile) {
                    continue;
                }
                if rng.gen_f64() >= rule.chance {
                    continue;
                }

                let cfg = &VEGETATION_CONFIGS[rule.kind as u8 as usize];

                // Slight random offset so vegetation doesn't look grid-aligned
                let ox = (rng.gen_f64() - 0.5) * 6.0_f64;
                let oy = (rng.gen_f64() - 0.5) * 6.0_f64;
                let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0 + ox as f32;
                let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0 + oy as f32;

                commands.spawn((
                    Vegetation {
                        kind: rule.kind,
                        name: cfg.name_en,
                        interaction: cfg.interaction,
                    },
                    Sprite {
                        color: cfg.color,
                        custom_size: Some(Vec2::new(cfg.size, cfg.size)),
                        anchor: Anchor::Center,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.0),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));

                tile_content.data.entry(y * MAP_WIDTH + x).or_default().push(TileEntry {
                    name: cfg.name_en,
                    category: TileCategory::Vegetation,
                    amount: 0,
                    w: 1,
                    h: 1,
                });
            }
        }
    }
}
