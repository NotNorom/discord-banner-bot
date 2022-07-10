use std::str::FromStr;

use anyhow::anyhow;
use poise::serenity_prelude::json::Value;
use reqwest::{Client, Method, Url};

use crate::{Error, Error::ImgurHashExtraction};

use super::Provider;

impl Provider {
    pub(super) async fn images_imgur(
        &self,
        client_id: &str,
        reqw_client: &Client,
        album: &Url,
    ) -> Result<Vec<Url>, Error> {
        let album_id = extract_album_hash(album)?;
        let response = reqw_client
            .request(
                Method::GET,
                format!("https://api.imgur.com/3/album/{}/images", album_id),
            )
            .header("Authorization", format!("Client-ID {}", client_id))
            .send()
            .await?;

        let json = response.json::<Value>().await?;
        let images: Vec<_> = json
            .get("data")
            .ok_or(anyhow!("Json has no data field"))?
            .as_array()
            .ok_or(anyhow!("Data field is not an array"))?
            .iter()
            .filter_map(|obj| obj.get("link"))
            .filter_map(|value| value.as_str())
            .filter_map(|link| Url::from_str(link).ok())
            .collect();

        Ok(images)
    }
}

fn extract_album_hash(album: &Url) -> Result<&str, Error> {
    let hash_url_segment = album
        .path_segments()
        .ok_or(ImgurHashExtraction("No path segments".into()))
        .and_then(|mut segments| {
            segments
                .nth(1)
                .ok_or(ImgurHashExtraction("Missing path segment, needs to be 2".into()))
        })?;

    if hash_url_segment.split_whitespace().count() > 1 {
        return Err(ImgurHashExtraction("hash may not contain white spaces".into()));
    }

    Ok(hash_url_segment)
}
