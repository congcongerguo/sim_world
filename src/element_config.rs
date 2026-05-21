// ---------------------------------------------------------------------------
// Element Configuration
// ---------------------------------------------------------------------------
// Central config tables for all map elements (terrain, resources, vegetation,
// features). Each entry defines the element's names, color, interaction
// behaviour, and optional spawn parameters.
//
// To add a new element:
//   1. Add a new variant to the relevant enum (TileType, ResourceKind, etc.)
//   2. Append a config entry to the corresponding config table below.
//   3. If the element should spawn naturally, add spawn parameters.
// ---------------------------------------------------------------------------

// Fields like name_zh, interaction on overlay elements, tile_type on the
// struct itself are for documentation / forward-compatibility.
#![allow(dead_code)]

use bevy::prelude::*;

use crate::buildings::BuildingKind;
use crate::features::FeatureKind;
use crate::map::TileType;
use crate::resources::ResourceKind;
use crate::vegetation::VegetationKind;

// ---------------------------------------------------------------------------
// Interaction – defines how a player can interact with a map element
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interaction {
    /// No special interaction.
    None,
    /// Walkable surface.
    Walkable,
    /// Blocks movement (e.g., deep water).
    Blocked,
    /// Damages on contact (e.g., lava, cactus).
    Damaging,
    /// Can be harvested (e.g., flowers, bushes).
    Harvestable,
    /// Can be mined (e.g., ore deposits).
    Mineable,
    /// Can be chopped (e.g., trees).
    Chopable,
    /// Fresh water source.
    WaterSource,
    /// Heals the player.
    Healing,
    /// Can be explored (e.g., ruins, fossils).
    Explorable,
}

impl Interaction {
    pub fn tag_en(&self) -> &'static str {
        match self {
            Interaction::None => "",
            Interaction::Walkable => "walkable",
            Interaction::Blocked => "blocked",
            Interaction::Damaging => "damaging",
            Interaction::Harvestable => "harvestable",
            Interaction::Mineable => "mineable",
            Interaction::Chopable => "chopable",
            Interaction::WaterSource => "water source",
            Interaction::Healing => "healing",
            Interaction::Explorable => "explorable",
        }
    }

    pub fn tag_zh(&self) -> &'static str {
        match self {
            Interaction::None => "",
            Interaction::Walkable => "可行走",
            Interaction::Blocked => "阻挡",
            Interaction::Damaging => "伤害",
            Interaction::Harvestable => "可采集",
            Interaction::Mineable => "可开采",
            Interaction::Chopable => "可砍伐",
            Interaction::WaterSource => "水源",
            Interaction::Healing => "治疗",
            Interaction::Explorable => "可探索",
        }
    }
}

// ---------------------------------------------------------------------------
// Terrain config
// ---------------------------------------------------------------------------

pub struct TerrainConfig {
    pub tile_type: TileType,
    pub name_en: &'static str,
    pub name_zh: &'static str,
    pub color: Color,
    pub interaction: Interaction,
}

