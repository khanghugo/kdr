//! Resources fetching should go:
//!
//! 0. The user loads the web site. The web site contains code for .dem, .bsp, ... parsers, ghost generation code, and WebGPU render code.
//!
//! 1. User sends a demo to the browser. A client side demo parser will find which map the demo needs.
//! The map name will be sent to the server.
//!
//! 2. The server will search for the map in its copy of the game.
//! If the map is found, the server will find other related resources and send it to client.
//! This process could be sped up by pre-building .res file for every .bsp, which I conveniently have exactly a tool for that.
//!
//! 3. The client receives .bsp, .mdl, .wad,... The client then starts data processing and feed it into renderer.
//!
//! To make this all efficient, we need two repos.
//! Client repo responsible for hosting all of the web site code to send to server.
//! Server repo responsible for processing map name request.
//!
//! This means, all code in this client repo will think about not having access to file system even though it can be used natively.

use std::{collections::HashMap, ffi::OsStr, path::Path};

use bsp_resource::BspResource;
use error::ResourceProviderError;
use ghost::{GhostError, GhostInfo, get_ghost};
use serde::Deserialize;

pub mod bsp_resource;
pub mod error;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod web;

const MODEL_ENTITIES: &[&str] = &["cycler_sprite", "env_sprite"];

// skybox order here, already works.
// need to make sure shader coordinate is flipped accordingly and culling mode is right
// don't touch this
const SKYBOX_SUFFIXES: &[&str] = &["ft", "bk", "up", "dn", "rt", "lf"];

#[derive(Debug, Clone, Deserialize)]
/// Map Identifier is sent from client to server to request files related to the map.
pub struct ResourceIdentifier {
    /// Name of the map. It should not have the ".bsp" extension
    ///
    /// Eg: "/path/to/hl.exe/cstrike/maps/de_dust2.bsp" should have the name "de_dust2".
    ///
    /// The client should sanitize name when sending to server. The server should also sanitize the received name.
    pub map_name: String,

    /// Game mod folder name. This data is inside a demo so it should know where it is.
    ///
    /// We need this so that we don't have to find which game mod it is in.
    ///
    /// Need to be aware of "_downloads" variance. It is very likely that our server will have data inside "_downloads" folder.
    pub game_mod: String,
}

pub type ResourceMap = HashMap<String, Vec<u8>>;

/// .bsp resources is sent from server to client.
pub struct Resource {
    /// Has to be [`bsp::Bsp`] just because native step already parses it.
    pub bsp: bsp::Bsp,

    /// All resources related to .bsp.
    ///
    /// Key: File path. File path should start from game mod not including game mod. Eg: "maps/de_dust2" or "models/fern.mdl".
    ///
    /// Value: Bytes of the associated file.
    pub resources: ResourceMap,
}

impl Resource {
    pub fn to_bsp_resource(self) -> BspResource {
        BspResource::new(self)
    }
}

/// Trait to fetch resources. This is here so that we can have both native and web implementations.
pub trait ResourceProvider {
    /// Gets map resource from given map identifier.
    // TODO: implement some sort of error handling with custom enum?
    async fn get_resource(
        &self,
        identifier: &ResourceIdentifier,
    ) -> Result<Resource, ResourceProviderError>;

    /// The client should also be able to parse a replay and get the map name out of it.
    ///
    /// Since we are dealing with all kinds of replays, we need to identify it at the client level.
    ///
    /// Nicely enough, with browser sandboxed file system, we do know the file name to nicely categorise it.
    ///
    /// The output should be a map identifier and then ghost data.
    ///
    /// The client should handle the error properly.
    async fn get_ghost_data<'a>(
        &self,
        path: impl AsRef<Path> + AsRef<OsStr>,
        ghost_blob: &'a [u8],
    ) -> Result<(ResourceIdentifier, GhostInfo), GhostError> {
        let ghost = get_ghost(path, ghost_blob)?;

        let map_identifier = ResourceIdentifier {
            map_name: ghost.map_name.to_owned(),
            game_mod: ghost.game_mod.to_owned(),
        };

        Ok((map_identifier, ghost))
    }
}

// this makes sure that we have ".bsp in the map name"
fn fix_bsp_file_name(s: &str) -> String {
    if s.ends_with(".bsp") {
        return s.to_string();
    } else {
        format!("{s}.bsp")
    }
}
