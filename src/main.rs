mod actions;
mod assets;
mod birds;
mod buildings;
mod camera;
mod caves;
mod clouds;
mod element_config;
mod farmland;
mod features;
mod generation;
mod lang;
mod map;
mod pathfinding;
mod player;
mod resources;
mod sim_rng;
mod sim_time;
mod ui;
mod vegetation;

use bevy::prelude::*;

fn main() {
    // Point asset path to the project root's assets/ directory
    let asset_path: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "assets"].iter().collect();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_path.to_string_lossy().to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Sim World".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(sim_rng::SimRngPlugin)
        .add_plugins(actions::ActionPlugin)
        .add_plugins(generation::GenerationPlugin)
        .add_plugins(lang::LangPlugin)
        .add_plugins(sim_time::SimTimePlugin)
        .add_plugins(assets::AssetPlugin)
        .add_plugins(map::MapPlugin)
        .add_plugins(camera::CameraPlugin)
        .add_plugins(farmland::FarmlandPlugin)
        .add_plugins(pathfinding::PathfindingPlugin)
        .add_plugins(player::CharacterPlugin)
        .add_plugins(vegetation::VegetationPlugin)
        .add_plugins(birds::BirdPlugin)
        .add_plugins(clouds::CloudPlugin)
        .add_plugins(caves::CavePlugin)
        .add_plugins(resources::ResourcePlugin)
        .add_plugins(features::FeaturePlugin)
        .add_plugins(buildings::BuildingPlugin)
        .add_plugins(ui::UIPlugin)
        .run();
}
