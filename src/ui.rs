use bevy::prelude::*;

use crate::birds::Bird;
use crate::caves::Cave;
use crate::clouds::Cloud;
use crate::features::Feature;
use crate::generation::{ElevationMap, MoistureMap};
use crate::lang::{tr, GameLang};
use crate::map::{Map, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::resources::Resource;
use crate::vegetation::Vegetation;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct HoveredTile(pub Option<(usize, usize)>);

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct InfoText;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredTile>();
        app.add_systems(Startup, spawn_info_panel);
        app.add_systems(Update, (update_hovered_tile, update_info_panel));
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn spawn_info_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/msyh.ttf");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                max_width: Val::Px(320.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
        ))
        .with_children(|parent| {
            parent.spawn((
                InfoText,
                Text::new("Hover over the map"),
                TextFont {
                    font: font,
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn update_hovered_tile(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut hovered: ResMut<HoveredTile>,
) {
    let Ok(window) = windows.get_single() else {
        hovered.0 = None;
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        hovered.0 = None;
        return;
    };
    let Ok((cam, cam_global)) = camera_q.get_single() else {
        hovered.0 = None;
        return;
    };
    let Ok(world_pos) = cam.viewport_to_world_2d(cam_global, cursor) else {
        hovered.0 = None;
        return;
    };

    let tile_x = (world_pos.x / TILE_SIZE).floor() as isize;
    let tile_y = (world_pos.y / TILE_SIZE).floor() as isize;

    if tile_x < 0
        || tile_y < 0
        || tile_x >= MAP_WIDTH as isize
        || tile_y >= MAP_HEIGHT as isize
    {
        hovered.0 = None;
        return;
    }

    hovered.0 = Some((tile_x as usize, tile_y as usize));
}

fn tile_from_transform(tf: &Transform) -> (usize, usize) {
    let x = (tf.translation.x / TILE_SIZE) as usize;
    let y = (tf.translation.y / TILE_SIZE) as usize;
    (x, y)
}

fn update_info_panel(
    lang: Res<GameLang>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    elevation: Option<Res<ElevationMap>>,
    moisture: Option<Res<MoistureMap>>,
    resource_q: Query<(&Resource, &Transform)>,
    vegetation_q: Query<(&Vegetation, &Transform)>,
    feature_q: Query<(&Feature, &Transform)>,
    bird_q: Query<(&Bird, &Transform)>,
    cave_q: Query<(&Cave, &Transform)>,
    cloud_q: Query<(&Cloud, &Transform)>,
    mut texts: Query<&mut Text, With<InfoText>>,
) {
    let Ok(mut text) = texts.get_single_mut() else {
        return;
    };
    let l = lang.0;

    let Some((tx, ty)) = hovered.0 else {
        text.0 = tr("Hover over the map", l).to_string();
        return;
    };

    let idx = ty * MAP_WIDTH + tx;
    let tile_xy = (tx, ty);

    let mut lines = Vec::new();
    lines.push(format!(
        "{}: ({}, {})  |  {}",
        tr("Tile", l),
        tx,
        ty,
        tr(map.tiles[idx].name(), l)
    ));
    if let (Some(elev), Some(moist)) = (elevation.as_ref(), moisture.as_ref()) {
        lines.push(format!(
            "{}: {:.3}  |  {}: {:.3}",
            tr("Elev", l),
            elev.0[idx],
            tr("Moist", l),
            moist.0[idx]
        ));
    }
    lines.push(tr("── Z Layers ──", l).to_string());

    // aerial
    for (_, tf) in cloud_q.iter() {
        if tile_from_transform(tf) == tile_xy {
            lines.push(format!("+5.0  {}  ({})", tr("Cloud", l), tr("air", l)));
        }
    }
    for (_, tf) in bird_q.iter() {
        if tile_from_transform(tf) == tile_xy {
            lines.push(format!("+3.0  {}  ({})", tr("Bird", l), tr("air", l)));
        }
    }

    // surface
    for (feat, tf) in feature_q.iter() {
        if tile_from_transform(tf) == tile_xy {
            lines.push(format!(
                "+1.2  {}  ({})",
                tr(feat.kind.name(), l),
                tr("landmark", l)
            ));
        }
    }
    for (res, tf) in resource_q.iter() {
        if tile_from_transform(tf) == tile_xy {
            lines.push(format!(
                "+1.1  {} ({})  ({})",
                tr(res.kind.name(), l),
                res.amount,
                tr("resource", l)
            ));
        }
    }
    for (veg, tf) in vegetation_q.iter() {
        if tile_from_transform(tf) == tile_xy {
            lines.push(format!(
                "+1.0  {}  ({})",
                tr(veg.kind.name(), l),
                tr("vegetation", l)
            ));
        }
    }

    // underground
    for (_, tf) in cave_q.iter() {
        if tile_from_transform(tf) == tile_xy {
            lines.push(format!("-1.0  {}  ({})", tr("Cave", l), tr("underground", l)));
        }
    }

    text.0 = lines.join("\n");
}
