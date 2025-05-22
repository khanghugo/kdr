pub const UNKNOWN_GAME_MOD: &str = "unknown";

pub const API_SCOPE_VERSION: &str = "v1";
pub const GET_MAPS_ENDPOINT: &str = "maps";
pub const GET_REPLAYS_ENDPOINT: &str = "replays";
pub const REQUEST_COMMON_RESOURCE_ENDPOINT: &str = "common-resource";

// for CheckHostConfiguration
pub const REQUEST_REPLAY_NAME_QUERY: &str = "replay";
pub const REQUEST_MAP_NAME_QUERY: &str = "map";
pub const REQUEST_MAP_GAME_MOD_QUERY: &str = "game";
pub const REQUEST_MAP_URI_QUERY: &str = "uri";

pub const KDR_CANVAS_ID: &str = "kdr-canvas";

pub const CANNOT_FIND_REQUESTED_MAP_ERROR: &str = "Cannot find requested map";
pub const CANNOT_FIND_REQUESTED_REPLAY_ERR: &str = "Cannot find requested replay";

pub const NO_DRAW_FUNC_BRUSHES: &[&str] = &[
    "func_hostage_rescue",
    "func_ladder",
    "func_buyzone",
    "func_bomb_target",
    "func_monster_clip",
];

pub const CONFIG_FILE_NAME: &str = "config.toml";

// must include the downloads variance because that is easier for me
// TODO: make this inside a config file, maybe a do a lazy cell to parse the config
// the worst to come is that we have to read a config file once multiple times wherever applicable :()
pub const COMMON_GAME_MODS: &[&str] = &[
    "valve",
    "valve_downloads",
    "ag",
    "ag_downloads",
    "cstrike",
    "cstrike_downloads",
];

pub const COMMON_RESOURCE_SOUND: &[&str] = &[
    "sound/player/pl_step1.wav",
    "sound/player/pl_step2.wav",
    "sound/player/pl_step3.wav",
    "sound/player/pl_step4.wav",
    "sound/common/wpn_select.wav",
    "sound/weapons/knife_hitwall1.wav",
    "sound/common/wpn_denyselect.wav",
    "sound/weapons/knife_slash2.wav",
];

pub const RESOURCE_VIEWMODELS: &[&str] = &[
    "models/v_ak47.mdl",
    "models/v_awp.mdl",
    "models/v_deagle.mdl",
    "models/v_famas.mdl",
    "models/v_knife.mdl",
    "models/v_m4a1.mdl",
    "models/v_m249.mdl",
    "models/v_p90.mdl",
    "models/v_scout.mdl",
    "models/v_sg552.mdl",
    "models/v_usp.mdl",
];

pub const RESOURCE_PLAYER_MODELS: &[&str] = &["models/player/leet/leet.mdl"];
