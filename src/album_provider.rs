//! This module is for different providers of albums.
//!
//! An album is a collection of images that can be choosen from.
//! A provider is a service like imgur, google drive, dropbox, or even just a folder on the local disc that
//! can _provide_ an album.

use std::{collections::HashMap, convert::TryFrom, fmt::Display, time::Duration};

use anyhow::anyhow;
use imgurs::ImgurClient;
use poise::async_trait;
use reqwest::Url;
use tokio::time::sleep;
use tracing::{debug, instrument};

mod imgur_album;

use crate::{settings, Error};

use self::imgur_album::Imgur;

#[async_trait]
trait Provider: Send + Sync {
    async fn provide(&self, album: &Url) -> Result<Vec<Url>, Error>;
}

/// Wrapper for all the different providers
pub struct Providers {
    clients: HashMap<ProviderKind, Box<dyn Provider>>,
}

impl Providers {
    /// Create new Providers collection
    pub fn new(settings: &settings::Provider, http: &reqwest::Client) -> Self {
        let mut clients = HashMap::new();

        if let Some(imgur_settings) = &settings.imgur {
            let imgur_client = ImgurClient::with_http_client(&imgur_settings.client_id, http.clone());
            let imgur_provider = Imgur::new(imgur_client);

            clients.insert(ProviderKind::Imgur, Box::new(imgur_provider) as Box<dyn Provider>);
        };

        Self { clients }
    }

    /// Return a list of images from the provider given the [album](Url)
    ///
    /// This function has retry logic
    #[instrument(skip(self))]
    pub async fn images(&self, album: &Album) -> Result<Vec<Url>, Error> {
        let image_getter = self
            .clients
            .get(&album.kind)
            .ok_or(Error::InactiveProvider(album.kind))?;

        // fuck bounds checking on plain old arrays, I am using an iterator!
        let mut attempt_timeouts = [100, 250, 750, 1500, 2500].into_iter();

        let mut attempt = 1;

        loop {
            debug!("Attempt {attempt}");
            attempt += 1;

            match image_getter.provide(&album.url).await {
                Ok(images) => return Ok(images),
                Err(err) => match attempt_timeouts.next() {
                    Some(timeout) => {
                        debug!("Fail. Trying different timeout: {timeout}ms");
                        sleep(Duration::from_millis(timeout)).await
                    },
                    None => {
                        debug!("Final fail. Out of retries");
                        return Err(err); // return last error when we have run out of retries
                    }
                },
            };
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum ProviderKind {
    Imgur,
    GoogleDriveFolder,
    LocalFilesystem,
}

impl TryFrom<&Url> for ProviderKind {
    type Error = Error;

    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let domain = url
            .domain()
            .ok_or_else(|| anyhow!("Must be a domain, not an IP address"))?;

        Ok(match domain {
            "imgur.com" => Self::Imgur,
            _ => Err(Error::UnsupportedProvider(domain.to_owned()))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Album {
    url: Url,
    kind: ProviderKind,
}

impl Album {
    pub fn new(url: Url, kind: ProviderKind) -> Self {
        Self { url, kind }
    }

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
            kind,
        })
    }
}

impl Display for Album {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}
