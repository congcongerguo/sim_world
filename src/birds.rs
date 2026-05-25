use bevy::prelude::*;

use crate::map::{TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

const BIRD_COUNT: usize = 25;
const BIRD_Z: f32 = 3.0;

/// A bird flying in the sky.
#[derive(Component)]
pub struct Bird {
    phase: f32,
    base_x: f32,
    base_y: f32,
    radius: f32,
    speed: f32,
}

pub struct BirdPlugin;

impl Plugin for BirdPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_birds);
        app.add_systems(Update, animate_birds);
    }
}

fn spawn_birds(mut commands: Commands, mut rng: ResMut<SimRng>) {
    let world_w = MAP_WIDTH as f32 * TILE_SIZE;
    let world_h = MAP_HEIGHT as f32 * TILE_SIZE;

    for _ in 0..BIRD_COUNT {
        let x = rng.gen_f64() as f32 * world_w;
        let y = rng.gen_f64() as f32 * world_h;
        let phase = rng.gen_f64() as f32 * std::f32::consts::TAU;
        let radius = 8.0 + rng.gen_f64() as f32 * 25.0;
        let speed = 0.3 + rng.gen_f64() as f32 * 0.5;

        commands.spawn((
            Bird { phase, base_x: x, base_y: y, radius, speed },
            Sprite {
                color: Color::srgba(0.85, 0.85, 0.90, 0.9),
                custom_size: Some(Vec2::new(4.0, 4.0)),
                ..default()
            },
            Transform::from_xyz(x, y, BIRD_Z),
            GlobalTransform::default(),
            Visibility::default(),
        ));
    }
}

fn animate_birds(
    time: Res<Time>,
    mut birds: Query<(&mut Bird, &mut Transform)>,
) {
    let dt = time.delta_secs();

    for (mut bird, mut tf) in birds.iter_mut() {
        bird.phase += dt * bird.speed;

        let dx = bird.phase.cos() * bird.radius;
        let dy = (bird.phase * 0.7).sin() * bird.radius * 0.4;

        tf.translation.x = bird.base_x + dx;
        tf.translation.y = bird.base_y + dy;
    }
}
