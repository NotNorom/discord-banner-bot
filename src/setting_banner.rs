//! This module is for extending the [GuildId](GuildId) struct
//! with functions for setting the banner from an URL.

use std::collections::HashMap;

use base64::{Engine, prelude::BASE64_STANDARD};
use bytes::Bytes;
use poise::serenity_prelude::{self, EditGuild, GuildId, Http, ImageData, Message, futures::TryStreamExt};
use rand::seq::IndexedRandom;
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
    CouldNotDeterminFileExtension(Url),

    #[error("Missing 'banner' feature")]
    MissingBannerFeature,

    #[error("Missing 'animated banner' feature: {} on message: {}", .0, .1.link())]
    MissingAnimatedBannerFeature(Url, Box<Message>),

    #[error("Image is empty: {} on message: {}", .0, .1.link())]
    ImageIsEmpty(Url, Box<Message>),

    #[error("Image is to big: {} on message: {}", .0, .1.link())]
    ImageIsTooBig(Url, Box<Message>),

    #[error("Image size not provided by discord: {} on message: {}", .0, .1.link())]
    ImageUnkownSize(Url, Box<Message>),

    #[error("Could not encode image to base64: {} on message: {}", .0, .1.link())]
    Base64Encoding(Url, Box<Message>),
}

/// Trait for setting a banner from an url
pub(crate) trait BannerFromUrl {
    /// Given an [Url](Url) to an image, set the guild banner
    /// This will download the image into memory,
    /// convert the bytes to base64 and then send it to discord
    async fn set_banner_from_url_and_message(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'static,
        reqw_client: &Client,
        url: &Url,
        message: &Message,
    ) -> Result<(), SetBannerError>;
}

impl BannerFromUrl for GuildId {
    #[instrument(skip_all)]
    async fn set_banner_from_url_and_message(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'static,
        reqw_client: &Client,
        url: &Url,
        message: &Message,
    ) -> Result<(), SetBannerError> {
        let extension = url
            .path()
            .split('.')
            .next_back()
            .ok_or_else(|| SetBannerError::CouldNotDeterminFileExtension(url.clone()))?;

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
                return Err(SetBannerError::MissingAnimatedBannerFeature(
                    url.clone(),
                    Box::new(message.clone()),
                ));
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
            Some(0) => {
                return Err(SetBannerError::ImageIsEmpty(
                    url.clone(),
                    Box::new(message.clone()),
                ));
            }
            Some(MAXIMUM_IMAGE_SIZE..) => {
                return Err(SetBannerError::ImageIsTooBig(
                    url.clone(),
                    Box::new(message.clone()),
                ));
            }
            // instead of failing, return the maximum size in hopes of it working out.
            // worst case, we've just allocated a few mb of memory that won't be used... oh well
            None => MAXIMUM_IMAGE_SIZE,
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
                        return Err(SetBannerError::ImageIsTooBig(url, Box::new(message.clone())));
                    }

                    acc.extend_from_slice(&value);
                    Ok((acc, url))
                },
            )
            .await?;

        // check actual content length
        match image_bytes.len() {
            0 => return Err(SetBannerError::ImageIsEmpty(url, Box::new(message.clone()))),
            MAXIMUM_IMAGE_SIZE.. => {
                return Err(SetBannerError::ImageIsTooBig(url, Box::new(message.clone())));
            }
            _ => {}
        }

        let b64_encoded_bytes = BASE64_STANDARD.encode(image_bytes);

        let image = ImageData::from_base64(format!("data:image/{extension};base64,{b64_encoded_bytes}"))
            .map_err(|_| SetBannerError::Base64Encoding(url.clone(), Box::new(message.clone())))?;

        let edit_guild = {
            #[cfg(feature = "dev")]
            {
                debug!("Setting icon");
                EditGuild::new().icon(Some(image))
            }

            #[cfg(not(feature = "dev"))]
            {
                debug!("Setting banner");
                EditGuild::new().banner(Some(image))
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
    async fn set_random_banner_with_message<'url>(
        &mut self,
        http: impl AsRef<Http> + Sync + Send + 'static,
        reqw_client: &Client,
        urls: &'url [(Url, Message)],
    ) -> Result<&'url Url, SetBannerError> {
        let (url, message) = urls
            .choose(&mut rand::rng())
            .ok_or(SetBannerError::CouldNotPickAUrl)?;

        self.set_banner_from_url_and_message(http, reqw_client, url, message)
            .await?;

        Ok(url)
    }
}

impl<T> RandomBanner for T where T: BannerFromUrl {}
