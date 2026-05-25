use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::assets::GameAssets;
use crate::element_config::{Interaction, VegSpawnRule, VEGETATION_CONFIGS, VEG_SPAWN_RULES};
use crate::map::{Map, TileCategory, TileContent, TileEntry, TileType, TILE_SIZE, MAP_WIDTH, MAP_HEIGHT};
use crate::sim_rng::SimRng;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Per-tile boolean mask: true if at least one tree (DeciduousTree, PineTree,
/// or PalmTree) occupies this tile.  Used by character AI for exploration and
/// settlement founding.  Built once at startup and kept in sync if trees are
/// ever chopped down.
#[derive(Resource, Default)]
pub struct TreeMask(pub Vec<bool>);

// ---------------------------------------------------------------------------
// Vegetation kinds
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum VegetationKind {
    DeciduousTree,
    PineTree,
    PalmTree,
    Bush,
    Flower,
    DeadBush,
    Cactus,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A vegetation entity (tree, bush, flower) on the map.
/// Fields are baked from element_config at spawn time.
#[derive(Component)]
pub struct Vegetation {
    pub kind: VegetationKind,
    pub name: &'static str,
    pub interaction: Interaction,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_vegetation);
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

/// Map vegetation kinds to texture handles.
fn veg_texture<'a>(kind: VegetationKind, assets: &'a GameAssets) -> &'a Handle<Image> {
    match kind {
        VegetationKind::DeciduousTree => &assets.veg_deciduous_tree,
        VegetationKind::PineTree => &assets.veg_pine_tree,
        VegetationKind::PalmTree => &assets.veg_palm_tree,
        VegetationKind::Bush => &assets.veg_bush,
        VegetationKind::Flower => &assets.veg_flower,
        VegetationKind::DeadBush => &assets.veg_dead_bush,
        VegetationKind::Cactus => &assets.veg_cactus,
    }
}

fn spawn_vegetation(
    mut commands: Commands,
    map: Res<Map>,
    mut rng: ResMut<SimRng>,
    mut tile_content: ResMut<TileContent>,
    assets: Res<GameAssets>,
) {
    let mut tree_mask = vec![false; MAP_WIDTH * MAP_HEIGHT];

    // Pre-group rules by terrain type: for each TileType, list of applicable rules
    let mut terrain_rules: [Vec<&VegSpawnRule>; TileType::VARIANT_COUNT] =
        std::array::from_fn(|_| Vec::new());
    for rule in VEG_SPAWN_RULES {
        for &tt in rule.terrain {
            terrain_rules[tt as u8 as usize].push(rule);
        }
    }

    // Single tile pass: check all applicable rules per tile
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let tile_type = map.tiles[y * MAP_WIDTH + x] as u8 as usize;
            for rule in &terrain_rules[tile_type] {
                if rng.gen_f64() >= rule.chance {
                    continue;
                }

                let cfg = &VEGETATION_CONFIGS[rule.kind as u8 as usize];

                let ox = (rng.gen_f64() - 0.5) * 6.0_f64;
                let oy = (rng.gen_f64() - 0.5) * 6.0_f64;
                let world_x = x as f32 * TILE_SIZE + TILE_SIZE / 2.0 + ox as f32;
                let world_y = y as f32 * TILE_SIZE + TILE_SIZE / 2.0 + oy as f32;

                let tex = veg_texture(rule.kind, &assets).clone();
                commands.spawn((
                    Vegetation {
                        kind: rule.kind,
                        name: cfg.name_en,
                        interaction: cfg.interaction,
                    },
                    Sprite {
                        image: tex,
                        custom_size: Some(Vec2::new(cfg.size, cfg.size)),
                        anchor: Anchor::Center,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, 1.0),
                    GlobalTransform::default(),
                    Visibility::default(),
                ));

                tile_content.data.entry(y * MAP_WIDTH + x).or_default().push(TileEntry {
                    name: cfg.name_en,
                    category: TileCategory::Vegetation,
                    amount: 0,
                    w: 1,
                    h: 1,
                });

                if matches!(rule.kind,
                    VegetationKind::DeciduousTree | VegetationKind::PineTree | VegetationKind::PalmTree
                ) {
                    tree_mask[y * MAP_WIDTH + x] = true;
                }
            }
        }
    }

    commands.insert_resource(TreeMask(tree_mask));
}
