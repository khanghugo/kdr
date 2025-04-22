use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use bsp::Bsp;
use common::{
    REQUEST_COMMON_RESOURCE_ENDPOINT, REQUEST_MAP_ENDPOINT, REQUEST_MAP_LIST_ENDPOINT,
    REQUEST_REPLAY_ENDPOINT, REQUEST_REPLAY_LIST,
};
use futures_util::StreamExt;
use ghost::GhostBlob;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::{
    MapList, ProgressResourceProvider, ResourceMap, error::ResourceProviderError, fix_bsp_file_name,
};

use super::ResourceProvider;

const MAP_NAME_KEY: &str = "map_name";
const GAME_MOD_KEY: &str = "game_mod";
const REPLAY_NAME_KEY: &str = "replay_name";

#[derive(Debug, Clone)]
pub struct WebResourceProvider {
    pub base_url: String,
}

impl WebResourceProvider {
    pub fn new(base_url: impl AsRef<str> + Into<String>) -> Self {
        Self {
            base_url: sanitize_base_url(base_url.as_ref()).to_string(),
        }
    }
}

impl ProgressResourceProvider for WebResourceProvider {
    async fn request_map_with_progress(
        &self,
        identifier: &crate::MapIdentifier,
        progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<crate::Resource, ResourceProviderError> {
        let map_name = fix_bsp_file_name(identifier.map_name.as_str());

        let url = format!("{}/{}", self.base_url, REQUEST_MAP_ENDPOINT);
        let client = reqwest::Client::new();

        let mut map = HashMap::new();
        map.insert(MAP_NAME_KEY, &map_name);
        map.insert(GAME_MOD_KEY, &identifier.game_mod);

        let response = client
            .post(url)
            .json(&map)
            .send()
            .await
            // dont map err for status
            // we want to read the error body at the very least so the client can display it
            // .error_for_status()
            .map_err(|op| ResourceProviderError::RequestError { source: op })?;

        // this means the server cannot find the request, so we just exit and return error
        const NOT_FOUND_CODE: u16 = 404;
        let status_code = response.status().as_u16();

        if status_code == NOT_FOUND_CODE {
            if let Ok(body) = response.text().await {
                return Err(ResourceProviderError::ResponseError {
                    status_code,
                    message: body,
                });
            }

            return Err(ResourceProviderError::ResponseError {
                status_code,
                message: "No message".to_string(),
            });
        };

        let response =
            response
                .error_for_status()
                .map_err(|_op| ResourceProviderError::ResponseError {
                    status_code,
                    message: "No message".to_string(),
                })?;

        // progress reporting
        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut byte_stream = response.bytes_stream();
        let mut all_bytes = Vec::new();

        while let Some(chunk) = byte_stream.next().await {
            let chunk =
                chunk.map_err(|op| ResourceProviderError::ResponsePayloadError { source: op })?;
            downloaded += chunk.len() as u64;
            all_bytes.extend_from_slice(&chunk);

            if total_size > 0 {
                progress_callback(downloaded as f32 / total_size as f32);
            }
        }

        let zip_bytes = all_bytes;

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

    async fn request_replay_with_progress(
        &self,
        replay_name: &str,
        progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<ghost::GhostBlob, ResourceProviderError> {
        // mimicking request_map_with_progress
        let url = format!("{}/{}", self.base_url, REQUEST_REPLAY_ENDPOINT);
        let client = reqwest::Client::new();

        let mut map = HashMap::new();
        map.insert(REPLAY_NAME_KEY, &replay_name);

        let response = client
            .post(url)
            .json(&map)
            .send()
            .await
            .map_err(|op| ResourceProviderError::RequestError { source: op })?;

        const NOT_FOUND_CODE: u16 = 404;
        let status_code = response.status().as_u16();

        if status_code == NOT_FOUND_CODE {
            if let Ok(body) = response.text().await {
                return Err(ResourceProviderError::ResponseError {
                    status_code,
                    message: body,
                });
            }

            return Err(ResourceProviderError::ResponseError {
                status_code,
                message: "No message".to_string(),
            });
        };

        let response =
            response
                .error_for_status()
                .map_err(|_op| ResourceProviderError::ResponseError {
                    status_code,
                    message: "No message".to_string(),
                })?;

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut byte_stream = response.bytes_stream();
        let mut all_bytes = Vec::new();

        while let Some(chunk) = byte_stream.next().await {
            let chunk =
                chunk.map_err(|op| ResourceProviderError::ResponsePayloadError { source: op })?;
            downloaded += chunk.len() as u64;
            all_bytes.extend_from_slice(&chunk);

            if total_size > 0 {
                progress_callback(downloaded as f32 / total_size as f32);
            }
        }

        let ghost_blob: GhostBlob = serde_json::from_slice(&all_bytes).unwrap();

        Ok(ghost_blob)
    }
}

impl ResourceProvider for WebResourceProvider {
    async fn request_map(
        &self,
        identifier: &super::MapIdentifier,
    ) -> Result<super::Resource, super::error::ResourceProviderError> {
        let dummy_callback = |_: f32| {};

        self.request_map_with_progress(identifier, dummy_callback)
            .await
    }

    async fn request_map_list(&self) -> Result<crate::MapList, ResourceProviderError> {
        let url = format!("{}/{}", self.base_url, REQUEST_MAP_LIST_ENDPOINT);

        let response = reqwest::get(url)
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|op| ResourceProviderError::RequestError { source: op })?;

        response
            .json()
            .await
            .map_err(|op| ResourceProviderError::ResponsePayloadError { source: op })
    }

    async fn request_replay_list(&self) -> Result<crate::ReplayList, ResourceProviderError> {
        let url = format!("{}/{}", self.base_url, REQUEST_REPLAY_LIST);

        let response = reqwest::get(url)
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|op| ResourceProviderError::RequestError { source: op })?;

        response
            .json()
            .await
            .map_err(|op| ResourceProviderError::ResponsePayloadError { source: op })
    }

    async fn request_replay(
        &self,
        replay_name: &str,
    ) -> Result<ghost::GhostBlob, ResourceProviderError> {
        let dummy_callback = |_: f32| {};

        self.request_replay_with_progress(replay_name, dummy_callback)
            .await
    }
}

impl WebResourceProvider {
    pub async fn request_common_resource(&self) -> Result<ResourceMap, ResourceProviderError> {
        let url = format!("{}/{}", self.base_url, REQUEST_COMMON_RESOURCE_ENDPOINT);

        let response = reqwest::get(url)
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|op| ResourceProviderError::RequestError { source: op })?;