/// All terrain type configurations, indexed by TileType repr(u8) value.
pub const TERRAIN_CONFIGS: [TerrainConfig; 15] = [
    TerrainConfig {
        tile_type: TileType::Grass,
        name_en: "Grass",
        name_zh: "草地",
        color: Color::srgb(0.30, 0.70, 0.20),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Water,
        name_en: "Water",
        name_zh: "浅水",
        color: Color::srgb(0.20, 0.35, 0.80),
        interaction: Interaction::WaterSource,
    },
    TerrainConfig {
        tile_type: TileType::DeepWater,
        name_en: "Deep Water",
        name_zh: "深海",
        color: Color::srgb(0.08, 0.15, 0.50),
        interaction: Interaction::Blocked,
    },
    TerrainConfig {
        tile_type: TileType::Sand,
        name_en: "Sand",
        name_zh: "沙地",
        color: Color::srgb(0.76, 0.70, 0.50),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Forest,
        name_en: "Forest",
        name_zh: "森林",
        color: Color::srgb(0.10, 0.50, 0.10),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Swamp,
        name_en: "Swamp",
        name_zh: "沼泽",
        color: Color::srgb(0.25, 0.45, 0.20),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Stone,
        name_en: "Stone",
        name_zh: "岩石",
        color: Color::srgb(0.50, 0.50, 0.50),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Dirt,
        name_en: "Dirt",
        name_zh: "泥土",
        color: Color::srgb(0.55, 0.40, 0.25),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Snow,
        name_en: "Snow",
        name_zh: "雪地",
        color: Color::srgb(0.95, 0.95, 0.95),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Lava,
        name_en: "Lava",
        name_zh: "熔岩",
        color: Color::srgb(0.80, 0.20, 0.05),
        interaction: Interaction::Damaging,
    },
    TerrainConfig {
        tile_type: TileType::Tundra,
        name_en: "Tundra",
        name_zh: "冻原",
        color: Color::srgb(0.60, 0.65, 0.55),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Ice,
        name_en: "Ice",
        name_zh: "冰原",
        color: Color::srgb(0.85, 0.90, 0.95),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Meadow,
        name_en: "Meadow",
        name_zh: "草甸",
        color: Color::srgb(0.50, 0.80, 0.25),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Desert,
        name_en: "Desert",
        name_zh: "沙漠",
        color: Color::srgb(0.85, 0.75, 0.40),
        interaction: Interaction::Walkable,
    },
    TerrainConfig {
        tile_type: TileType::Clay,
        name_en: "Clay",
        name_zh: "黏土",
        color: Color::srgb(0.65, 0.45, 0.30),
        interaction: Interaction::Walkable,
    },
];

// ---------------------------------------------------------------------------
// Resource config
// ---------------------------------------------------------------------------

pub struct ResourceSpawnRule {
    pub terrain: &'static [TileType],
    pub chance: f64,
    pub min_amount: u32,
    pub max_amount: u32,
}

pub struct ResourceConfig {
    pub kind: ResourceKind,
    pub name_en: &'static str,
    pub name_zh: &'static str,
    pub color: Color,
    pub interaction: Interaction,
    pub overlay_size: f32,
    pub spawn: ResourceSpawnRule,
}

/// All resource configs, one per ResourceKind variant.
pub const RESOURCE_CONFIGS: [ResourceConfig; 7] = [
    ResourceConfig {
        kind: ResourceKind::IronOre,
        name_en: "Iron Ore",
        name_zh: "铁矿",
        color: Color::srgb(0.50, 0.40, 0.35),
        interaction: Interaction::Mineable,
        overlay_size: 10.0,
        spawn: ResourceSpawnRule {
            terrain: &[TileType::Stone, TileType::Dirt],
            chance: 0.06,
            min_amount: 30,
            max_amount: 120,
        },
    },
    ResourceConfig {
        kind: ResourceKind::CoalOre,
        name_en: "Coal",
        name_zh: "煤矿",
        color: Color::srgb(0.15, 0.15, 0.15),
        interaction: Interaction::Mineable,
        overlay_size: 10.0,
        spawn: ResourceSpawnRule {
            terrain: &[TileType::Stone, TileType::Dirt],
            chance: 0.08,
            min_amount: 40,
            max_amount: 160,
        },
    },
    ResourceConfig {
        kind: ResourceKind::CopperOre,
        name_en: "Copper Ore",
        name_zh: "铜矿",
        color: Color::srgb(0.80, 0.50, 0.20),
        interaction: Interaction::Mineable,
        overlay_size: 10.0,
        spawn: ResourceSpawnRule {
            terrain: &[TileType::Stone],
            chance: 0.04,
            min_amount: 20,
            max_amount: 80,
        },
    },
    ResourceConfig {
        kind: ResourceKind::GoldOre,
        name_en: "Gold Ore",
        name_zh: "金矿",
        color: Color::srgb(0.90, 0.80, 0.15),
        interaction: Interaction::Mineable,
        overlay_size: 8.0,
        spawn: ResourceSpawnRule {
            terrain: &[TileType::Stone],
            chance: 0.012,
            min_amount: 10,
            max_amount: 40,
        },
    },
    ResourceConfig {
        kind: ResourceKind::ClayDeposit,
        name_en: "Clay",
        name_zh: "黏土矿",
        color: Color::srgb(0.65, 0.45, 0.30),
        interaction: Interaction::Mineable,
        overlay_size: 10.0,
        spawn: ResourceSpawnRule {
            terrain: &[TileType::Clay],
            chance: 0.25,
            min_amount: 20,
            max_amount: 80,
        },
    },
    ResourceConfig {
        kind: ResourceKind::SandDeposit,
        name_en: "Sand",
        name_zh: "沙矿",
        color: Color::srgb(0.82, 0.75, 0.55),
        interaction: Interaction::Mineable,
        overlay_size: 10.0,
        spawn: ResourceSpawnRule {
            terrain: &[TileType::Sand, TileType::Desert],
            chance: 0.15,
            min_amount: 30,
            max_amount: 100,
        },
    },
    ResourceConfig {
        kind: ResourceKind::StoneDeposit,
        name_en: "Stone",
        name_zh: "石矿",
        color: Color::srgb(0.45, 0.42, 0.40),
        interaction: Interaction::Mineable,
        overlay_size: 12.0,
        spawn: ResourceSpawnRule {
            terrain: &[TileType::Stone, TileType::Dirt, TileType::Tundra],
            chance: 0.12,
            min_amount: 50,
            max_amount: 200,
        },
    },
];

