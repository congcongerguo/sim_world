use bevy::prelude::*;

use crate::element_config::Interaction;
use crate::farmland::{CropState, FarmTile};
use crate::generation::ElevationMap;
use crate::lang::{tr, GameLang};
use crate::map::{Map, TerrainData, TileCategory, TileContent, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::player::{Character, DeathEvents, Gender, GraveInfo, House, LifeStage, MaritalStatus, ShopLocation, StateHistory};
use crate::sim_time::YEAR;
use crate::sim_time::{SimTime, TimeScale};

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct HoveredTile(pub Option<(usize, usize)>);

/// A tile that has been clicked and "locked" for persistent display.
#[derive(Resource, Default)]
pub struct Selection(pub Option<(usize, usize)>);

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct InfoText;

#[derive(Component)]
struct HouseholdSummaryText;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredTile>();
        app.init_resource::<Selection>();
        app.add_systems(Startup, spawn_info_panel);
        app.add_systems(Update, update_hovered_tile);
        app.add_systems(Update, update_info_panel);
        app.add_systems(Update, update_household_summary);
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
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    // Right-side household summary panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                max_width: Val::Px(340.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
        ))
        .with_children(|parent| {
            parent.spawn((
                HouseholdSummaryText,
                Text::new(""),
                TextFont {
                    font: font,
                    font_size: 13.0,
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
    mut selection: ResMut<Selection>,
    mouse: Res<ButtonInput<MouseButton>>,
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

    // Left click: toggle selection on this tile
    if mouse.just_pressed(MouseButton::Left) {
        let pos = (tile_x as usize, tile_y as usize);
        if selection.0 == Some(pos) {
            selection.0 = None; // deselect on second click
        } else {
            selection.0 = Some(pos);
        }
    }

    // Right click: clear selection
    if mouse.just_pressed(MouseButton::Right) {
        selection.0 = None;
    }
}

fn update_info_panel(
    lang: Res<GameLang>,
    (hovered, selection): (Res<HoveredTile>, Res<Selection>),
    map: Res<Map>,
    elevation: Option<Res<ElevationMap>>,
    terrain_data: Res<TerrainData>,
    tile_content: Res<TileContent>,
    (sim, scale): (Res<SimTime>, Res<TimeScale>),
    shop_location: Res<ShopLocation>,
    death_events: Res<DeathEvents>,
    state_history: Res<StateHistory>,
    house_q: Query<(&House, &Transform)>,
    char_q: Query<(Entity, &Character, &Transform)>,
    farm_q: Query<&FarmTile>,
    grave_q: Query<(&GraveInfo, &Transform)>,
    mut texts: Query<&mut Text, With<InfoText>>,
) {
    let Ok(mut text) = texts.get_single_mut() else {
        return;
    };
    let l = lang.0;

    // Time display as calendar date
    let (y, m, d) = sim.date();
    let speed_label = if scale.speed == 0.0 {
        tr("PAUSED", l)
    } else {
        "×"
    };
    let mut lines = Vec::new();
    if scale.speed == 0.0 {
        lines.push(format!(
            "{} {:04}-{:02}-{:02}  [{}]",
            tr("Time", l), y, m + 1, d + 1, speed_label
        ));
    } else {
        lines.push(format!(
            "{} {:04}-{:02}-{:02}  {}{:.1}",
            tr("Time", l), y, m + 1, d + 1, speed_label, scale.speed
        ));
    }

    // Use selected tile (locked) if set, otherwise hovered
    let display_tile = selection.0.or(hovered.0);
    let Some((tx, ty)) = display_tile else {
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
    // Lock indicator when selection is active
    if selection.0.is_some() {
        lines.push(format!("  [{}]", tr("Locked", l)));
    }
    // Elevation display
    if let Some(ref elev) = elevation {
        let e = elev.0[idx];
        let contour_band = (e * 10.0).floor() as u32;
        lines.push(format!(
            "  Z: {:.0}m  ({} {})",
            e * 100.0,
            tr("Band", l),
            contour_band,
        ));
    }
    if inter != Interaction::None {
        lines.push(format!(
            "  [{}]",
            match l {
                crate::lang::Lang::Zh => inter.tag_zh(),
                _ => inter.tag_en(),
            }
        ));
    }

    // Tile overlay content (resources, vegetation, features, buildings)
    if let Some(entries) = tile_content.data.get(&idx) {
        for entry in entries {
            let cat_zh = match entry.category {
                TileCategory::Resource => tr("Resource", l),
                TileCategory::Vegetation => tr("Vegetation", l),
                TileCategory::Feature => tr("Feature", l),
                TileCategory::Building => tr("Building", l),
                TileCategory::Cave => tr("Cave", l),
            };
            let name = tr(entry.name, l);
            if entry.amount > 0 {
                lines.push(format!("  {}  |  {}  |  {}: {}", cat_zh, name, tr("Amount", l), entry.amount));
            } else {
                lines.push(format!("  {}  |  {}", cat_zh, name));
            }
        }
    }

    lines.push(tr("── Z Layers ──", l).to_string());

    // Check if a death happened at this tile this frame (before tombstone exists)
    let death_this_frame = death_events.tiles.contains(&(tx, ty));

    for (entity, ch, tf) in char_q.iter() {
        let px = (tf.translation.x / TILE_SIZE).floor() as isize;
        let py = (tf.translation.y / TILE_SIZE).floor() as isize;
        if px >= 0 && py >= 0 && px as usize == tx && py as usize == ty {
            // Skip characters that died this frame (ghost prevention)
            if death_this_frame {
                continue;
            }
            let role = match ch.stage {
                LifeStage::Child => tr("Child", l),
                LifeStage::Adult => tr("Adult", l),
            };
            let gender_str = match ch.gender {
                Gender::Male => tr("Male", l),
                Gender::Female => tr("Female", l),
            };
            let marital_str = match ch.marital {
                MaritalStatus::Single => tr("Single", l),
                MaritalStatus::Married => tr("Married", l),
                MaritalStatus::Widowed => tr("Widowed", l),
            };
            lines.push(format!("+2.0  {} {} ({})  {}:{}  [{}]",
                gender_str, role, tr("character", l), tr("Food", l), ch.food, marital_str));
            // Show top 2 personality traits
            let p = &ch.personality;
            let mut traits = vec![
                (tr("Openness", l), p.openness),
                (tr("Conscientiousness", l), p.conscientiousness),
                (tr("Extraversion", l), p.extraversion),
                (tr("Agreeableness", l), p.agreeableness),
                (tr("Neuroticism", l), p.neuroticism),
            ];
            traits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let trait_strs: Vec<String> = traits.iter().map(|(name, val)| {
                format!("{}: {:.0}%", name, val * 100.0)
            }).collect();
            lines.push(format!("  {}", trait_strs.join("  |  ")));

            // State history (last 10 state transitions)
            if let Some(history) = state_history.entries.get(&entity) {
                if !history.is_empty() {
                    lines.push(format!("  ── {} ──", "State History"));
                    for (i, (_t, desc)) in history.iter().rev().enumerate() {
                        let marker = if i == 0 { "→ " } else { "  " };
                        lines.push(format!("  {}{}", marker, desc));
                    }
                }
            }
        }
    }

    for (house, _) in house_q.iter() {
        if tx >= house.tile_x && tx < house.tile_x + house.w
            && ty >= house.tile_y && ty < house.tile_y + house.h
        {
            let adults = char_q.iter().filter(|(_, c, _)| c.house_id == house.id && c.stage == LifeStage::Adult).count();
            let children = char_q.iter().filter(|(_, c, _)| c.house_id == house.id && c.stage == LifeStage::Child).count();
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

    // Shop interaction hint
    if tx >= shop_location.tile_x && tx < shop_location.tile_x + 2
        && ty >= shop_location.tile_y && ty < shop_location.tile_y + 2
    {
        lines.push(format!("+0.3  {}", tr("Village Shop", l)));
        lines.push(format!("  [{}] {}", tr("Action", l), tr("Press C to trade", l)));
    }

    for (info, tf) in grave_q.iter() {
        let gx = (tf.translation.x / TILE_SIZE).floor() as isize;
        let gy = (tf.translation.y / TILE_SIZE).floor() as isize;
        if gx >= 0 && gy >= 0 && gx as usize == tx && gy as usize == ty {
            lines.push(format!("+1.0  {}", tr("Tombstone", l)));
            let gstr = match info.gender {
                Gender::Male => tr("Male", l),
                Gender::Female => tr("Female", l),
            };
            lines.push(format!("  {} {}  {}: #{}  {}: {:.0}{}", gstr, tr("Tombstone", l), tr("House", l), info.house_id, tr("Age", l), info.age / YEAR, tr("years", l)));
            lines.push(format!("  {}: {}", tr("Cause", l), tr(info.cause, l)));
        }
    }

    text.0 = lines.join("\n");
}

/// Right-side panel showing all households' status at a glance.
fn update_household_summary(
    lang: Res<GameLang>,
    house_q: Query<&House>,
    char_q: Query<&Character>,
    farm_q: Query<&FarmTile>,
    sim: Res<SimTime>,
    scale: Res<TimeScale>,
    mut texts: Query<&mut Text, With<HouseholdSummaryText>>,
) {
    let Ok(mut text) = texts.get_single_mut() else { return };
    let l = lang.0;

    let (y, m, d) = sim.date();
    let speed_label = if scale.speed == 0.0 {
        tr("PAUSED", l)
    } else {
        "×"
    };
    let mut lines = Vec::new();
    if scale.speed == 0.0 {
        lines.push(format!("{}  {:04}-{:02}-{:02}  [{}]",
            tr("Households", l), y, m + 1, d + 1, speed_label));
    } else {
        lines.push(format!("{}  {:04}-{:02}-{:02}  {}{:.1}",
            tr("Households", l), y, m + 1, d + 1, speed_label, scale.speed));
    }
    lines.push(format!("──{}──", tr("─", l)));

    // Collect all houses sorted by ID
    let mut houses: Vec<&House> = house_q.iter().collect();
    houses.sort_by_key(|h| h.id);

    for house in houses {
        let adults = char_q.iter().filter(|c| c.house_id == house.id && c.stage == LifeStage::Adult).count();
        let children = char_q.iter().filter(|c| c.house_id == house.id && c.stage == LifeStage::Child).count();

        let total_farm = farm_q.iter().filter(|ft| ft.plot == house.id).count();
        let ready = farm_q.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Ready).count();
        let growing = farm_q.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Growing).count();
        let fallow = farm_q.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Fallow).count();
        let weedy = farm_q.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Weedy).count();
        let clearing = farm_q.iter().filter(|ft| ft.plot == house.id && ft.state == CropState::Clearing).count();

        lines.push(format!(
            "{} #{}  {}:{}  {}:{}  {}:{}  {}:{}",
            tr("House", l), house.id,
            tr("Food", l), house.storage,
            tr("Ess", l), house.essentials,
            tr("Adults", l), adults,
            tr("Children", l), children,
        ));
        lines.push(format!(
            "  {}: {}  {}:R{} G{} F{} W{} C{}",
            tr("Farm", l), total_farm,
            tr("Tiles", l), ready, growing, fallow, weedy, clearing,
        ));
    }

    text.0 = lines.join("\n");
}
