use std::str::FromStr;

use poise::serde_json::Value;
use reqwest::{Client, Method, Url};

use crate::Error;

use super::ProviderKind;

impl ProviderKind {
    pub(super) async fn images_imgur(
        &self,
        client_id: &str,
        reqw_client: &Client,
        album: &Url,
    ) -> Result<Vec<Url>, Error> {
        let album_hash = extract_album_hash(album).ok_or("No album hash found")?;
        let response = reqw_client
            .request(
                Method::GET,
                format!("https://api.imgur.com/3/album/{}/images", album_hash),
            )
            .header("Authorization", format!("Client-ID {}", client_id))
            .send()
            .await?;

        let json = response.json::<Value>().await?;
        let images: Vec<_> = json
            .get("data")
            .ok_or("Json has no data field")?
            .as_array()
            .ok_or("Data field is not an array")?
            .iter()
            .filter_map(|obj| obj.get("link"))
            .filter_map(|value| value.as_str())
            .filter_map(|link| Url::from_str(link).ok())
            .collect();

        Ok(images)
    }
}

fn extract_album_hash(album: &Url) -> Option<&str> {
    album.path_segments()?.nth(1)
}
