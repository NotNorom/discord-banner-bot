use poise;
use url::Url;

use crate::{Context, Error};

/// Picks a random image from the album every n minutes and sets it as the banner.
#[poise::command(prefix_command, slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Imgur album"] album: String,
    #[description = "After how many minutes the image should change. Default is 30 minutes"]
    interval: Option<u64>,
) -> Result<(), Error> {
    // guild id
    let guild_id = ctx.guild_id().ok_or("No guild id available")?;

    // interval
    let interval = interval.unwrap_or(30);
    if interval < 15 {
        return Err("Interval must be at least 15 minutes")?;
    }

    // album url
    let album = album.parse::<Url>()?;

    // debug
    poise::say_reply(
        ctx,
        format!(
            "Album: {:?}, Interval: {:?}, Guild ID: {:?}",
            &album, &interval, &guild_id
        ),
    )
    .await?;

    let user_data = ctx.data();
    user_data.enque(guild_id, album, interval).await?;

    Ok(())
}

/// Stops picking random images
#[poise::command(prefix_command, slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("No guild id available")?;

    poise::say_reply(ctx, format!("Stopped")).await?;

    let user_data = ctx.data();
    user_data.deque(guild_id).await?;

    Ok(())
}

/// Tells you the album that is being used right now
#[poise::command(prefix_command, slash_command)]
pub async fn album(ctx: Context<'_>) -> Result<(), Error> {
    poise::say_reply(ctx, format!("insert album link here")).await?;

    Ok(())
}

/// Link to the current image
#[poise::command(prefix_command, slash_command)]
pub async fn current(ctx: Context<'_>) -> Result<(), Error> {
    poise::say_reply(ctx, format!("insert current image link here")).await?;

    Ok(())
}