// ---------------------------------------------------------------------------
// Vegetation config
// ---------------------------------------------------------------------------

pub struct VegetationConfig {
    pub kind: VegetationKind,
    pub name_en: &'static str,
    pub name_zh: &'static str,
    pub color: Color,
    pub interaction: Interaction,
    pub size: f32,
}

/// All vegetation configs, one per VegetationKind variant.
pub const VEGETATION_CONFIGS: [VegetationConfig; 7] = [
    VegetationConfig {
        kind: VegetationKind::DeciduousTree,
        name_en: "Deciduous Tree",
        name_zh: "落叶树",
        color: Color::srgb(0.15, 0.55, 0.12),
        interaction: Interaction::Chopable,
        size: 14.0,
    },
    VegetationConfig {
        kind: VegetationKind::PineTree,
        name_en: "Pine Tree",
        name_zh: "松树",
        color: Color::srgb(0.08, 0.40, 0.08),
        interaction: Interaction::Chopable,
        size: 12.0,
    },
    VegetationConfig {
        kind: VegetationKind::PalmTree,
        name_en: "Palm Tree",
        name_zh: "棕榈树",
        color: Color::srgb(0.25, 0.60, 0.15),
        interaction: Interaction::Chopable,
        size: 14.0,
    },
    VegetationConfig {
        kind: VegetationKind::Bush,
        name_en: "Bush",
        name_zh: "灌木",
        color: Color::srgb(0.30, 0.55, 0.18),
        interaction: Interaction::Harvestable,
        size: 8.0,
    },
    VegetationConfig {
        kind: VegetationKind::Flower,
        name_en: "Flower",
        name_zh: "花",
        color: Color::srgb(0.90, 0.30, 0.50),
        interaction: Interaction::Harvestable,
        size: 5.0,
    },
    VegetationConfig {
        kind: VegetationKind::DeadBush,
        name_en: "Dead Bush",
        name_zh: "枯草",
        color: Color::srgb(0.45, 0.35, 0.20),
        interaction: Interaction::None,
        size: 7.0,
    },
    VegetationConfig {
        kind: VegetationKind::Cactus,
        name_en: "Cactus",
        name_zh: "仙人掌",
        color: Color::srgb(0.25, 0.55, 0.20),
        interaction: Interaction::Damaging,
        size: 10.0,
    },
];

/// Vegetation spawn rules (1-to-many: same kind can spawn on different terrains).
pub struct VegSpawnRule {
    pub kind: VegetationKind,
    pub terrain: &'static [TileType],
    pub chance: f64,
}

