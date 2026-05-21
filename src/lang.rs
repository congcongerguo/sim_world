use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Language
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    #[default]
    Zh,
}

#[derive(Resource, Default)]
pub struct GameLang(pub Lang);

// ---------------------------------------------------------------------------
// Translation
// ---------------------------------------------------------------------------

/// Translate `en` according to `lang`.
pub fn tr(en: &'static str, lang: Lang) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Zh => zh(en),
    }
}

/// Return the Chinese translation for the given English string key.
pub fn zh(key: &'static str) -> &'static str {
    ZH.iter().find_map(|(k, v)| (*k == key).then_some(*v)).unwrap_or(key)
}

// ---------------------------------------------------------------------------
// Chinese dictionary
// ---------------------------------------------------------------------------

static ZH: &[(&str, &str)] = &[
    // Tile types
    ("Grass", "草地"),
    ("Water", "浅水"),
    ("Deep Water", "深海"),
    ("Sand", "沙地"),
    ("Forest", "森林"),
    ("Swamp", "沼泽"),
    ("Stone", "岩石"),
    ("Dirt", "泥土"),
    ("Snow", "雪地"),
    ("Lava", "熔岩"),
    ("Tundra", "冻原"),
    ("Ice", "冰原"),
    ("Meadow", "草甸"),
    ("Desert", "沙漠"),
    ("Clay", "黏土"),
    // Feature names (must match FeatureKind::name())
    ("Rock Formation", "岩层"),
    ("Ancient Ruins", "遗迹"),
    ("Ancient Tree", "古树"),
    ("Hot Spring", "温泉"),
    ("Geyser", "间歇泉"),
    ("Meteor Crater", "陨石坑"),
    ("Fossil", "化石"),
    // Resource names (must match ResourceKind::name())
    ("Iron Ore", "铁矿"),
    ("Coal", "煤矿"),
    ("Copper Ore", "铜矿"),
    ("Gold Ore", "金矿"),
    ("Clay", "黏土矿"),
    ("Sand", "沙矿"),
    ("Stone", "石矿"),
    // Vegetation names
    ("Deciduous Tree", "落叶树"),
    ("Pine Tree", "松树"),
    ("Palm Tree", "棕榈树"),
    ("Bush", "灌木"),
    ("Flower", "花"),
    ("Dead Bush", "枯草"),
    ("Cactus", "仙人掌"),
    // Building names
    ("House", "房屋"),
    ("Stone House", "石屋"),
    ("Watchtower", "瞭望塔"),
    ("Workshop", "工坊"),
    ("Well", "水井"),
    // UI strings
    ("Hover over the map", "悬停在地图上查看信息"),
    ("Tile", "坐标"),
    ("Elev", "海拔"),
    ("Moist", "湿度"),
    ("── Z Layers ──", "── 层级 ──"),
    ("air", "空中"),
    ("landmark", "地标"),
    ("resource", "资源"),
    ("vegetation", "植被"),
    ("building", "建筑"),
    ("underground", "地下"),
    ("Cloud", "云"),
    ("Bird", "鸟"),
    ("Cave", "洞穴"),
    ("Player", "角色"),
    ("Character", "角色"),
    ("character", "角色"),
    ("Time", "时间"),
    ("PAUSED", "已暂停"),
    ("Farmland", "农田"),
    ("Fallow", "空闲"),
    ("Growing", "生长中"),
    ("Weedy", "杂草"),
    ("Ready", "可收获"),
    ("Clearing", "开垦中"),
    ("Storage", "储量"),
    ("farm", "农田"),
    ("Action", "操作"),
    ("Press C to interact", "按 C 键操作"),
    ("Adult", "成人"),
    ("Child", "小孩"),
    ("Adults", "成人"),
    ("Children", "小孩"),
    ("Tombstone", "墓碑"),
    ("Cause", "死因"),
    ("Old Age", "寿终"),
];

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct LangPlugin;

impl Plugin for LangPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameLang>();
        app.add_systems(Update, lang_switch_input);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn lang_switch_input(keys: Res<ButtonInput<KeyCode>>, mut lang: ResMut<GameLang>) {
    if keys.just_pressed(KeyCode::F1) {
        lang.0 = Lang::En;
    }
    if keys.just_pressed(KeyCode::F2) {
        lang.0 = Lang::Zh;
    }
}
