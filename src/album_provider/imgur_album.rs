use std::str::FromStr;

use imgurs::ImgurClient;
use poise::async_trait;
use reqwest::Url;
use tracing::instrument;

use crate::{Error, Error::ImgurIdExtraction};

use super::Provider;

pub(super) struct Imgur(ImgurClient);

impl Imgur {
    pub fn new(client: ImgurClient) -> Self {
        Self(client)
    }
}

#[async_trait]
impl Provider for Imgur {
    #[instrument(skip(self, album))]
    async fn provide(&self, album: &Url) -> Result<Vec<Url>, Error> {
        let album_id = extract_album_id(album)?;

        let album_data = self.0.album_info(album_id).await?;
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

/// Extract the album id of an imgur url
#[instrument]
fn extract_album_id(album: &Url) -> Result<&str, Error> {
    let Some(path_segments) = album.path_segments() else {
        return Err(ImgurIdExtraction("No id found. Are you missing the part behind the '/' ?".into()));
    };

    let path_segments: Vec<_> = path_segments.collect();

    let Some(id) = path_segments.last() else {
        return Err(ImgurIdExtraction("No id found. Are you missing the part behind the '/' ?".into()));
    };

    if id.split_whitespace().count() > 1 {
        return Err(ImgurIdExtraction("Id contains whitespaces".into()));
    }

    Ok(id)
}
