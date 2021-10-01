use std::str::FromStr;

use poise::{async_trait, serde_json::Value};
use reqwest::{Client, Method, Url};

use crate::Error;

use super::Provider;

/// Provider for an Imgur album like "https://imgur.com/a/YM1yHhx"
pub struct ImgurAlbum<'a> {
    client: &'a Client,
    client_id: &'a str,
}

impl<'a> ImgurAlbum<'a> {
    pub fn new(client: &'a Client, client_id: &'a str) -> Self {
        Self { client, client_id }
    }

    /// Given an imgur link like https://imgur.com/a/YM1yHhx, return just the YM1yHhx part.
    fn extract_album_hash(album: &Url) -> Option<&str> {
        album.path_segments()?.nth(1)
    }
}

#[async_trait]
impl<'a> Provider for ImgurAlbum<'a> {
    async fn album(&self, album: &Url) -> Result<Vec<Url>, Error> {
        let album_hash = Self::extract_album_hash(album).ok_or("No album hash found")?;
        let response = self
            .client
            .request(
                Method::GET,
                format!("https://api.imgur.com/3/album/{}/images", album_hash),
            )
            .header("Authorization", format!("Client-ID {}", self.client_id))
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
