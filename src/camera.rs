use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use crate::map;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera);
        app.add_systems(Update, camera_control);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_camera(mut commands: Commands) {
    let center = Vec3::new(
        map::MAP_WIDTH as f32 * map::TILE_SIZE / 2.0,
        map::MAP_HEIGHT as f32 * map::TILE_SIZE / 2.0,
        0.0,
    );

    // Start zoomed in enough to see individual tiles clearly.
    let init_scale = 1.0;

    commands.spawn((
        Camera2d,
        Transform::from_translation(center),
        OrthographicProjection {
            scale: init_scale,
            ..OrthographicProjection::default_2d()
        },
        GlobalTransform::default(),
    ));
}

// ---------------------------------------------------------------------------
// Pan + Zoom (cursor-centered)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PanState {
    last_cursor: Option<Vec2>,
}

fn camera_control(
    mut pan_state: Local<PanState>,
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut scroll_events: EventReader<MouseWheel>,
    mut camera_q: Query<
        (&Camera, &GlobalTransform, &mut Transform, &mut OrthographicProjection),
        With<Camera2d>,
    >,
) {
    let Ok((cam, cam_global, mut transform, mut projection)) = camera_q.get_single_mut() else {
        return;
    };

    // --- zoom (centered on cursor, reversed: scroll up = zoom out) ---
    let cursor_pos = windows
        .get_single()
        .ok()
        .and_then(|w| w.cursor_position());

    for ev in scroll_events.read() {
        let old_scale = projection.scale;
        // Reversed: scroll up (positive y) zooms out, scroll down zooms in
        projection.scale = (projection.scale * (1.0_f32 - ev.y * 0.15_f32))
            .clamp(0.1_f32, 100.0_f32);
        let new_scale = projection.scale;

        // Adjust camera so the world point under the cursor stays fixed.
        if let Some(cursor) = cursor_pos {
            if let Ok(world_pos) = cam.viewport_to_world_2d(cam_global, cursor) {
                let ratio = new_scale / old_scale;
                let old_pos = transform.translation.truncate();
                let new_pos = world_pos + (old_pos - world_pos) * ratio;
                transform.translation.x = new_pos.x;
                transform.translation.y = new_pos.y;
            }
        }
    }

    // --- pan with left-mouse drag ---
    let Ok(window) = windows.get_single() else {
        return;
    };
    let cursor = window.cursor_position();

    if mouse.pressed(MouseButton::Left) {
        if let (Some(pos), Some(last)) = (cursor, pan_state.last_cursor) {
            let delta = pos - last;
            // Scale pan speed by zoom level so drag distance matches world distance
            transform.translation.x -= delta.x * projection.scale;
            transform.translation.y += delta.y * projection.scale;
        }
        pan_state.last_cursor = cursor;
    } else {
        pan_state.last_cursor = None;
    }
}
