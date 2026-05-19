mod birds;
mod camera;
mod caves;
mod clouds;
mod features;
mod generation;
mod lang;
mod map;
mod resources;
mod sim_rng;
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
        .add_plugins(generation::GenerationPlugin)
        .add_plugins(lang::LangPlugin)
        .add_plugins(map::MapPlugin)
        .add_plugins(resources::ResourcePlugin)
        .add_plugins(vegetation::VegetationPlugin)
        .add_plugins(features::FeaturePlugin)
        .add_plugins(camera::CameraPlugin)
        .add_plugins(clouds::CloudPlugin)
        .add_plugins(birds::BirdPlugin)
        .add_plugins(caves::CavePlugin)
        .add_plugins(ui::UIPlugin)
        .run();
}
