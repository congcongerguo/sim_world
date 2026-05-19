use bevy::prelude::*;

use crate::map::{TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CLOUD_COUNT: usize = 60;
const CLOUD_Z: f32 = 5.0;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A cloud that drifts across the sky.
#[derive(Component)]
pub struct Cloud {
    pub speed: Vec2,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct CloudPlugin;

impl Plugin for CloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_clouds);
        app.add_systems(Update, animate_clouds);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn spawn_clouds(mut commands: Commands, mut rng: ResMut<SimRng>) {
    let world_w = MAP_WIDTH as f32 * TILE_SIZE;
    let world_h = MAP_HEIGHT as f32 * TILE_SIZE;

    for _ in 0..CLOUD_COUNT {
        let x = rng.gen_f64() as f32 * world_w;
        let y = rng.gen_f64() as f32 * world_h;
        let w = 40.0 + rng.gen_f64() as f32 * 100.0;
        let h = 12.0 + rng.gen_f64() as f32 * 35.0;
        let alpha = 0.15 + rng.gen_f64() as f32 * 0.45;

        // gentle horizontal drift with slight vertical wobble
        let speed = Vec2::new(
            -3.0 + rng.gen_f64() as f32 * 6.0,
            -0.5 + rng.gen_f64() as f32 * 1.0,
        );

        commands.spawn((
            Cloud { speed },
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            },
            Transform::from_xyz(x, y, CLOUD_Z),
            GlobalTransform::default(),
            Visibility::default(),
        ));
    }
}

fn animate_clouds(
    time: Res<Time>,
    mut clouds: Query<(&Cloud, &mut Transform)>,
) {
    let world_w = MAP_WIDTH as f32 * TILE_SIZE;
    let world_h = MAP_HEIGHT as f32 * TILE_SIZE;
    let margin_w = world_w * 0.15;
    let margin_h = world_h * 0.15;
    let dt = time.delta_secs();

    for (cloud, mut tf) in clouds.iter_mut() {
        tf.translation.x += cloud.speed.x * dt;
        tf.translation.y += cloud.speed.y * dt;

        // wrap around when a cloud drifts too far off the map edge
        if tf.translation.x < -margin_w {
            tf.translation.x += world_w + margin_w * 2.0;
        } else if tf.translation.x > world_w + margin_w {
            tf.translation.x -= world_w + margin_w * 2.0;
        }
        if tf.translation.y < -margin_h {
            tf.translation.y += world_h + margin_h * 2.0;
        } else if tf.translation.y > world_h + margin_h {
            tf.translation.y -= world_h + margin_h * 2.0;
        }
    }
}
