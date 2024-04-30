//! This module is for extending the [GuildId](GuildId) struct
//! with functions for setting the banner from an URL.

use std::collections::HashMap;

use poise::{
    async_trait,
    serenity_prelude::{self, CreateAttachment, EditGuild, GuildId, Http},
};
use rand::prelude::SliceRandom;
use reqwest::Client;
use tracing::{debug, info, instrument};
use url::Url;

use crate::constants::MAXIMUM_IMAGE_SIZE;

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
    #[error("Image is empty: {}", .0)]
    ImageIsEmpty(Url),
    #[error("Image is to big: {}", .0)]
    ImageIsTooBig(Url),
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
            use serenity_prelude::small_fixed_array::FixedString;

            let guild = self.to_partial_guild(http.as_ref()).await?;
            let features = guild.features;

            if !features.contains(&FixedString::from_static_trunc("BANNER")) {
                return Err(SetBannerError::MissingBannerFeature);
            }
        }

        let extension = url
            .path()
            .split('.')
            .last()
            .ok_or_else(|| SetBannerError::CouldNotDeterminFileExtension)?;

        debug!("Found extension: {extension}");
        // @todo insert check for animated banners here

        // discord cdn has a few image parameters and we can limit the size here
        // hopefully preventing us from runnig into the 10mb limit.
        // the only thing thats a little wierd is having to save the ex, is and hm
        // query parameters. oh well.
        let mut query_params = HashMap::with_capacity(3);
        for (key, value) in url
            .query_pairs()
            .filter(|(key, _)| matches!(key.as_bytes(), b"ex" | b"is" | b"hm"))
        {
            query_params.insert(key, value);
        }

        let mut url = url.clone();
        url.query_pairs_mut()
            .clear()
            .extend_pairs(query_params.iter())
            .append_pair("width", "1920")
            .append_pair("height", "1080")
            .finish();

        let image_bytes = reqw_client
            .get(url.as_ref())
            .send()
            .await?
            .bytes()
            .await?
            .to_vec();
        let amount_of_bytes = image_bytes.len();
        debug!("Amount of image bytes downloaded: {}", amount_of_bytes);

        match amount_of_bytes {
            0 => return Err(SetBannerError::ImageIsEmpty(url.to_owned())),
            MAXIMUM_IMAGE_SIZE.. => return Err(SetBannerError::ImageIsTooBig(url.to_owned())), // 10mb
            _ => {}
        };

        let attachment = CreateAttachment::bytes(image_bytes, format!("banner.{extension}"));

        let edit_guild = {
            #[cfg(feature = "dev")]
            {
                debug!("Setting icon");
                EditGuild::new().icon(Some(&attachment))
            }

            #[cfg(not(feature = "dev"))]
            {
                debug!("Setting banner");
                EditGuild::new().banner(Some(&attachment))
            }
        };

        self.edit(http.as_ref(), edit_guild).await?;

        info!("Guild {} changed to: {}", self.get(), url.as_str());

        Ok(())
    }
}
