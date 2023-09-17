use std::str::FromStr;

use imgurs::ImgurClient;
use poise::async_trait;
use reqwest::Url;
use tracing::instrument;

use super::{Provider, ProviderError};

#[derive(Debug)]
pub(super) struct Imgur(ImgurClient);

impl Imgur {
    pub fn new(client: ImgurClient) -> Self {
        Self(client)
    }
}

#[async_trait]
impl Provider for Imgur {
    #[instrument(skip_all)]
    async fn provide(&self, album: &Url) -> Result<Vec<Url>, ProviderError> {
        let album_id = extract_album_id(album)?;

        let album_data = self.0.album_info(album_id).await?;
        let images: Vec<Url> = album_data
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
#[instrument(skip_all)]
fn extract_album_id(album: &Url) -> Result<&str, ProviderError> {
    let Some(path_segments) = album.path_segments() else {
        return Err(ProviderError::ImgurIdExtraction(
            "No id found. Are you missing the part behind the '/' ?".into(),
        ));
    };

    let path_segments: Vec<_> = path_segments.collect();

    let Some(id) = path_segments.last() else {
        return Err(ProviderError::ImgurIdExtraction(
            "No id found. Are you missing the part behind the '/' ?".into(),
        ));
    };

    if id.split_whitespace().count() > 1 {
        return Err(ProviderError::ImgurIdExtraction("Id contains whitespaces".into()));
    }

    Ok(id)
}
