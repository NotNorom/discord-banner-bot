//! This module is for extending the [GuildId](GuildId) struct
//! with functions for setting the banner from an URL.

use anyhow::anyhow;
use base64::Engine;
use poise::{
    async_trait,
    serenity_prelude::{GuildId, Http},
};
use rand::prelude::SliceRandom;
use reqwest::Client;
use tracing::{debug, info, instrument};
use url::Url;

use crate::Error;

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
    ) -> Result<(), Error> {
        let url = urls
            .choose(&mut rand::thread_rng())
            .ok_or_else(|| anyhow!("Could not pick a url"))?;

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
    ) -> Result<(), Error>;
}

#[async_trait]
impl RandomBanner for GuildId {
    #[instrument(skip(http, reqw_client, url))]
    async fn set_banner_from_url(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'async_trait,
        reqw_client: &Client,
        url: &Url,
    ) -> Result<(), Error> {
        // Disable banner feature check when in dev environment
        #[cfg(not(feature = "dev"))]
        {
            let guild = self.to_partial_guild(http.as_ref()).await?;
            let features = guild.features;

            if !features.contains(&"BANNER".to_owned()) {
                return Err(Error::Command(crate::error::Command::GuildHasNoBannerSet));
            }
        }

        let extension = url
            .as_str()
            .split('.')
            .last()
            .ok_or_else(|| anyhow!("No file extension on image url"))?;

        debug!("Found extention: {extension}");

        let image_bytes = reqw_client.get(url.as_ref()).send().await?.bytes().await?;
        debug!("Amount of downloaded image bytes: {}", image_bytes.len());
        let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);

        debug!("Base64 image bytes: {b64}");
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

        info!("{} changed banner to: {}", self.0, url);

        Ok(())
    }
}
