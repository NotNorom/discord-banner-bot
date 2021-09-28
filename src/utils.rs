use poise::serenity_prelude::{GuildId, Http};
use url::Url;

use crate::Error;

pub(crate) async fn set_random_image_for_guild(
    _http: &Http,
    _guild_id: &GuildId,
    _album: &Url,
) -> Result<(), Error> {
    println!("Would download an image for {} from {}", _guild_id, _album);
    //let image = todo!("image downloading");
    //guild_id.edit(&http, |g| g.icon(Some(image))).await?;

    Ok(())
}
