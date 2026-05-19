use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::map::{Map, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Vegetation kinds
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VegetationKind {
    DeciduousTree,
    PineTree,
    PalmTree,
    Bush,
    Flower,
    DeadBush,
    Cactus,
}

impl VegetationKind {
    pub fn name(&self) -> &'static str {
        match self {
            VegetationKind::DeciduousTree => "Deciduous Tree",
            VegetationKind::PineTree => "Pine Tree",
            VegetationKind::PalmTree => "Palm Tree",
            VegetationKind::Bush => "Bush",
            VegetationKind::Flower => "Flower",
            VegetationKind::DeadBush => "Dead Bush",
            VegetationKind::Cactus => "Cactus",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            VegetationKind::DeciduousTree => Color::srgb(0.15, 0.55, 0.12),
            VegetationKind::PineTree => Color::srgb(0.08, 0.40, 0.08),
            VegetationKind::PalmTree => Color::srgb(0.25, 0.60, 0.15),
            VegetationKind::Bush => Color::srgb(0.30, 0.55, 0.18),
            VegetationKind::Flower => Color::srgb(0.90, 0.30, 0.50),
            VegetationKind::DeadBush => Color::srgb(0.45, 0.35, 0.20),
            VegetationKind::Cactus => Color::srgb(0.25, 0.55, 0.20),
        }
    }

    pub fn size(&self) -> f32 {
        match self {
            VegetationKind::DeciduousTree => 14.0,
            VegetationKind::PineTree => 12.0,
            VegetationKind::PalmTree => 14.0,
            VegetationKind::Bush => 8.0,
            VegetationKind::Flower => 5.0,
            VegetationKind::DeadBush => 7.0,
            VegetationKind::Cactus => 10.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A vegetation entity (tree, bush, flower) on the map.
#[derive(Component)]
pub struct Vegetation {
    pub kind: VegetationKind,
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
// Spawning rules
// ---------------------------------------------------------------------------

struct VegRule {
    kind: VegetationKind,
    terrain: &'static [TileType],
    chance: f64,
}

const VEG_RULES: &[VegRule] = &[
    VegRule {
        kind: VegetationKind::DeciduousTree,
        terrain: &[TileType::Forest],
        chance: 0.35,
    },
    VegRule {
        kind: VegetationKind::DeciduousTree,
        terrain: &[TileType::Grass],
        chance: 0.06,
    },
    VegRule {
        kind: VegetationKind::DeciduousTree,
        terrain: &[TileType::Meadow],
        chance: 0.12,
    },
    VegRule {
        kind: VegetationKind::PineTree,
        terrain: &[TileType::Forest],
        chance: 0.15,
    },
    VegRule {
        kind: VegetationKind::PineTree,
        terrain: &[TileType::Tundra],
        chance: 0.08,
    },
    VegRule {
        kind: VegetationKind::PineTree,
        terrain: &[TileType::Stone],
        chance: 0.03,
    },
    VegRule {
        kind: VegetationKind::PalmTree,
        terrain: &[TileType::Sand],
        chance: 0.05,
    },
    VegRule {
        kind: VegetationKind::Bush,
        terrain: &[TileType::Grass, TileType::Meadow],
        chance: 0.10,
    },
    VegRule {
        kind: VegetationKind::Bush,
        terrain: &[TileType::Dirt, TileType::Tundra],
        chance: 0.05,
    },
    VegRule {
        kind: VegetationKind::Flower,
        terrain: &[TileType::Meadow, TileType::Grass],
        chance: 0.04,
    },
    VegRule {
        kind: VegetationKind::DeadBush,
        terrain: &[TileType::Desert],
        chance: 0.08,
    },
    VegRule {
        kind: VegetationKind::Cactus,
        terrain: &[TileType::Desert, TileType::Sand],
        chance: 0.03,
    },
];

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn spawn_vegetation(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
) {
    for rule in VEG_RULES {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = map.tiles[y * MAP_WIDTH + x];
                if !rule.terrain.contains(&tile) {
                    continue;
                }
                if rng.gen_f64() >= rule.chance {
                    continue;
                }

                // Slight random offset so vegetation doesn't look grid-aligned
                let ox = (rng.gen_f64() - 0.5) * 6.0_f64;
                let oy = (rng.gen_f64() - 0.5) * 6.0_f64;
                let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0 + ox as f32;
                let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0 + oy as f32;
                let sz = rule.kind.size();

                commands.spawn((
                    Vegetation { kind: rule.kind },
                    Sprite {
                        color: rule.kind.color(),
                        custom_size: Some(Vec2::new(sz, sz)),
                        anchor: Anchor::Center,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.0),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));
            }
        }
    }
}
