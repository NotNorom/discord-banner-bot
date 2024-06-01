//! This module is for extending the [GuildId](GuildId) struct
//! with functions for setting the banner from an URL.

use std::collections::HashMap;

use bytes::Bytes;
use poise::serenity_prelude::{self, futures::TryStreamExt, CreateAttachment, EditGuild, GuildId, Http};
use rand::prelude::SliceRandom;
use reqwest::Client;
use tracing::{debug, info, instrument};
use url::Url;

use crate::constants::MAXIMUM_IMAGE_SIZE;

/// Errors possible when setting a banner
#[derive(Debug, thiserror::Error)]
pub enum SetBannerError {
    #[error(transparent)]
    Transport(#[from] reqwest::Error),

    #[error(transparent)]
    DiscordApi(#[from] serenity_prelude::Error),

    #[error("Could not pick a url. Rng failed")]
    CouldNotPickAUrl,

    #[error("Could not determin file extenstion on image")]
    CouldNotDeterminFileExtension,

    #[error("Missing 'banner' feature")]
    MissingBannerFeature,

    #[error("Missing 'animated banner' feature: {}", .0)]
    MissingAnimatedBannerFeature(Url),

    #[error("Image is empty: {}", .0)]
    ImageIsEmpty(Url),

    #[error("Image is to big: {}", .0)]
    ImageIsTooBig(Url),

    #[error("Image size not provided by discord: {}", .0)]
    ImageUnkownSize(Url),
}

/// Trait for setting a banner from an url
pub(crate) trait BannerFromUrl {
    /// Given an [Url](Url) to an image, set the guild banner
    /// This will download the image into memory,
    /// convert the bytes to base64 and then send it to discord
    async fn set_banner_from_url(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'static,
        reqw_client: &Client,
        url: &Url,
    ) -> Result<(), SetBannerError>;
}

impl BannerFromUrl for GuildId {
    #[instrument(skip_all)]
    async fn set_banner_from_url(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'static,
        reqw_client: &Client,
        url: &Url,
    ) -> Result<(), SetBannerError> {
        let extension = url
            .path()
            .split('.')
            .last()
            .ok_or_else(|| SetBannerError::CouldNotDeterminFileExtension)?;

        debug!("Found extension: {extension}");
        // Disable banner feature check when in dev environment
        #[cfg(not(feature = "dev"))]
        {
            use serenity_prelude::small_fixed_array::FixedString;

            let guild = self.to_partial_guild(http.as_ref()).await?;
            let features = guild.features;

            if !features.contains(&FixedString::from_static_trunc("BANNER")) {
                return Err(SetBannerError::MissingBannerFeature);
            }

            if extension.to_lowercase() == "gif"
                && !features.contains(&FixedString::from_static_trunc("ANIMATED_BANNER"))
            {
                return Err(SetBannerError::MissingAnimatedBannerFeature(url.clone()));
            }
        }

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
            .append_pair("width", "960")
            .append_pair("height", "540")
            .finish();

        debug!("requesting image");
        let response = reqw_client.get(url.as_ref()).send().await?;

        debug!("checking image size");
        // check content length header
        let estimated_content_length = match response
            .content_length()
            .map(|len| usize::try_from(len).unwrap_or(usize::MAX))
        {
            Some(0) => return Err(SetBannerError::ImageIsEmpty(url.clone())),
            Some(MAXIMUM_IMAGE_SIZE..) => return Err(SetBannerError::ImageIsTooBig(url.clone())),
            None => return Err(SetBannerError::ImageUnkownSize(url.clone())),
            Some(len) => len,
        };

        // Use stream to get image bytes because if using response.bytes()
        // there would be a risk of downloading huuuuuge files into RAM if for example
        // the content_length header would be spoofed.
        // To give me some headroom

        debug!("fetching image");
        let (image_bytes, _): (Vec<u8>, Url) = response
            .bytes_stream()
            .map_err(SetBannerError::Transport)
            .try_fold(
                (Vec::<u8>::with_capacity(estimated_content_length), url.clone()),
                |(mut acc, url), value: Bytes| async move {
                    if acc.len() + value.len() > MAXIMUM_IMAGE_SIZE {
                        return Err(SetBannerError::ImageIsTooBig(url));
                    }

                    acc.extend_from_slice(&value);
                    Ok((acc, url))
                },
            )
            .await?;

        // check actual content length
        match image_bytes.len() {
            0 => return Err(SetBannerError::ImageIsEmpty(url)),
            MAXIMUM_IMAGE_SIZE.. => return Err(SetBannerError::ImageIsTooBig(url)),
            _ => {}
        }

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

pub(crate) trait RandomBanner: BannerFromUrl {
    /// Given a slice of [Url](Url), pick a random entry
    /// and try and set it as the guild banner
    ///
    /// Returns Ok(url) with the url being choosen
    #[instrument(skip_all)]
    async fn set_random_banner<'url>(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'static,
        reqw_client: &Client,
        urls: &'url [Url],
    ) -> Result<&'url Url, SetBannerError> {
        let url = urls
            .choose(&mut rand::thread_rng())
            .ok_or(SetBannerError::CouldNotPickAUrl)?;

        self.set_banner_from_url(http, reqw_client, url).await?;

        Ok(url)
    }
}

impl<T> RandomBanner for T where T: BannerFromUrl {}
