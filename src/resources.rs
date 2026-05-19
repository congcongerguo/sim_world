use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::map::{Map, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Resource types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ResourceKind {
    IronOre,
    CoalOre,
    CopperOre,
    GoldOre,
    ClayDeposit,
    SandDeposit,
    StoneDeposit,
}

impl ResourceKind {
    pub fn color(&self) -> Color {
        match self {
            ResourceKind::IronOre => Color::srgb(0.50, 0.40, 0.35),
            ResourceKind::CoalOre => Color::srgb(0.15, 0.15, 0.15),
            ResourceKind::CopperOre => Color::srgb(0.80, 0.50, 0.20),
            ResourceKind::GoldOre => Color::srgb(0.90, 0.80, 0.15),
            ResourceKind::ClayDeposit => Color::srgb(0.65, 0.45, 0.30),
            ResourceKind::SandDeposit => Color::srgb(0.82, 0.75, 0.55),
            ResourceKind::StoneDeposit => Color::srgb(0.45, 0.42, 0.40),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ResourceKind::IronOre => "Iron Ore",
            ResourceKind::CoalOre => "Coal",
            ResourceKind::CopperOre => "Copper Ore",
            ResourceKind::GoldOre => "Gold Ore",
            ResourceKind::ClayDeposit => "Clay",
            ResourceKind::SandDeposit => "Sand",
            ResourceKind::StoneDeposit => "Stone",
        }
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// A resource deposit on the map (ore, clay, etc.)
#[derive(Component)]
pub struct Resource {
    pub kind: ResourceKind,
    pub amount: u32,
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
// Spawning rules
// ---------------------------------------------------------------------------

struct SpawnRule {
    kind: ResourceKind,
    terrain: &'static [TileType],
    chance: f64,        // 0..1 probability per eligible tile
    min_amount: u32,
    max_amount: u32,
    overlay_size: f32,  // visual size in pixels
}

const RULES: &[SpawnRule] = &[
    SpawnRule {
        kind: ResourceKind::IronOre,
        terrain: &[TileType::Stone, TileType::Dirt],
        chance: 0.06,
        min_amount: 30,
        max_amount: 120,
        overlay_size: 10.0,
    },
    SpawnRule {
        kind: ResourceKind::CoalOre,
        terrain: &[TileType::Stone, TileType::Dirt],
        chance: 0.08,
        min_amount: 40,
        max_amount: 160,
        overlay_size: 10.0,
    },
    SpawnRule {
        kind: ResourceKind::CopperOre,
        terrain: &[TileType::Stone],
        chance: 0.04,
        min_amount: 20,
        max_amount: 80,
        overlay_size: 10.0,
    },
    SpawnRule {
        kind: ResourceKind::GoldOre,
        terrain: &[TileType::Stone],
        chance: 0.012,
        min_amount: 10,
        max_amount: 40,
        overlay_size: 8.0,
    },
    SpawnRule {
        kind: ResourceKind::ClayDeposit,
        terrain: &[TileType::Clay],
        chance: 0.25,
        min_amount: 20,
        max_amount: 80,
        overlay_size: 10.0,
    },
    SpawnRule {
        kind: ResourceKind::SandDeposit,
        terrain: &[TileType::Sand, TileType::Desert],
        chance: 0.15,
        min_amount: 30,
        max_amount: 100,
        overlay_size: 10.0,
    },
    SpawnRule {
        kind: ResourceKind::StoneDeposit,
        terrain: &[TileType::Stone, TileType::Dirt, TileType::Tundra],
        chance: 0.12,
        min_amount: 50,
        max_amount: 200,
        overlay_size: 12.0,
    },
];

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn spawn_resources(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
    mut counts: ResMut<ResourceCounts>,
) {
    counts.by_kind.clear();

    for rule in RULES {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = map.tiles[y * MAP_WIDTH + x];
                if !rule.terrain.contains(&tile) {
                    continue;
                }
                if rng.gen_f64() >= rule.chance {
                    continue;
                }

                let amount = rng.gen_range(rule.min_amount as usize, rule.max_amount as usize) as u32;
                let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0;

                commands.spawn((
                    Resource {
                        kind: rule.kind,
                        amount,
                    },
                    Sprite {
                        color: rule.kind.color(),
                        custom_size: Some(Vec2::new(rule.overlay_size, rule.overlay_size)),
                        anchor: Anchor::Center,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.1),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));

                *counts.by_kind.entry(rule.kind).or_insert(0) += 1;
            }
        }
    }
}
