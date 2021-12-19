use std::convert::TryFrom;

use url::Url;

use crate::{
    album_provider::ProviderKind,
    constants::{DEFAULT_INTERVAL, MINIMUM_INTERVAL},
    Context, Error,
};

/// Picks a random image from the album every n minutes and sets it as the banner.
#[poise::command(prefix_command, slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Imgur album"] album: String,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
) -> Result<(), Error> {
    // guild id
    let guild_id = ctx.guild_id().ok_or("No guild id available")?;

    // interval
    let interval = interval.unwrap_or(DEFAULT_INTERVAL);
    if interval < MINIMUM_INTERVAL {
        return Err(format!("Interval must be at least {} minutes", MINIMUM_INTERVAL).into());
    }

    // album url
    let album = album.parse::<Url>()?;

    let provider = ProviderKind::try_from(&album)?;

    // answer the user
    poise::send_reply(ctx, |f| {
        let content = format!(
            "Scheduling banner change for every {} minutes using this album: <{}>",
            &interval,
            &album.as_str()
        );
        f.content(content).ephemeral(true)
    })
    .await?;

    let user_data = ctx.data();

    // schedule it
    user_data.enque(guild_id, album, interval, provider).await?;

    Ok(())
}

/// Stops picking random images
#[poise::command(prefix_command, slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("No guild id available")?;

    // answer the user
    poise::send_reply(ctx, |f| {
        f.content("Stopped currently running timer").ephemeral(true)
    })
    .await?;

    // unschedule it!
    let user_data = ctx.data();
    user_data.deque(guild_id).await?;

    Ok(())
}

/// Tells you the album that is being used right now
#[poise::command(prefix_command, slash_command)]
pub async fn album(ctx: Context<'_>) -> Result<(), Error> {
    poise::say_reply(ctx, "This command is work in progress").await?;

    Ok(())
}

/// Link to the current image
#[poise::command(prefix_command, slash_command)]
pub async fn current(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("No guild id available")?;

    let guild = guild_id.to_partial_guild(&ctx.discord().http).await?;
    let banner = guild.banner_url().ok_or("Guild has no banner")?;

    // answer the user
    poise::send_reply(ctx, |f| {
        f.content(&banner)
            .ephemeral(true)
            .embed(|e| e.image(&banner).colour((255, 0, 255)))
    })
    .await?;

    Ok(())
}
