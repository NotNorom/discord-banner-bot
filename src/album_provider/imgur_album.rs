use std::str::FromStr;

use imgurs::ImgurClient;
use reqwest::{Client, Url};

use crate::{Error, Error::ImgurHashExtraction};

use super::Provider;

impl Provider {
    pub(super) async fn images_imgur(
        &self,
        client_id: &str,
        reqw_client: &Client,
        album: &Url,
    ) -> Result<Vec<Url>, Error> {
        let imgur_client = ImgurClient::with_http_client(client_id, reqw_client.clone());

        let album_id = extract_album_hash(album)?;

        let album_data = imgur_client.album_info(album_id).await?;
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
        return Err(ImgurHashExtraction("hash may not contain white spaces".into()));
    }

    Ok(hash_url_segment)
}
