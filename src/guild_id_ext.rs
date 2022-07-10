//! This module is for extending the [GuildId](GuildId) struct
//! with functions for setting the banner from an URL.

use anyhow::anyhow;
use poise::{
    async_trait,
    serenity_prelude::{GuildId, Http},
};
use rand::prelude::SliceRandom;
use reqwest::Client;
use url::Url;

use crate::Error;

#[async_trait]
pub(crate) trait RandomBanner {
    /// Given a slice of [Url](Url), pick a random entry
    /// and try and set it as the guild banner
    async fn set_random_banner(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'async_trait,
        reqw_client: &Client,
        urls: &[Url],
    ) -> Result<(), Error> {
        let url = urls
            .choose(&mut rand::thread_rng())
            .ok_or(anyhow!("Could not pick a url"))?;

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
    async fn set_banner_from_url(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'async_trait,
        reqw_client: &Client,
        url: &Url,
    ) -> Result<(), Error> {
        let extension = url
            .as_str()
            .split('.')
            .last()
            .ok_or(anyhow!("No file extension on image url"))?;

        let image_bytes = reqw_client.get(url.as_ref()).send().await?.bytes().await?;
        let b64 = base64::encode(&image_bytes);
        self.edit(http.as_ref(), |g| {
            g.banner(Some(&format!("data:image/{};base64,{}", extension, b64)))
        })
        .await?;

        Ok(())
    }
}