pub const VEG_SPAWN_RULES: &[VegSpawnRule] = &[
    VegSpawnRule { kind: VegetationKind::DeciduousTree, terrain: &[TileType::Forest],                   chance: 0.18 },
    VegSpawnRule { kind: VegetationKind::DeciduousTree, terrain: &[TileType::Grass],                    chance: 0.02 },
    VegSpawnRule { kind: VegetationKind::DeciduousTree, terrain: &[TileType::Meadow],                   chance: 0.04 },
    VegSpawnRule { kind: VegetationKind::PineTree,      terrain: &[TileType::Forest],                   chance: 0.10 },
    VegSpawnRule { kind: VegetationKind::PineTree,      terrain: &[TileType::Tundra],                   chance: 0.04 },
    VegSpawnRule { kind: VegetationKind::PineTree,      terrain: &[TileType::Stone],                    chance: 0.01 },
    VegSpawnRule { kind: VegetationKind::PalmTree,      terrain: &[TileType::Sand],                     chance: 0.02 },
    VegSpawnRule { kind: VegetationKind::Bush,          terrain: &[TileType::Grass, TileType::Meadow],  chance: 0.08 },
    VegSpawnRule { kind: VegetationKind::Bush,          terrain: &[TileType::Dirt, TileType::Tundra],   chance: 0.04 },
    VegSpawnRule { kind: VegetationKind::Flower,        terrain: &[TileType::Meadow, TileType::Grass],  chance: 0.04 },
    VegSpawnRule { kind: VegetationKind::DeadBush,      terrain: &[TileType::Desert],                   chance: 0.06 },
    VegSpawnRule { kind: VegetationKind::Cactus,        terrain: &[TileType::Desert, TileType::Sand],   chance: 0.02 },
];

// ---------------------------------------------------------------------------
// Feature / landmark config
// ---------------------------------------------------------------------------

pub struct FeatureSpawnRule {
    pub terrain: &'static [TileType],
    pub chance: f64,
}

pub struct FeatureConfig {
    pub kind: FeatureKind,
    pub name_en: &'static str,
    pub name_zh: &'static str,
    pub color: Color,
    pub interaction: Interaction,
    pub size: f32,
    pub spawn: FeatureSpawnRule,
}

/// All feature configs, one per FeatureKind variant.
pub const FEATURE_CONFIGS: [FeatureConfig; 7] = [
    FeatureConfig {
        kind: FeatureKind::RockFormation,
        name_en: "Rock Formation",
        name_zh: "岩层",
        color: Color::srgb(0.40, 0.38, 0.35),
        interaction: Interaction::Blocked,
        size: 16.0,
        spawn: FeatureSpawnRule {
            terrain: &[TileType::Stone, TileType::Dirt],
            chance: 0.02,
        },
    },
    FeatureConfig {
        kind: FeatureKind::Ruins,
        name_en: "Ancient Ruins",
        name_zh: "遗迹",
        color: Color::srgb(0.45, 0.35, 0.25),
        interaction: Interaction::Explorable,
        size: 18.0,
        spawn: FeatureSpawnRule {
            terrain: &[TileType::Desert, TileType::Grass, TileType::Sand],
            chance: 0.003,
        },
    },
    FeatureConfig {
        kind: FeatureKind::AncientTree,
        name_en: "Ancient Tree",
        name_zh: "古树",
        color: Color::srgb(0.08, 0.35, 0.08),
        interaction: Interaction::Explorable,
        size: 18.0,
        spawn: FeatureSpawnRule {
            terrain: &[TileType::Forest],
            chance: 0.005,
        },
    },
    FeatureConfig {
        kind: FeatureKind::HotSpring,
        name_en: "Hot Spring",
        name_zh: "温泉",
        color: Color::srgb(0.60, 0.80, 0.90),
        interaction: Interaction::Healing,
        size: 14.0,
        spawn: FeatureSpawnRule {
            terrain: &[TileType::Stone, TileType::Tundra],
            chance: 0.008,
        },
    },
    FeatureConfig {
        kind: FeatureKind::Geyser,
        name_en: "Geyser",
        name_zh: "间歇泉",
        color: Color::srgb(0.70, 0.85, 0.95),
        interaction: Interaction::Damaging,
        size: 14.0,
        spawn: FeatureSpawnRule {
            terrain: &[TileType::Tundra, TileType::Snow],
            chance: 0.004,
        },
    },
    FeatureConfig {
        kind: FeatureKind::MeteorCrater,
        name_en: "Meteor Crater",
        name_zh: "陨石坑",
        color: Color::srgb(0.35, 0.25, 0.15),
        interaction: Interaction::Explorable,
        size: 20.0,
        spawn: FeatureSpawnRule {
            terrain: &[TileType::Desert, TileType::Tundra],
            chance: 0.002,
        },
    },
    FeatureConfig {
        kind: FeatureKind::Fossil,
        name_en: "Fossil",
        name_zh: "化石",
        color: Color::srgb(0.70, 0.65, 0.55),
        interaction: Interaction::Explorable,
        size: 12.0,
        spawn: FeatureSpawnRule {
            terrain: &[TileType::Desert, TileType::Clay, TileType::Stone],
            chance: 0.006,
        },
    },
];