        let status_code = response.status().as_u16();

        const HTTP_NO_CONTENT: u16 = 204;

        // explicitly return empty hash map
        if status_code == HTTP_NO_CONTENT {
            return Ok(ResourceMap::new());
        }

        let response =
            response
                .error_for_status()
                .map_err(|_op| ResourceProviderError::ResponseError {
                    status_code,
                    message: "No message".to_string(),
                })?;

        let zip_bytes = response
            .bytes()
            .await
            .map_err(|op| ResourceProviderError::ResponsePayloadError { source: op })?;

        extract_zip_to_hashmap(&zip_bytes)
            .map_err(|op| ResourceProviderError::ZipDecompress { source: op })
    }
}

fn extract_zip_to_hashmap(zip_bytes: &[u8]) -> Result<ResourceMap, zip::result::ZipError> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(reader)?;
    let mut file_map = ResourceMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let filename = file.name().to_string();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        file_map.insert(filename, contents);
    }

    Ok(file_map)
}

// eh, good enough
fn sanitize_base_url(s: &str) -> &str {
    let l = s.len();

    if s.ends_with("/") {
        return &s[..(l - 1)];
    } else {
        return s;
    }
}

pub fn parse_location_search(s: &str) -> HashMap<String, String> {
    s.trim_start_matches("?")
        .split_terminator("&")
        .filter_map(|pairs| {
            let what: Vec<&str> = pairs.split_terminator("=").collect();

            let [key, value] = what.as_slice() else {
                return None;
            };

            Some((key.to_string(), value.to_string()))
        })
        .collect()
}
