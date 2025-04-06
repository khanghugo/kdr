use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use bsp::Bsp;
use zip::ZipArchive;

use crate::loader::{error::ResourceProviderError, fix_bsp_file_name};

use super::ResourceProvider;

const MAP_NAME_KEY: &str = "map_name";
const GAME_MOD_KEY: &str = "game_mod";
const REQUEST_ENDPOINT: &str = "request-map";

#[derive(Debug, Clone)]
pub struct WebResourceProvider {
    pub base_url: String,
}

impl WebResourceProvider {
    pub fn new(base_url: impl AsRef<str> + Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

impl ResourceProvider for WebResourceProvider {
    async fn get_resource(
        &self,
        identifier: &super::ResourceIdentifier,
    ) -> Result<super::Resource, super::error::ResourceProviderError> {
        let map_name = fix_bsp_file_name(identifier.map_name.as_str());

        let url = format!("{}/{}", self.base_url, REQUEST_ENDPOINT);
        let client = reqwest::Client::new();

        let mut map = HashMap::new();
        map.insert(MAP_NAME_KEY, &map_name);
        map.insert(GAME_MOD_KEY, &identifier.game_mod);

        let response = client
            .post(url)
            .json(&map)
            .send()
            .await
            .map_err(|op| ResourceProviderError::PostError { source: op })?
            .error_for_status()
            .map_err(|op| ResourceProviderError::ResponseError { source: op })?;

        let zip_bytes = response
            .bytes()
            .await
            .map_err(|op| ResourceProviderError::ResponseBytesError { source: op })?;

        let extracted_files = extract_zip_to_hashmap(&zip_bytes)
            .map_err(|op| ResourceProviderError::ZipDecompress { source: op })?;

        // the bsp is inside our extracted files
        let bsp_bytes = extracted_files
            .get(format!("maps/{map_name}").as_str())
            .ok_or_else(|| ResourceProviderError::BspFromArchive)?;
        let bsp =
            Bsp::from_bytes(bsp_bytes).map_err(|op| ResourceProviderError::CannotParseBsp {
                source: op,
                bsp_name: map_name,
            })?;

        Ok(super::Resource {
            bsp,
            resources: extracted_files,
        })
    }
}

fn extract_zip_to_hashmap(
    zip_bytes: &[u8],
) -> Result<HashMap<String, Vec<u8>>, zip::result::ZipError> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(reader)?;
    let mut file_map: HashMap<String, Vec<u8>> = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let filename = file.name().to_string();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        file_map.insert(filename, contents);
    }

    Ok(file_map)
}
