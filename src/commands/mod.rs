pub mod help;

use std::convert::TryFrom;

use poise::serenity_prelude::GuildId;
use url::Url;

use crate::{
    album_provider::Provider,
    constants::{DEFAULT_INTERVAL, MINIMUM_INTERVAL},
    Context, Error,
};

/// Picks a random image from the album every n minutes and sets it as the banner.
#[poise::command(prefix_command, slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Imgur album"]
    album: String,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
) -> Result<(), Error> {
    // guild id
    let guild_id = ctx.guild_id().ok_or("Command must be run in server")?;

    // interval
    let interval = interval.unwrap_or(DEFAULT_INTERVAL);
    if interval < MINIMUM_INTERVAL {
        return Err(format!("Interval must be at least {} minutes", MINIMUM_INTERVAL).into());
    }

    // album url
    let album = album.parse::<Url>()?;

    let provider = Provider::try_from(&album)?;

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
    // interval is in minutes, so we multiply by 60 seconds
    user_data
        .enque(guild_id, album, provider, interval * 60, None)
        .await?;

    Ok(())
}

/// Stops picking random images
#[poise::command(prefix_command, slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Command must be run in server")?;

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
    let guild_id = ctx.guild_id().ok_or("Command must be run in server")?;

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

/// Picks a random image from the album every n minutes and sets it as the banner for the guild
/// with the given id.
#[poise::command(prefix_command, slash_command, hide_in_help, owners_only)]
pub async fn start_for_guild(ctx: Context<'_>,
    #[description = "Guild ID"]
    guild_id: u64,
    #[description = "Imgur album"]
    album: String,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
) -> Result<(), Error> {
    // guild id
    let guild_id = GuildId(guild_id);

    // interval
    let interval = interval.unwrap_or(DEFAULT_INTERVAL);
    if interval < MINIMUM_INTERVAL {
        return Err("Interval must be at least 15 minutes".into());
    }

    // album url
    let album = album.parse::<Url>()?;

    let provider = Provider::try_from(&album)?;

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
    // interval is in minutes, so we multiply by 60 seconds
    user_data
        .enque(guild_id, album, provider, interval * 60, None)
        .await?;

    Ok(())
}