// ---------------------------------------------------------------------------
// Building config
// ---------------------------------------------------------------------------

pub struct BuildingSpawnRule {
    pub terrain: &'static [TileType],
    pub chance: f64,
}

pub struct BuildingConfig {
    pub kind: BuildingKind,
    pub name_en: &'static str,
    pub name_zh: &'static str,
    pub color: Color,
    pub interaction: Interaction,
    pub width: usize,
    pub height: usize,
    pub spawn: BuildingSpawnRule,
}

/// All building configs.
pub const BUILDING_CONFIGS: [BuildingConfig; 5] = [
    BuildingConfig {
        kind: BuildingKind::House,
        name_en: "House",
        name_zh: "房屋",
        color: Color::srgb(0.68, 0.52, 0.30),
        interaction: Interaction::Walkable,
        width: 2,
        height: 2,
        spawn: BuildingSpawnRule {
            terrain: &[TileType::Grass, TileType::Meadow],
            chance: 0.003,
        },
    },
    BuildingConfig {
        kind: BuildingKind::StoneHouse,
        name_en: "Stone House",
        name_zh: "石屋",
        color: Color::srgb(0.50, 0.48, 0.45),
        interaction: Interaction::Walkable,
        width: 2,
        height: 2,
        spawn: BuildingSpawnRule {
            terrain: &[TileType::Stone, TileType::Dirt],
            chance: 0.002,
        },
    },
    BuildingConfig {
        kind: BuildingKind::Watchtower,
        name_en: "Watchtower",
        name_zh: "瞭望塔",
        color: Color::srgb(0.35, 0.30, 0.25),
        interaction: Interaction::Explorable,
        width: 1,
        height: 1,
        spawn: BuildingSpawnRule {
            terrain: &[TileType::Grass, TileType::Stone, TileType::Meadow],
            chance: 0.0005,
        },
    },
    BuildingConfig {
        kind: BuildingKind::Workshop,
        name_en: "Workshop",
        name_zh: "工坊",
        color: Color::srgb(0.55, 0.35, 0.20),
        interaction: Interaction::Walkable,
        width: 3,
        height: 2,
        spawn: BuildingSpawnRule {
            terrain: &[TileType::Dirt, TileType::Clay],
            chance: 0.001,
        },
    },
    BuildingConfig {
        kind: BuildingKind::Well,
        name_en: "Well",
        name_zh: "水井",
        color: Color::srgb(0.40, 0.60, 0.80),
        interaction: Interaction::WaterSource,
        width: 1,
        height: 1,
        spawn: BuildingSpawnRule {
            terrain: &[TileType::Grass, TileType::Sand],
            chance: 0.001,
        },
    },
];
