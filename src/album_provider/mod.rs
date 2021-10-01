use poise::async_trait;
use rand::prelude::SliceRandom;
use reqwest::Url;

use crate::Error;

pub mod imgur_album;
pub use imgur_album::ImgurAlbum;

#[async_trait]
pub trait Provider: Sync {
    /// Given a link to the online provider, return all the images
    async fn album(&self, album: &Url) -> Result<Vec<Url>, Error>;

    /// Given a link to the online provider, return a random image
    async fn random_entry<'a>(&self, album: &Url) -> Result<Url, Error> {
        let image_urls = self.album(album).await?;
        image_urls
            .choose(&mut rand::thread_rng())
            .ok_or_else(|| "Could not pick a url".into())
            .map(Clone::clone)
    }
}
