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

fn setup_camera(mut commands: Commands, windows: Query<&Window>) {
    let center = Vec3::new(
        map::MAP_WIDTH as f32 * map::TILE_SIZE / 2.0,
        map::MAP_HEIGHT as f32 * map::TILE_SIZE / 2.0,
        0.0,
    );

    let world_w = map::MAP_WIDTH as f32 * map::TILE_SIZE;
    let world_h = map::MAP_HEIGHT as f32 * map::TILE_SIZE;

    // Fit the entire map into the viewport on startup.
    // In Bevy 0.15, higher scale = larger visible area (more zoomed out).
    let init_scale = if let Ok(window) = windows.get_single() {
        (world_w / window.width()).max(world_h / window.height()) * 1.1
    } else {
        20.0
    };

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

    // --- zoom (centered on cursor) ---
    let cursor_pos = windows
        .get_single()
        .ok()
        .and_then(|w| w.cursor_position());

    for ev in scroll_events.read() {
        let old_scale = projection.scale;
        projection.scale = (projection.scale * (1.0_f32 + ev.y * 0.15_f32))
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

    // --- pan with middle-mouse drag ---
    let Ok(window) = windows.get_single() else {
        return;
    };
    let cursor = window.cursor_position();

    if mouse.pressed(MouseButton::Middle) {
        if let (Some(pos), Some(last)) = (cursor, pan_state.last_cursor) {
            let delta = pos - last;
            let inv_scale = 1.0 / projection.scale;
            transform.translation.x -= delta.x * inv_scale;
            transform.translation.y += delta.y * inv_scale;
        }
        pan_state.last_cursor = cursor;
    } else {
        pan_state.last_cursor = None;
    }
}
