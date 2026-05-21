use bevy::prelude::*;

use crate::element_config::Interaction;
use crate::farmland::{CropState, FarmTile};
use crate::lang::{tr, GameLang};
use crate::map::{Map, TerrainData, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::player::{Character, GraveInfo, House, LifeStage};
use crate::sim_time::{SimTime, TimeScale};

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

fn update_info_panel(
    lang: Res<GameLang>,
    hovered: Res<HoveredTile>,
    map: Res<Map>,
    terrain_data: Res<TerrainData>,
    sim: Res<SimTime>,
    scale: Res<TimeScale>,
    house_q: Query<(&House, &Transform)>,
    char_q: Query<(&Character, &Transform)>,
    farm_q: Query<&FarmTile>,
    grave_q: Query<(&GraveInfo, &Transform)>,
    mut texts: Query<&mut Text, With<InfoText>>,
) {
    let Ok(mut text) = texts.get_single_mut() else {
        return;
    };
    let l = lang.0;

    // Time display
    let total_secs = sim.elapsed as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    let speed_label = if scale.speed == 0.0 {
        tr("PAUSED", l)
    } else {
        "×"
    };
    let mut lines = Vec::new();
    if scale.speed == 0.0 {
        lines.push(format!(
            "{} {:02}:{:02}:{:02}  [{}]",
            tr("Time", l), hours, mins, secs, speed_label
        ));
    } else {
        lines.push(format!(
            "{} {:02}:{:02}:{:02}  {}{:.1}",
            tr("Time", l), hours, mins, secs, speed_label, scale.speed
        ));
    }

    let Some((tx, ty)) = hovered.0 else {
        lines.push(tr("Hover over the map", l).to_string());
        text.0 = lines.join("\n");
        return;
    };

    let idx = ty * MAP_WIDTH + tx;

    let tile_type = map.tiles[idx];
    let inter = terrain_data.interactions[tile_type as u8 as usize];
    lines.push(format!(
        "{}: ({}, {})  |  {}",
        tr("Tile", l),
        tx,
        ty,
        tr(terrain_data.names[tile_type as u8 as usize], l)
    ));
    if inter != Interaction::None {
        lines.push(format!(
            "  [{}]",
            match l {
                crate::lang::Lang::Zh => inter.tag_zh(),
                _ => inter.tag_en(),
            }
        ));
    }
    lines.push(tr("── Z Layers ──", l).to_string());

    for (ch, tf) in char_q.iter() {
        let px = (tf.translation.x / TILE_SIZE).floor() as isize;
        let py = (tf.translation.y / TILE_SIZE).floor() as isize;
        if px >= 0 && py >= 0 && px as usize == tx && py as usize == ty {
            let role = match ch.stage {
                LifeStage::Child => tr("Child", l),
                LifeStage::Adult => tr("Adult", l),
            };
            lines.push(format!("+2.0  {} ({})", role, tr("character", l)));
        }
    }

    for (house, _) in house_q.iter() {
        if tx >= house.tile_x && tx < house.tile_x + house.w
            && ty >= house.tile_y && ty < house.tile_y + house.h
        {
            let adults = char_q.iter().filter(|(c, _)| c.house_id == house.id && c.stage == LifeStage::Adult).count();
            let children = char_q.iter().filter(|(c, _)| c.house_id == house.id && c.stage == LifeStage::Child).count();
            lines.push(format!("+1.3  {} ({}×{})  ({})",
                tr("House", l), house.w, house.h, tr("building", l)));
            lines.push(format!("  {}: {}  {}: {}  {}: {}",
                tr("Storage", l), house.storage,
                tr("Adults", l), adults,
                tr("Children", l), children));
        }
    }

    for farm in farm_q.iter() {
        if farm.tile_x == tx && farm.tile_y == ty {
            let st = match farm.state {
                CropState::Fallow => tr("Fallow", l),
                CropState::Growing => tr("Growing", l),
                CropState::Weedy => tr("Weedy", l),
                CropState::Ready => tr("Ready", l),
                CropState::Clearing => tr("Clearing", l),
            };
            lines.push(format!("+1.0  {} {}  ({})", tr("Farmland", l), st, tr("farm", l)));
            lines.push(format!("  [{}] {}", tr("Action", l), tr("Press C to interact", l)));
        }
    }

    for (info, tf) in grave_q.iter() {
        let gx = (tf.translation.x / TILE_SIZE).floor() as isize;
        let gy = (tf.translation.y / TILE_SIZE).floor() as isize;
        if gx >= 0 && gy >= 0 && gx as usize == tx && gy as usize == ty {
            lines.push(format!("+1.0  {}", tr("Tombstone", l)));
            lines.push(format!("  {}: #{}  {}: {:.0}", tr("House", l), info.house_id, tr("Age", l), info.age));
            lines.push(format!("  {}: {}", tr("Cause", l), tr(info.cause, l)));
        }
    }

    text.0 = lines.join("\n");
}
