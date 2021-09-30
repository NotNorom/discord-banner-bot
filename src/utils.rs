use std::str::FromStr;

use poise::{
    serde_json::Value,
    serenity_prelude::{GuildId, Http},
};
use rand::prelude::*;
use reqwest::{Client, Method};
use url::Url;

use crate::Error;

pub async fn set_random_image_for_guild(
    http: &Http,
    reqw_client: &Client,
    guild_id: &mut GuildId,
    album: &Url,
) -> Result<(), Error> {
    println!("Would download an image for {} from {}", guild_id, album);
    //let image = todo!("image downloading");
    //guild_id.edit(&http, |g| g.icon(Some(image))).await?;
    let image_urls = get_images_from_imgur_album(reqw_client, album).await?;

    let url = image_urls
        .choose(&mut rand::thread_rng())
        .ok_or("Could not pick a url")?;
    let extension = url
        .as_str()
        .split(".")
        .last()
        .ok_or("No file extension on image url")?;

    let image_bytes = reqw_client.get(url.as_ref()).send().await?.bytes().await?;
    let b64 = base64::encode(&image_bytes);

    guild_id
        .edit(&http, |g| {
            g.icon(Some(&format!("data:image/{};base64,{}", extension, b64)))
        })
        .await?;

    Ok(())
}

/// Enter imgur album url, get back links to all the images
pub async fn get_images_from_imgur_album(client: &Client, album: &Url) -> Result<Vec<Url>, Error> {
    let imgur_client_id = dotenv::var("IMGUR_CLIENT_ID")?;
    let album_hash = extract_album_hash(&album).ok_or("No album hash found")?;
    let response = client
        .request(
            Method::GET,
            format!("https://api.imgur.com/3/album/{}/images", album_hash),
        )
        .header("Authorization", format!("Client-ID {}", imgur_client_id))
        .send()
        .await?;

    let json = response.json::<Value>().await?;
    let images: Vec<_> = json
        .get("data")
        .ok_or("Json has no data field")?
        .as_array()
        .ok_or("Data field is not an array")?
        .into_iter()
        .filter_map(|obj| obj.get("link"))
        .filter_map(|value| value.as_str())
        .filter_map(|link| Url::from_str(link).ok())
        .collect();

    println!("{:#?}", json);

    Ok(images)
}

/// Given an imgur link like https://imgur.com/a/YM1yHhx, return just the YM1yHhx part.
fn extract_album_hash<'a>(album: &'a Url) -> Option<&'a str> {
    Some(album.path_segments()?.nth(1)?)
}
