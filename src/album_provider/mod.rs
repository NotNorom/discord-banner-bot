use std::convert::TryFrom;

use reqwest::{Client, Url};

mod imgur_album;

use crate::Error;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Provider {
    /// Provider for an Imgur album like "https://imgur.com/a/YM1yHhx"
    Imgur { client_id: String },
}

impl Provider {
    pub async fn images(&self, reqw_client: &Client, album: &Url) -> Result<Vec<Url>, Error> {
        match self {
            Provider::Imgur { client_id } => self.images_imgur(client_id, reqw_client, album).await,
        }
    }
}

impl TryFrom<&Url> for Provider {
    type Error = Error;
    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let domain = url.domain().ok_or("Must be domain, not IP address")?;
        match domain {
            "imgur.com" => {
                let client_id = dotenv::var("IMGUR_CLIENT_ID")?;
                Ok(Self::Imgur { client_id })
            }
            _ => Err("Unsupported provider domain".into()),
        }
    }
}
