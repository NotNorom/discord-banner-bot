use poise::serenity_prelude::{GuildId, Http};
use reqwest::Client;
use url::Url;

use crate::{
    album_provider::{ImgurAlbum, Provider},
    Error,
};

// Selects a random image from an imgur album as the server banner
pub async fn set_random_banner_for_guild(
    http: &Http,
    reqw_client: &Client,
    guild_id: &mut GuildId,
    album: &Url,
) -> Result<(), Error> {
    let imgur_client_id = dotenv::var("IMGUR_CLIENT_ID")?;

    // Select the provider according to the album url the user passed to us
    let provider: &dyn Provider = { &mut ImgurAlbum::new(reqw_client, &imgur_client_id) };
    let url = provider.random_entry(album).await?;

    // encode the image data as b64
    let extension = url
        .as_str()
        .split('.')
        .last()
        .ok_or("No file extension on image url")?;

    let image_bytes = reqw_client.get(url.as_ref()).send().await?.bytes().await?;
    let b64 = base64::encode(&image_bytes);

    // set the guild banner using the base64 image data
    guild_id
        .edit(&http, |g| {
            g.banner(Some(&format!("data:image/{};base64,{}", extension, b64)))
        })
        .await?;

    Ok(())
}
