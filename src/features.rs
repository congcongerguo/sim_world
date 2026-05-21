use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::element_config::{Interaction, FEATURE_CONFIGS};
use crate::map::{Map, TileCategory, TileContent, TileEntry, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Feature kinds
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum FeatureKind {
    RockFormation,
    Ruins,
    AncientTree,
    HotSpring,
    Geyser,
    MeteorCrater,
    Fossil,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A special point of interest on the map.
/// Fields are baked from element_config at spawn time.
#[derive(Component)]
pub struct Feature {
    pub kind: FeatureKind,
    pub name: &'static str,
    pub interaction: Interaction,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct FeaturePlugin;

impl Plugin for FeaturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_features);
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn spawn_features(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
    mut tile_content: ResMut<TileContent>,
) {
    for cfg in FEATURE_CONFIGS {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = map.tiles[y * MAP_WIDTH + x];
                if !cfg.spawn.terrain.contains(&tile) {
                    continue;
                }
                if rng.gen_f64() >= cfg.spawn.chance {
                    continue;
                }

                let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0;

                commands.spawn((
                    Feature {
                        kind: cfg.kind,
                        name: cfg.name_en,
                        interaction: cfg.interaction,
                    },
                    Sprite {
                        color: cfg.color,
                        custom_size: Some(Vec2::new(cfg.size, cfg.size)),
                        anchor: Anchor::Center,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.2),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));

                tile_content.data.entry(y * MAP_WIDTH + x).or_default().push(TileEntry {
                    name: cfg.name_en,
                    category: TileCategory::Feature,
                    amount: 0,
                    w: 1,
                    h: 1,
                });
            }
        }
    }
}
