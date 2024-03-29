//! This module is for different providers of albums.
//!
//! An album is a collection of images that can be choosen from.
//! A provider is a service like imgur, google drive, dropbox, or even just a folder on the local disc that
//! can _provide_ an album.

use std::{
    collections::HashMap,
    convert::TryFrom,
    fmt::{Debug, Display},
    time::Duration,
};

use imgurs::ImgurClient;
use poise::async_trait;
use reqwest::Url;
use tokio::time::sleep;
use tracing::{debug, info, instrument};

mod imgur_album;

use crate::{settings, Error};

use self::imgur_album::Imgur;

#[async_trait]
trait Provider: Send + Sync + Debug {
    async fn provide(&self, album: &Url) -> Result<Vec<Url>, ProviderError>;
}

/// Wrapper for all the different providers
#[derive(Debug)]
pub struct ImageProviders {
    clients: HashMap<ProviderKind, Box<dyn Provider>>,
}

impl ImageProviders {
    /// Create new Providers collection
    pub fn new(settings: &settings::Provider, http: &reqwest::Client) -> Self {
        let mut clients = HashMap::new();

        if let Some(imgur_settings) = &settings.imgur {
            let imgur_client = ImgurClient::with_http_client(&imgur_settings.client_id, http.clone());
            let imgur_provider = Imgur::new(imgur_client);

            clients.insert(ProviderKind::Imgur, Box::new(imgur_provider) as Box<dyn Provider>);
            info!("Providers, Imgur client set up");
        };

        Self { clients }
    }

    /// Return a list of images from the provider given the [album](Url)
    ///
    /// This function has retry logic
    #[instrument(skip_all)]
    pub async fn images(&self, album: &Album) -> Result<Vec<Url>, ProviderError> {
        let image_getter = self
            .clients
            .get(&album.kind)
            .ok_or(ProviderError::Inactive(album.kind))?;

        // fuck bounds checking on plain old arrays, I am using an iterator!
        let mut attempt_timeouts = [100, 250, 750, 1500, 2500].into_iter();

        let mut attempt = 1;

        loop {
            debug!("Attempt {attempt}");
            attempt += 1;

            match image_getter.provide(&album.url).await {
                Ok(images) => {
                    debug!("Success. Provider got back {} images.", images.len());
                    return Ok(images);
                }
                Err(err) => match attempt_timeouts.next() {
                    Some(timeout) => {
                        debug!("Fail. Trying different timeout: {timeout}ms");
                        sleep(Duration::from_millis(timeout)).await
                    }
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
    type Error = ProviderError;

    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let domain = url.domain().ok_or(ProviderError::InvalidDomain(url.to_owned()))?;

        Ok(match domain {
            "imgur.com" => Self::Imgur,
            _ => return Err(ProviderError::Unsupported(domain.to_owned())),
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
        write!(f, "{}", self.url.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Unsupported provider: {0}. For a list of supported providers see /help")]
    Unsupported(String),

    #[error(
        "Inactive provider: {0:?}. Provider is supported but inactive. Please contact the bot owner /help"
    )]
    Inactive(ProviderKind),

    #[error("Extraction of imgur id failed: {0}. Is the url correct?")]
    ImgurIdExtraction(String),

    #[error("Invalid domain")]
    InvalidDomain(Url),

    #[error(transparent)]
    Imgur(#[from] imgurs::Error),
}
