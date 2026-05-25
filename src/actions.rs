use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::farmland::{farm_texture, CropState, FarmTile, MIN_READY_TIME};
use crate::player::House;

// Shop constants (mirrored from player.rs for action handling)
const SHOP_COST_FOOD: u32 = 3;
const SHOP_GAIN_ESSENTIALS: u32 = 10;

// ---------------------------------------------------------------------------
// Action event – add new variants for new action types
// ---------------------------------------------------------------------------

#[derive(Event)]
pub enum ActionEvent {
    /// Toggle a single farm tile's state.
    ///   Fallow → Growing  |  Weedy → Growing  |  Ready → Fallow (+ harvest)
    /// When `house_id` is set, harvested produce is deposited there.
    FarmInteract {
        tile_x: usize,
        tile_y: usize,
        house_id: Option<usize>,
    },
    /// A character visits the shop to trade food for daily essentials.
    ShopTrade {
        house_id: usize,
    },
    /// A character emigrates to a new settlement, taking food from the old house.
    Emigrate {
        house_id: usize,
        food_amount: u32,
    },
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ActionPlugin;

impl Plugin for ActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ActionEvent>();
        // process_action_events 已迁移到 CharacterPlugin 的 FixedUpdate 链中
    }
}

// ---------------------------------------------------------------------------
// Action processor – single place for all action effects
// ---------------------------------------------------------------------------

pub fn process_action_events(
    mut events: EventReader<ActionEvent>,
    mut farm_tiles: Query<(&mut FarmTile, &mut Sprite)>,
    mut houses: Query<&mut House>,
    assets: Res<GameAssets>,
) {
    for event in events.read() {
        match event {
            ActionEvent::FarmInteract { tile_x, tile_y, house_id } => {
                apply_farm_interact(*tile_x, *tile_y, *house_id, &mut farm_tiles, &mut houses, &assets);
            }
            ActionEvent::ShopTrade { house_id } => {
                apply_shop_trade(*house_id, &mut houses);
            }
            ActionEvent::Emigrate { house_id, food_amount } => {
                apply_emigrate(*house_id, *food_amount, &mut houses);
            }
        }
    }
}

fn apply_farm_interact(
    tile_x: usize,
    tile_y: usize,
    house_id: Option<usize>,
    farm_tiles: &mut Query<(&mut FarmTile, &mut Sprite)>,
    houses: &mut Query<&mut House>,
    assets: &GameAssets,
) {
    // Find the specific tile
    let mut current = None;
    for (ft, _) in farm_tiles.iter() {
        if ft.tile_x == tile_x && ft.tile_y == tile_y {
            current = Some((ft.state, ft.plot, ft.growth));
            break;
        }
    }
    let Some((state, plot_id, growth)) = current else { return };

    // Ready tiles need minimum display time before harvest
    // (manual C-key interaction with house_id=None bypasses this)
    if state == CropState::Ready && growth < MIN_READY_TIME && house_id.is_some() {
        return;
    }

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

    // Apply state to this single tile + swap texture
    for (mut ft, mut sprite) in farm_tiles.iter_mut() {
        if ft.tile_x != tile_x || ft.tile_y != tile_y {
            continue;
        }
        ft.state = st;
        ft.growth = 0.0;
        sprite.color = Color::WHITE;
        sprite.image = farm_texture(st, assets).clone();
        break;
    }

    info!(
        "[FARM] Plot #{} tile ({},{}): {} (by house {:?})",
        plot_id, tile_x, tile_y, action_label, house_id,
    );

    // Harvest: deposit 1 food into the character's house
    if was_ready {
        if let Some(hid) = house_id {
            for mut house in houses.iter_mut() {
                if house.id == hid {
                    house.storage += 12;
                    info!(
                        "[FARM] Tile ({},{}) HARVEST → House #{} +12 food (storage: {})",
                        tile_x, tile_y, hid, house.storage,
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

fn apply_emigrate(house_id: usize, food_amount: u32, houses: &mut Query<&mut House>) {
    for mut house in houses.iter_mut() {
        if house.id == house_id {
            let actual = food_amount.min(house.storage);
            house.storage -= actual;
            info!(
                "[EMIGRATE] House #{}: -{} food for new settlement (remaining: {})",
                house_id, actual, house.storage,
            );
            break;
        }
    }
}
