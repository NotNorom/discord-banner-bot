use std::num::NonZeroUsize;

use chrono::{DateTime, Utc};
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
    utils::current_unix_timestamp,
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
    #[description = "When to start the schedule. Default is instantly."] start_at: Option<DateTime<Utc>>,
    #[description = "How many messages to look back for images. Default is 100, maximum is 200"]
    message_limit: Option<usize>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;
    let options = StartBannerOptions::new(guild_id, channel_id)
        .interval(interval)?
        .start_at(start_at)?
        .message_limit(message_limit)?;
    start_banner(ctx, options).await
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
    #[description = "When to start the schedule. Default is instantly."] start_at: Option<DateTime<Utc>>,
    #[description = "How many messages to look back for images. Default is 100, maximum is 200"]
    message_limit: Option<usize>,
) -> Result<(), Error> {
    let options = StartBannerOptions::new(guild_id, channel_id)
        .interval(interval)?
        .start_at(start_at)?
        .message_limit(message_limit)?;

    start_banner(ctx, options).await
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

struct StartBannerOptions {
    guild_id: GuildId,
    channel_id: ChannelId,
    interval: u64,
    start_at: Option<DateTime<Utc>>,
    message_limit: usize,
}

impl StartBannerOptions {
    pub fn new(guild_id: GuildId, channel_id: ChannelId) -> Self {
        Self {
            guild_id,
            channel_id,
            interval: 15,
            start_at: None,
            message_limit: 200,
        }
    }

    pub fn interval(mut self, interval: Option<u64>) -> Result<Self, Error> {
        let interval = interval.unwrap_or(DEFAULT_INTERVAL);
        if interval < MINIMUM_INTERVAL {
            return Err(CommandErr::BelowMinTimeout.into());
        }

        if interval > MAXIMUM_INTERVAL {
            return Err(CommandErr::AboveMaxTimeout.into());
        }

        self.interval = interval;
        Ok(self)
    }

    pub fn start_at(mut self, start_at: Option<DateTime<Utc>>) -> Result<Self, Error> {
        let Some(start_at) = start_at else {
            return Ok(self);
        };

        let now = Utc::now();
        let in_the_past = start_at < now;

        if in_the_past {
            return Err(CommandErr::StartTimeInThePast { now, given: start_at }.into());
        }

        self.start_at = Some(start_at);
        Ok(self)
    }

    pub fn message_limit(mut self, message_limit: Option<usize>) -> Result<Self, Error> {
        let message_limit = message_limit.unwrap_or(DEFAULT_MESSAGE_LIMIT);
        if message_limit > MAXIMUM_MESSAGE_LIMIT {
            return Err(CommandErr::AboveMaxMessageLimit.into());
        }

        self.message_limit = message_limit;
        Ok(self)
    }
}

async fn start_banner(ctx: Context<'_>, options: StartBannerOptions) -> Result<(), Error> {
    let StartBannerOptions {
        guild_id,
        channel_id,
        interval,
        start_at,
        message_limit,
    } = options;

    // disable BANNER check when dev feature is enabled
    #[cfg(not(feature = "dev"))]
    {
        use poise::serenity_prelude::small_fixed_array::FixedString;
        let guild = guild_id.to_partial_guild(ctx.http()).await?;
        if !guild.features.contains(&FixedString::from_static_trunc("BANNER")) {
            return Err(CommandErr::GuildHasNoBannerFeature.into());
        }
    }

    let user_data = ctx.data();

    let now = current_unix_timestamp();
    let start_at = start_at.map(|s| s.timestamp() as u64).unwrap_or(now);
    let offset_from_now = start_at - now;

    // schedule it
    // interval is in minutes, so we multiply by 60 seconds
    let schedule_builder = ScheduleBuilder::new(guild_id, channel_id, interval * 60)
        .message_limit(message_limit)
        .start_at(start_at);

    user_data.enque(schedule_builder.build()).await?;

    let content = MessageBuilder::new()
        .push(&*format!(
            "Scheduling banner change for every {interval} minutes using channel "
        ))
        .channel(channel_id)
        .push(&*format!(
            ". Starting at {start_at} in {offset_from_now} seconds."
        ))
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
