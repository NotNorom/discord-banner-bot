use std::{num::NonZeroUsize, time::Duration};

use poise::{
    serenity_prelude::{ChannelId, CreateEmbed, EmbedMessageBuilding, GuildId, MessageBuilder},
    CreateReply,
};

use crate::{
    constants::{
        DEFAULT_INTERVAL, DEFAULT_MESSAGE_LIMIT, MAXIMUM_INTERVAL, MAXIMUM_MESSAGE_LIMIT, MINIMUM_INTERVAL,
    },
    error::Command as CommandErr,
    finding_media::last_reachable_message,
    schedule::ScheduleBuilder,
    Context, Error,
};

/// Picks a random image from the channel every interval minutes and sets it as the banner.
#[poise::command(
    prefix_command,
    slash_command,
    required_bot_permissions = "MANAGE_GUILD | VIEW_CHANNEL | READ_MESSAGE_HISTORY | SEND_MESSAGES | SEND_MESSAGES_IN_THREADS",
    required_permissions = "MANAGE_GUILD",
    default_member_permissions = "MANAGE_GUILD",
    guild_only
)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Channel"]
    #[rename = "channel"]
    channel_id: ChannelId,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
    #[description = "How many messages to look back for images. Default is 100, maximum is 200"]
    message_limit: Option<usize>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;
    start_banner(ctx, guild_id, channel_id, interval, message_limit).await
}

/// Picks a random image from the channel every n minutes and sets it as the banner for that server.
#[poise::command(prefix_command, slash_command, hide_in_help, owners_only)]
pub async fn start_for_guild(
    ctx: Context<'_>,
    #[description = "Guild ID"]
    #[rename = "guild"]
    guild_id: GuildId,
    #[description = "Channel"]
    #[rename = "channel"]
    channel_id: ChannelId,
    #[description = "After how many minutes the image should change. Default is 30, minimum 15."]
    interval: Option<u64>,
    #[description = "How many messages to look back for images. Default is 100, maximum is 200"]
    message_limit: Option<usize>,
) -> Result<(), Error> {
    start_banner(ctx, guild_id, channel_id, interval, message_limit).await
}

/// Stops picking random images
#[poise::command(
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES | SEND_MESSAGES_IN_THREADS",
    required_permissions = "MANAGE_GUILD",
    default_member_permissions = "MANAGE_GUILD"
)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;
    stop_banner(ctx, guild_id).await
}

/// Stops picking random images in that server
#[poise::command(prefix_command, slash_command, hide_in_help, owners_only)]
pub async fn stop_for_guild(
    ctx: Context<'_>,
    #[description = "Guild ID"]
    #[rename = "guild"]
    guild_id: GuildId,
) -> Result<(), Error> {
    stop_banner(ctx, guild_id).await
}

/// Tells you the channel that is being used right now
#[poise::command(
    prefix_command,
    slash_command,
    required_bot_permissions = "SEND_MESSAGES | SEND_MESSAGES_IN_THREADS",
    required_permissions = "MANAGE_GUILD",
    default_member_permissions = "MANAGE_GUILD",
    guild_only
)]
pub async fn current_schedule(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;

    let user_data = ctx.data();
    let schedule = user_data.get_schedule(guild_id).await?;

    let message_limit = schedule
        .message_limit()
        .map(NonZeroUsize::get)
        .unwrap_or_default();

    let message_builder = MessageBuilder::new()
        .push("Current channel: ")
        .channel(schedule.channel_id())
        .push(format!(". Current message limit: {message_limit}.",).as_str());

    let message = match last_reachable_message(ctx.http(), &schedule).await {
        Some(msg) => message_builder
            .push(" Last reachable message: ")
            .push_named_link("click here", msg.link().as_str())
            .build(),
        None => message_builder.build(),
    };

    poise::send_reply(ctx, CreateReply::default().content(message).ephemeral(true)).await?;

    Ok(())
}

/// Link to the banner that is currently displayed
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn current_banner(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;

    // disable BANNER check when dev feature is enabled
    #[cfg(not(feature = "dev"))]
    {
        use poise::serenity_prelude::small_fixed_array::FixedString;
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

async fn start_banner(
    ctx: Context<'_>,
    guild_id: GuildId,
    channel: ChannelId,
    interval: Option<u64>,
    message_limit: Option<usize>,
) -> Result<(), Error> {
    // disable BANNER check when dev feature is enabled
    #[cfg(not(feature = "dev"))]
    {
        use poise::serenity_prelude::small_fixed_array::FixedString;
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

    // message limit
    let message_limit = message_limit.unwrap_or(DEFAULT_MESSAGE_LIMIT);
    if message_limit > MAXIMUM_MESSAGE_LIMIT {
        return Err(CommandErr::AboveMaxMessageLimit.into());
    }

    let user_data = ctx.data();

    // schedule it
    // interval is in minutes, so we multiply by 60 seconds
    let schedule = ScheduleBuilder::new(guild_id, channel, Duration::from_secs(interval * 60))
        .message_limit(message_limit)
        .build();

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

async fn stop_banner(ctx: Context<'_>, guild_id: GuildId) -> Result<(), Error> {
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
