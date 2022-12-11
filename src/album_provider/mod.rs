//! This module is for different providers of albums.
//!
//! An album is a collection of images that can be choosen from.
//! A provider is a service like imgur, google drive, dropbox, or even just a folder on the local disc that
//! can _provide_ an album.

use std::convert::TryFrom;

use anyhow::anyhow;
use reqwest::{Client, Url};

mod imgur_album;

use crate::Error;

/// This enum differentiates between different providers
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Provider {
    /// An Imgur album like `https://imgur.com/a/YM1yHhx`
    Imgur {
        /// See <https://apidocs.imgur.com/>
        client_id: String,
    },
}

impl Provider {
    /// Return a list of images from the provider given the [album](Url)
    pub async fn images(&self, reqw_client: &Client, album: &Url) -> Result<Vec<Url>, Error> {
        match self {
            Provider::Imgur { client_id } => self.images_imgur(client_id, reqw_client, album).await,
        }
    }
}

/// Try to create a Provider from an url
impl TryFrom<&Url> for Provider {
    type Error = Error;
    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let domain = url.domain().ok_or_else(|| anyhow!("Must be domain, not IP address"))?;
        match domain {
            "imgur.com" => {
                let client_id = dotenv::var("IMGUR_CLIENT_ID")?;
                Ok(Self::Imgur { client_id })
            }
            _ => Err(Error::UnsupportedProvider(domain.to_string())),
        }
    }
}
