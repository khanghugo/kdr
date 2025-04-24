pub const UNKNOWN_GAME_MOD: &str = "unknown";

pub const REQUEST_MAP_ENDPOINT: &str = "request-map";
pub const REQUEST_REPLAY_ENDPOINT: &str = "request-replay";
pub const REQUEST_COMMON_RESOURCE_ENDPOINT: &str = "request-common";
pub const REQUEST_MAP_LIST_ENDPOINT: &str = "request-map-list";
pub const REQUEST_REPLAY_LIST: &str = "request-replay-list";

pub const REQUEST_MAP_GAME_MOD_QUERY: &str = "game-mod";

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
