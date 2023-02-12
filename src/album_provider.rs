//! This module is for different providers of albums.
//!
//! An album is a collection of images that can be choosen from.
//! A provider is a service like imgur, google drive, dropbox, or even just a folder on the local disc that
//! can _provide_ an album.

use std::{convert::TryFrom, fmt::Display, time::Duration};

use anyhow::anyhow;
use imgurs::ImgurClient;
use reqwest::Url;
use tokio::time::sleep;

mod imgur_album;

use crate::{settings, Error};

/// Wrapper for all the different providers
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Providers {
    clients: ProviderClients,
}

impl Providers {
    /// Create new Providers collection
    pub fn new(settings: &settings::Provider, http: &reqwest::Client) -> Self {
        Self {
            clients: ProviderClients::new(settings, http),
        }
    }

    /// Return a list of images from the provider given the [album](Url)
    ///
    /// This function has retry logic
    pub async fn images(&self, album: &Album) -> Result<Vec<Url>, Error> {
        use ProviderKind::*;

        let image_getter = Box::new(|| match album.provider_kind {
            Imgur => self.imgur(&album.url),
        });

        // fuck bounds checking on plain old arrays, I am using an iterator!
        let mut attempt_timeouts = [100, 250, 750, 1500, 2500].into_iter();

        loop {
            match image_getter().await {
                Ok(images) => return Ok(images),
                Err(err) => match attempt_timeouts.next() {
                    Some(timeout) => sleep(Duration::from_millis(timeout)).await,
                    None => return Err(err), // return error when we have run out of retries
                },
            };
        }
    }
}

/// Contains all available providers
#[derive(Debug, Clone)]
struct ProviderClients {
    imgur: ImgurClient,
}

impl ProviderClients {
    fn new(settings: &settings::Provider, http: &reqwest::Client) -> Self {
        let imgur = ImgurClient::with_http_client(&settings.imgur.client_id, http.clone());

        Self { imgur }
    }
}

/// Used to select which provider to use
#[non_exhaustive]
#[derive(Debug, Clone)]
enum ProviderKind {
    Imgur,
}

impl TryFrom<&Url> for ProviderKind {
    type Error = Error;

    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let domain = url
            .domain()
            .ok_or_else(|| anyhow!("Must be domain, not IP address"))?;
        match domain {
            "imgur.com" => Ok(Self::Imgur),
            _ => Err(Error::UnsupportedProvider(domain.to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Album {
    url: Url,
    provider_kind: ProviderKind,
}

impl Album {
    pub fn url(&self) -> &Url {
        &self.url
    }
}

impl TryFrom<&Url> for Album {
    type Error = Error;

    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let kind = url.try_into()?;

        Ok(Self {
            url: url.clone(),
            provider_kind: kind,
        })
    }
}

impl Display for Album {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}
