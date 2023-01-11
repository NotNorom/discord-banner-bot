use std::str::FromStr;

use imgurs::ImgurClient;
use reqwest::Url;

use crate::{Error, Error::ImgurHashExtraction};

use super::Providers;

impl Providers {
    pub(super) async fn images_imgur(&self, client: &ImgurClient, album: &Url) -> Result<Vec<Url>, Error> {
        let album_id = extract_album_hash(album)?;

        let album_data = client.album_info(album_id).await?;
        let images = album_data
            .data
            .images
            .iter()
            .map(|image_info| image_info.link.clone())
            .filter_map(|link| Url::from_str(&link).ok())
            .collect();

        Ok(images)
    }
}

/// Extract the hash part of an imgur url
fn extract_album_hash(album: &Url) -> Result<&str, Error> {
    let hash_url_segment = album
        .path_segments()
        .ok_or_else(|| ImgurHashExtraction("No path segments".into()))
        .and_then(|mut segments| {
            segments
                .nth(1)
                .ok_or_else(|| ImgurHashExtraction("Missing path segment, needs to be 2".into()))
        })?;

    if hash_url_segment.split_whitespace().count() > 1 {
        return Err(ImgurHashExtraction("Hash contains whitespaces".into()));
    }

    Ok(hash_url_segment)
}
