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
mod player;
mod resources;
mod sim_rng;
mod sim_time;
mod ui;
mod vegetation;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
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
        .add_plugins(player::CharacterPlugin)
        .add_plugins(vegetation::VegetationPlugin)
        .add_plugins(ui::UIPlugin)
        .run();
}
