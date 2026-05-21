use bevy::prelude::*;

use crate::map::{Map, TileCategory, TileContent, TileEntry, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

const CAVE_Z: f32 = -0.5;

/// A cave below the surface.
#[derive(Component)]
pub struct Cave;

pub struct CavePlugin;

impl Plugin for CavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_caves);
    }
}

fn spawn_caves(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
    mut tile_content: ResMut<TileContent>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let tile = map.tiles[y * MAP_WIDTH + x];

            // Only spawn caves under rocky / mountainous terrain
            if !matches!(tile, TileType::Stone | TileType::Dirt | TileType::Tundra) {
                continue;
            }

            // ~1.2 % chance per eligible tile
            if rng.gen_f64() > 0.012 {
                continue;
            }

            let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
            let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0;

            commands.spawn((
                Cave,
                Sprite {
                    color: Color::srgba(0.12, 0.08, 0.06, 0.55),
                    custom_size: Some(Vec2::new(12.0, 12.0)),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, CAVE_Z),
                GlobalTransform::default(),
                Visibility::default(),
            ));

            tile_content.data.entry(y * MAP_WIDTH + x).or_default().push(TileEntry {
                name: "Cave",
                category: TileCategory::Cave,
                amount: 0,
                w: 1,
                h: 1,
            });
        }
    }
}
