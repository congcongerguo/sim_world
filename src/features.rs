use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::map::{Map, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Feature kinds
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FeatureKind {
    RockFormation,
    Ruins,
    AncientTree,
    HotSpring,
    Geyser,
    MeteorCrater,
    Fossil,
}

impl FeatureKind {
    pub fn color(&self) -> Color {
        match self {
            FeatureKind::RockFormation => Color::srgb(0.40, 0.38, 0.35),
            FeatureKind::Ruins => Color::srgb(0.45, 0.35, 0.25),
            FeatureKind::AncientTree => Color::srgb(0.08, 0.35, 0.08),
            FeatureKind::HotSpring => Color::srgb(0.60, 0.80, 0.90),
            FeatureKind::Geyser => Color::srgb(0.70, 0.85, 0.95),
            FeatureKind::MeteorCrater => Color::srgb(0.35, 0.25, 0.15),
            FeatureKind::Fossil => Color::srgb(0.70, 0.65, 0.55),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FeatureKind::RockFormation => "Rock Formation",
            FeatureKind::Ruins => "Ancient Ruins",
            FeatureKind::AncientTree => "Ancient Tree",
            FeatureKind::HotSpring => "Hot Spring",
            FeatureKind::Geyser => "Geyser",
            FeatureKind::MeteorCrater => "Meteor Crater",
            FeatureKind::Fossil => "Fossil",
        }
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A special point of interest on the map.
#[derive(Component)]
pub struct Feature {
    pub kind: FeatureKind,
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
// Spawning rules
// ---------------------------------------------------------------------------

struct FeatureRule {
    kind: FeatureKind,
    terrain: &'static [TileType],
    chance: f64,
    size: f32,
}

const FEATURE_RULES: &[FeatureRule] = &[
    FeatureRule {
        kind: FeatureKind::RockFormation,
        terrain: &[TileType::Stone, TileType::Dirt],
        chance: 0.02,
        size: 16.0,
    },
    FeatureRule {
        kind: FeatureKind::Ruins,
        terrain: &[TileType::Desert, TileType::Grass, TileType::Sand],
        chance: 0.003,
        size: 18.0,
    },
    FeatureRule {
        kind: FeatureKind::AncientTree,
        terrain: &[TileType::Forest],
        chance: 0.005,
        size: 18.0,
    },
    FeatureRule {
        kind: FeatureKind::HotSpring,
        terrain: &[TileType::Stone, TileType::Tundra],
        chance: 0.008,
        size: 14.0,
    },
    FeatureRule {
        kind: FeatureKind::Geyser,
        terrain: &[TileType::Tundra, TileType::Snow],
        chance: 0.004,
        size: 0.0,    // will use the larger of width/height
    },
    FeatureRule {
        kind: FeatureKind::MeteorCrater,
        terrain: &[TileType::Desert, TileType::Tundra],
        chance: 0.002,
        size: 20.0,
    },
    FeatureRule {
        kind: FeatureKind::Fossil,
        terrain: &[TileType::Desert, TileType::Clay, TileType::Stone],
        chance: 0.006,
        size: 12.0,
    },
];

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn spawn_features(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
) {
    for rule in FEATURE_RULES {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = map.tiles[y * MAP_WIDTH + x];
                if !rule.terrain.contains(&tile) {
                    continue;
                }
                if rng.gen_f64() >= rule.chance {
                    continue;
                }

                let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0;
                let sz = rule.size;

                commands.spawn((
                    Feature { kind: rule.kind },
                    Sprite {
                        color: rule.kind.color(),
                        custom_size: Some(Vec2::new(sz, sz)),
                        anchor: Anchor::Center,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.2),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));
            }
        }
    }
}
