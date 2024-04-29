use std::time::Duration;

use poise::{
    serenity_prelude::{self, ChannelId, CreateEmbed, GuildId, MessageBuilder},
    CreateReply,
};

use crate::{
    constants::{DEFAULT_INTERVAL, MAXIMUM_INTERVAL, MINIMUM_INTERVAL},
    error::Command as CommandErr,
    schedule::Schedule,
    Context, Error,
};

/// Picks a random image from the channel every interval minutes and sets it as the banner.
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Channel"] channel: ChannelId,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;
    do_the_banner_thing(ctx, guild_id, channel, interval).await
}

/// Stops picking random images
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;

    // unschedule it!
    let user_data = ctx.data();
    user_data.deque(guild_id).await?;

    // answer the user
    poise::send_reply(
        ctx,
        CreateReply::default()
            .content("Stopped currently running schedule")
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Tells you the channel that is being used right now
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn channel(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;

    let user_data = ctx.data();
    let channel = user_data.get_channel(guild_id).await?;

    let message = MessageBuilder::new().channel(channel).build();

    poise::send_reply(ctx, CreateReply::default().content(message).ephemeral(true)).await?;

    Ok(())
}

/// Link to the banner that is currently displayed
#[poise::command(prefix_command, slash_command)]
pub async fn current(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;

    // disable BANNER check when dev feature is enabled
    #[cfg(not(feature = "dev"))]
    {
        use serenity_prelude::small_fixed_array::FixedString;
        let guild = guild_id.to_partial_guild(ctx.http()).await?;
        if !guild.features.contains(&FixedString::from_static_trunc("BANNER")) {
            return Err(CommandErr::GuildHasNoBannerFeature.into());
        }
    }

    let guild = guild_id.to_partial_guild(&ctx).await?;
    let banner = guild.banner_url().ok_or(CommandErr::GuildHasNoBannerSet)?;

    // answer the user
    poise::send_reply(
        ctx,
        CreateReply::default()
            .content(&banner)
            .ephemeral(true)
            .embed(CreateEmbed::new().image(&banner).colour((255, 0, 255))),
    )
    .await?;

    Ok(())
}

/// Picks a random image from the channel every n minutes and sets it as the banner for that server.
#[poise::command(prefix_command, slash_command, hide_in_help, owners_only)]
pub async fn start_for_guild(
    ctx: Context<'_>,
    #[description = "Guild ID"] guild_id: serenity_prelude::Guild,
    #[description = "Channel"] channel: ChannelId,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
) -> Result<(), Error> {
    do_the_banner_thing(ctx, guild_id.id, channel, interval).await
}

async fn do_the_banner_thing(
    ctx: Context<'_>,
    guild_id: GuildId,
    channel: ChannelId,
    interval: Option<u64>,
) -> Result<(), Error> {
    // disable BANNER check when dev feature is enabled
    #[cfg(not(feature = "dev"))]
    {
        use serenity_prelude::small_fixed_array::FixedString;
        let guild = guild_id.to_partial_guild(ctx.http()).await?;
        if !guild.features.contains(&FixedString::from_static_trunc("BANNER")) {
            return Err(CommandErr::GuildHasNoBannerFeature.into());
        }
    }

    // interval
    let interval = interval.unwrap_or(DEFAULT_INTERVAL);
    if interval < MINIMUM_INTERVAL {
        return Err(CommandErr::BelowMinTimeout.into());
    }

    if interval > MAXIMUM_INTERVAL {
        return Err(CommandErr::AboveMaxTimeout.into());
    }

    let user_data = ctx.data();

    // schedule it
    // interval is in minutes, so we multiply by 60 seconds
    let schedule = Schedule::new(Duration::from_secs(interval * 60), guild_id, channel);
    user_data.enque(schedule).await?;

    let content = MessageBuilder::new()
        .push(&*format!(
            "Scheduling banner change for every {interval} minutes using channel "
        ))
        .channel(channel)
        .build();

    // answer the user
    poise::send_reply(ctx, CreateReply::default().content(content).ephemeral(true)).await?;

    Ok(())
}
