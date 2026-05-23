use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::farmland::{farm_texture, CropState, FarmTile};
use crate::player::House;

// Shop constants (mirrored from player.rs for action handling)
const SHOP_COST_FOOD: u32 = 3;
const SHOP_GAIN_ESSENTIALS: u32 = 10;

// ---------------------------------------------------------------------------
// Action event – add new variants for new action types
// ---------------------------------------------------------------------------

#[derive(Event)]
pub enum ActionEvent {
    /// Toggle a farm plot's state.
    ///   Fallow → Growing  |  Weedy → Growing  |  Ready → Fallow (+ harvest)
    /// When `house_id` is set, harvested produce is deposited there.
    FarmInteract {
        plot_id: usize,
        house_id: Option<usize>,
    },
    /// A character visits the shop to trade food for daily essentials.
    ShopTrade {
        house_id: usize,
    },
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ActionPlugin;

impl Plugin for ActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ActionEvent>();
        app.add_systems(Update, process_action_events);
    }
}

// ---------------------------------------------------------------------------
// Action processor – single place for all action effects
// ---------------------------------------------------------------------------

fn process_action_events(
    mut events: EventReader<ActionEvent>,
    mut farm_tiles: Query<(&mut FarmTile, &mut Sprite)>,
    mut houses: Query<&mut House>,
    assets: Res<GameAssets>,
) {
    for event in events.read() {
        match event {
            ActionEvent::FarmInteract { plot_id, house_id } => {
                apply_farm_interact(*plot_id, *house_id, &mut farm_tiles, &mut houses, &assets);
            }
            ActionEvent::ShopTrade { house_id } => {
                apply_shop_trade(*house_id, &mut houses);
            }
        }
    }
}

fn apply_farm_interact(
    plot_id: usize,
    house_id: Option<usize>,
    farm_tiles: &mut Query<(&mut FarmTile, &mut Sprite)>,
    houses: &mut Query<&mut House>,
    assets: &GameAssets,
) {
    // Read current state of the plot
    let mut current = None;
    for (ft, _) in farm_tiles.iter() {
        if ft.plot == plot_id {
            current = Some(ft.state);
            break;
        }
    }
    let Some(state) = current else { return };

    let was_ready = state == CropState::Ready;
    let new_state = match state {
        CropState::Fallow => Some(CropState::Growing),
        CropState::Weedy => Some(CropState::Growing),
        CropState::Ready => Some(CropState::Fallow),
        _ => None,
    };

    let Some(st) = new_state else { return };

    let action_label = match (state, st) {
        (CropState::Fallow, CropState::Growing) => "PLANT",
        (CropState::Weedy, CropState::Growing) => "WEED",
        (CropState::Ready, CropState::Fallow) => "HARVEST",
        _ => "TOGGLE",
    };

    // Apply state to every tile in the plot + swap texture
    let new_tex = farm_texture(st, assets).clone();
    for (mut ft, mut sprite) in farm_tiles.iter_mut() {
        if ft.plot != plot_id {
            continue;
        }
        ft.state = st;
        ft.growth = 0.0;
        sprite.color = Color::WHITE;
        sprite.image = new_tex.clone();
    }

    info!(
        "[FARM] Plot #{}: {} (by house {:?})",
        plot_id, action_label, house_id,
    );

    // Harvest: deposit produce into the character's house
    if was_ready {
        if let Some(hid) = house_id {
            let tile_count = farm_tiles
                .iter()
                .filter(|(ft, _)| ft.plot == plot_id)
                .count() as u32;

            for mut house in houses.iter_mut() {
                if house.id == hid {
                    house.storage += tile_count * 2;
                    info!(
                        "[FARM] Plot #{} HARVEST → House #{} +{} food (storage: {})",
                        plot_id, hid, tile_count, house.storage,
                    );
                    break;
                }
            }
        }
    }
}

fn apply_shop_trade(house_id: usize, houses: &mut Query<&mut House>) {
    for mut house in houses.iter_mut() {
        if house.id == house_id {
            if house.storage >= SHOP_COST_FOOD {
                house.storage -= SHOP_COST_FOOD;
                house.essentials += SHOP_GAIN_ESSENTIALS;
                info!(
                    "[SHOP] House #{} bought essentials (food: {}, essentials: {})",
                    house.id, house.storage, house.essentials,
                );
            } else {
                info!(
                    "[SHOP] House #{} too poor (food: {})",
                    house.id, house.storage,
                );
            }
            break;
        }
    }
}
