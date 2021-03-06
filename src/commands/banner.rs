use poise::serenity_prelude::GuildId;
use reqwest::Url;

use crate::{
    album_provider::Provider,
    constants::{DEFAULT_INTERVAL, MAXIMUM_INTERVAL, MINIMUM_INTERVAL},
    Context, Error,
};

/// Picks a random image from the album every n minutes and sets it as the banner.
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Imgur album"] album: String,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
) -> Result<(), Error> {
    use crate::error::Command::*;

    // guild id
    let guild_id = ctx.guild_id().ok_or(GuildOnly)?;

    // interval
    let interval = interval.unwrap_or(DEFAULT_INTERVAL);
    if interval < MINIMUM_INTERVAL {
        return Err(BelowMinTimeout.into());
    }

    if interval > MAXIMUM_INTERVAL {
        return Err(AboveMaxTimeout.into());
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
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    use crate::error::Command::*;
    let guild_id = ctx.guild_id().ok_or(GuildOnly)?;

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
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn album(ctx: Context<'_>) -> Result<(), Error> {
    use crate::error::Command::*;
    let guild_id = ctx.guild_id().ok_or(GuildOnly)?;

    let user_data = ctx.data();
    let album = user_data.get_album(guild_id).await?;

    poise::send_reply(ctx, |f| f.content(album.to_string()).ephemeral(true)).await?;

    Ok(())
}

/// Link to the current image
#[poise::command(prefix_command, slash_command)]
pub async fn current(ctx: Context<'_>) -> Result<(), Error> {
    use crate::error::Command::*;
    let guild_id = ctx.guild_id().ok_or(GuildOnly)?;

    let guild = guild_id.to_partial_guild(&ctx.discord().http).await?;
    let banner = guild.banner_url().ok_or(GuildHasNoBanner)?;

    // answer the user
    poise::send_reply(ctx, |f| {
        f.content(&banner)
            .ephemeral(true)
            .embed(|e| e.image(&banner).colour((255, 0, 255)))
    })
    .await?;

    Ok(())
}

/// Picks a random image from the album every n minutes and sets it as the banner for that server.
#[poise::command(prefix_command, slash_command, hide_in_help, owners_only)]
pub async fn start_for_guild(
    ctx: Context<'_>,
    #[description = "Guild ID"] guild_id: u64,
    #[description = "Imgur album"] album: String,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
) -> Result<(), Error> {
    use crate::error::Command::*;

    // guild id
    let guild_id = GuildId(guild_id);

    // interval
    let interval = interval.unwrap_or(DEFAULT_INTERVAL);
    if interval < MINIMUM_INTERVAL {
        return Err(BelowMinTimeout.into());
    }

    if interval > MAXIMUM_INTERVAL {
        return Err(AboveMaxTimeout.into());
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
