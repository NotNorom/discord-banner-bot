//! This module is for extending the [GuildId](GuildId) struct
//! with functions for setting the banner from an URL.

use base64::Engine;
use poise::{
    async_trait,
    serenity_prelude::{self, GuildId, Http},
};
use rand::prelude::SliceRandom;
use reqwest::Client;
use tracing::{debug, info, instrument};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum SetBannerError {
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
    #[error(transparent)]
    DiscordApi(#[from] serenity_prelude::Error),
    #[error("could not pick a url. rng failed")]
    CouldNotPickAUrl,
    #[error("could not determin file extenstion on image")]
    CouldNotDeterminFileExtension,
    #[error("Missing 'banner' feature")]
    MissingBannerFeature,
    #[error("Image is empty")]
    ImageIsEmpty
}

#[async_trait]
pub(crate) trait RandomBanner {
    /// Given a slice of [Url](Url), pick a random entry
    /// and try and set it as the guild banner
    #[instrument(skip(self, http, reqw_client, urls))]
    async fn set_random_banner(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'async_trait,
        reqw_client: &Client,
        urls: &[Url],
    ) -> Result<(), SetBannerError> {
        let url = urls
            .choose(&mut rand::thread_rng())
            .ok_or(SetBannerError::CouldNotPickAUrl)?;

        self.set_banner_from_url(http, reqw_client, url).await
    }

    /// Given an [Url](Url) to an image, set the guild banner
    /// This will download the image into memory,
    /// convert the bytes to base64 and then send it to discord
    async fn set_banner_from_url(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'async_trait,
        reqw_client: &Client,
        url: &Url,
    ) -> Result<(), SetBannerError>;
}

#[async_trait]
impl RandomBanner for GuildId {
    #[instrument(skip(http, reqw_client, url))]
    async fn set_banner_from_url(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'async_trait,
        reqw_client: &Client,
        url: &Url,
    ) -> Result<(), SetBannerError> {
        // Disable banner feature check when in dev environment
        #[cfg(not(feature = "dev"))]
        {
            let guild = self.to_partial_guild(http.as_ref()).await?;
            let features = guild.features;

            if !features.contains(&"BANNER".to_owned()) {
                return Err(SetBannerError::MissingBannerFeature);
            }
        }

        let extension = url
            .as_str()
            .split('.')
            .last()
            .ok_or_else(|| SetBannerError::CouldNotDeterminFileExtension)?;

        // @todo insert check for animated banners here

        debug!("Found extention: {extension}");

        let image_bytes = reqw_client.get(url.as_ref()).send().await?.bytes().await?;
        let amount_of_bytes = image_bytes.len();
        debug!("Amount of image bytes downloaded: {}", amount_of_bytes);

        if amount_of_bytes == 0 {
            return Err(SetBannerError::ImageIsEmpty);
        }

        let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);

        let payload = format!("data:image/{extension};base64,{b64}");

        self.edit(http.as_ref(), |g| {
            #[cfg(feature = "dev")]
            {
                debug!("Setting icon");
                g.icon(Some(&payload))
            }

            #[cfg(not(feature = "dev"))]
            {
                debug!("Setting banner");
                g.banner(Some(&payload))
            }
        })
        .await?;

        info!("Guild {} changed to: {}", self.0, url.as_str());

        Ok(())
    }
}
