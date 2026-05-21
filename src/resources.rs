use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::element_config::{Interaction, RESOURCE_CONFIGS};
use crate::map::{Map, TileCategory, TileContent, TileEntry, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Resource types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum ResourceKind {
    IronOre,
    CoalOre,
    CopperOre,
    GoldOre,
    ClayDeposit,
    SandDeposit,
    StoneDeposit,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// A resource deposit on the map (ore, clay, etc.)
/// Fields are baked from element_config at spawn time.
#[derive(Component)]
pub struct Resource {
    pub kind: ResourceKind,
    pub amount: u32,
    pub name: &'static str,
    pub interaction: Interaction,
}

/// Total game resource counts (for UI / debugging)
#[derive(Resource, Default)]
pub struct ResourceCounts {
    pub by_kind: std::collections::HashMap<ResourceKind, u32>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ResourcePlugin;

impl Plugin for ResourcePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResourceCounts>();
        app.add_systems(PostStartup, spawn_resources);
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn spawn_resources(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
    mut counts: ResMut<ResourceCounts>,
    mut tile_content: ResMut<TileContent>,
) {
    counts.by_kind.clear();

    for cfg in RESOURCE_CONFIGS {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = map.tiles[y * MAP_WIDTH + x];
                if !cfg.spawn.terrain.contains(&tile) {
                    continue;
                }
                if rng.gen_f64() >= cfg.spawn.chance {
                    continue;
                }

                let amount = rng.gen_range(cfg.spawn.min_amount as usize, cfg.spawn.max_amount as usize) as u32;
                let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0;

                commands.spawn((
                    Resource {
                        kind: cfg.kind,
                        amount,
                        name: cfg.name_en,
                        interaction: cfg.interaction,
                    },
                    Sprite {
                        color: cfg.color,
                        custom_size: Some(Vec2::new(cfg.overlay_size, cfg.overlay_size)),
                        anchor: Anchor::Center,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.1),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));

                tile_content.data.entry(y * MAP_WIDTH + x).or_default().push(TileEntry {
                    name: cfg.name_en,
                    category: TileCategory::Resource,
                    amount,
                    w: 1,
                    h: 1,
                });

                *counts.by_kind.entry(cfg.kind).or_insert(0) += 1;
            }
        }
    }
}
