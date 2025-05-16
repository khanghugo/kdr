use serde::{Deserialize, Serialize};

use super::*;

// Order of appearance for serde.
#[derive(Serialize, Deserialize, Debug)]
struct RjGhostInfo {
    frames: Vec<RjGhostFrame>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RjGhostFrame {
    #[serde(rename = "position")]
    origin: [f32; 3],
    #[serde(rename = "orientation")]
    viewangles: [f32; 2], // pitch yaw
    #[serde(rename = "length")]
    frametime: f32,
    time: f32, // total time
    buttons: u32,
}

pub fn romanian_jumpers_ghost_parse(filename: &str, file: &str) -> eyre::Result<GhostInfo> {
    let romanian_jumpers_ghost: RjGhostInfo = serde_json::from_str(&file)?;

    // Convert romanian_jumpers_ghost to our normal ghost.
    Ok(GhostInfo {
        ghost_name: filename.to_owned(),
        map_name: "NoMapName".to_string(),
        game_mod: "cstrike".to_string(),
        frames: romanian_jumpers_ghost
            .frames
            .iter()
            .map(|ghost| GhostFrame {
                frametime: Some(ghost.frametime),
                origin: Vec3::from_array([ghost.origin[0], -ghost.origin[2], ghost.origin[1]]),
                viewangles: Vec3::from_array([ghost.viewangles[0], ghost.viewangles[1], 0.]),
                viewoffset_z: 0.,
                buttons: ghost.buttons.into(),
                fov: None,
                extras: None,
            })
            .collect(),
    })
}
