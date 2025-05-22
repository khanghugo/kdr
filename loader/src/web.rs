use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use bsp::Bsp;
use common::{
    API_SCOPE_VERSION, GET_MAPS_ENDPOINT, GET_REPLAYS_ENDPOINT, REQUEST_COMMON_RESOURCE_ENDPOINT,
};
use futures_util::StreamExt;
use ghost::GhostBlob;
use zip::ZipArchive;

use crate::{
    ProgressResourceProvider, ResourceMap, error::ResourceProviderError, fix_bsp_file_name,
};

use super::ResourceProvider;

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

enum RequestMethod {
    GET,
    POST,
}

impl WebResourceProvider {
    async fn get_with_progress(
        url: &str,
        progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<Vec<u8>, ResourceProviderError> {
        let client = reqwest::Client::new();

        let response = client
            .get(url)
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

        Ok(all_bytes)
    }

    pub fn web_resource_zip_bytes_to_resource(
        zip_bytes: Vec<u8>,
        map_name: String,
    ) -> Result<crate::Resource, ResourceProviderError> {
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

    pub async fn request_map_with_uri_with_progress(
        identifier: &crate::MapIdentifier,
        uri: &str,
        progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<crate::Resource, ResourceProviderError> {
        let map_name = fix_bsp_file_name(identifier.map_name.as_str());

        let all_bytes = Self::get_with_progress(uri, progress_callback).await?;

        Self::web_resource_zip_bytes_to_resource(all_bytes, map_name)
    }
}

impl ProgressResourceProvider for WebResourceProvider {
    async fn get_map_with_progress(
        &self,
        identifier: &crate::MapIdentifier,
        progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<crate::Resource, ResourceProviderError> {
        let map_name = fix_bsp_file_name(identifier.map_name.as_str());
        let url = format!(
            "{}/{API_SCOPE_VERSION}/{GET_MAPS_ENDPOINT}/{}/{}",
            self.base_url, identifier.game_mod, map_name
        );

        let all_bytes = Self::get_with_progress(&url, progress_callback).await?;

        Self::web_resource_zip_bytes_to_resource(all_bytes, map_name)
    }

    async fn get_replay_with_progress(
        &self,
        replay_name: &str,
        progress_callback: impl Fn(f32) + Send + 'static,
    ) -> Result<ghost::GhostBlob, ResourceProviderError> {
        let url = format!(
            "{}/{API_SCOPE_VERSION}/{GET_REPLAYS_ENDPOINT}/{}",
            self.base_url, replay_name
        );

        let all_bytes = Self::get_with_progress(&url, progress_callback).await?;
        let ghost_blob: GhostBlob = rmp_serde::from_slice(&all_bytes).unwrap();

        Ok(ghost_blob)
    }
}

impl ResourceProvider for WebResourceProvider {
    async fn get_map(
        &self,
        identifier: &super::MapIdentifier,
    ) -> Result<super::Resource, super::error::ResourceProviderError> {
        let dummy_callback = |_: f32| {};

        self.get_map_with_progress(identifier, dummy_callback).await
    }

    async fn get_map_list(&self) -> Result<crate::MapList, ResourceProviderError> {
        let url = format!("{}/{API_SCOPE_VERSION}/{GET_MAPS_ENDPOINT}", self.base_url);

        let response = reqwest::get(url)
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|op| ResourceProviderError::RequestError { source: op })?;

        response
            .json()
            .await
            .map_err(|op| ResourceProviderError::ResponsePayloadError { source: op })
    }

    async fn get_replay_list(&self) -> Result<crate::ReplayList, ResourceProviderError> {
        let url = format!(
            "{}/{API_SCOPE_VERSION}/{GET_REPLAYS_ENDPOINT}",
            self.base_url
        );

        let response = reqwest::get(url)
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|op| ResourceProviderError::RequestError { source: op })?;

        response
            .json()
            .await
            .map_err(|op| ResourceProviderError::ResponsePayloadError { source: op })
    }

    async fn get_replay(
        &self,
        replay_name: &str,
    ) -> Result<ghost::GhostBlob, ResourceProviderError> {
        let dummy_callback = |_: f32| {};

        self.get_replay_with_progress(replay_name, dummy_callback)
            .await
    }

    async fn request_common_resource(&self) -> Result<ResourceMap, ResourceProviderError> {
        let url = format!(
            "{}/{API_SCOPE_VERSION}/{REQUEST_COMMON_RESOURCE_ENDPOINT}",
            self.base_url
        );

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
