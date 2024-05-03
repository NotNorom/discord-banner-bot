use std::{
    collections::HashSet,
    fmt::{Debug, Display},
    num::NonZeroU16,
    sync::Arc,
};

use async_repeater::RepeaterHandle;
use poise::serenity_prelude::{
    Context, Error as SerenityError, HttpError as SerenityHttpError, MessageBuilder, User, UserId,
};
use reqwest::StatusCode;
use thiserror::Error;
use tracing::{info, warn};

use crate::{
    constants::{MAXIMUM_INTERVAL, MAXIMUM_MESSAGE_LIMIT, MINIMUM_INTERVAL},
    database::{guild_schedule::GuildSchedule, Database},
    schedule::Schedule,
    schedule_runner::{RunnerError, ScheduleAction},
    setting_banner::SetBannerError,
    settings::SettingsError,
    utils::{dm_user, dm_users},
};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Settings(#[from] SettingsError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Serenity(#[from] poise::serenity_prelude::Error),

    #[error(transparent)]
    Redis(#[from] fred::error::RedisError),

    #[error(transparent)]
    Command(#[from] Command),

    #[error("Scheduler Error: {msg:?}. Please contact the developer. See /help")]
    Scheduler { msg: String },

    #[error(transparent)]
    StdFmt(#[from] std::fmt::Error),

    #[error(transparent)]
    SendDm(#[from] SendDm),

    #[error(transparent)]
    SetBanner(#[from] SetBannerError),
}

#[derive(Debug, thiserror::Error)]
pub enum Command {
    #[error("Command must be run in a server")]
    GuildOnly,

    #[error("Interval must be at least {} minutes", MINIMUM_INTERVAL)]
    BelowMinTimeout,

    #[error("Interval must be at most {} minutes", MAXIMUM_INTERVAL)]
    AboveMaxTimeout,

    #[error("Message limit must be greater than 0")]
    MessageLimitIszero,

    #[error("Message limit must be at most {}", MAXIMUM_MESSAGE_LIMIT)]
    AboveMaxMessageLimit,

    #[error("Server doesn't have a banner set")]
    GuildHasNoBannerSet,

    #[error("Server doesn't have the required boost level")]
    GuildHasNoBannerFeature,
}

/// Error when sending direct messages to a user
#[derive(Debug, thiserror::Error)]
pub struct SendDm {
    user: Box<User>,
    kind: SendDmKind,
}

impl SendDm {
    pub fn pseudo_user(user: Box<User>) -> Error {
        Error::SendDm(Self {
            user,
            kind: SendDmKind::PseudoUser,
        })
    }

    pub fn bot_user(user: Box<User>) -> Error {
        Error::SendDm(Self {
            user,
            kind: SendDmKind::BotUser,
        })
    }

    pub fn other(user: Box<User>, msg: &str) -> Error {
        Error::SendDm(Self {
            user,
            kind: SendDmKind::Other(msg.to_owned()),
        })
    }
}

#[derive(Debug)]
pub enum SendDmKind {
    /// User does not actually exist
    PseudoUser,
    /// User is a bot
    BotUser,
    /// Anyting else
    Other(String),
}

impl Display for SendDm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use SendDmKind::*;

        let user_name = &self.user.name;
        let discriminator = self.user.discriminator.map(NonZeroU16::get).unwrap_or_default();
        let user_id = self.user.id;

        write!(f, "Could not send dm to {user_name}#{discriminator}, {user_id}: ")?;

        match self.kind {
            PseudoUser => write!(f, "Pseudo user"),
            BotUser => write!(f, "User is a bot"),
            Other(ref reason) => write!(f, "{reason}"),
        }
    }
}

/// Handles framework related errors.
/// Does __not__ handle scheduler related errors
pub async fn handle_framework_error<U, E>(error: poise::FrameworkError<'_, U, E>) -> Result<(), Error>
where
    U: Send + Sync + 'static,
    E: std::fmt::Display + std::fmt::Debug,
{
    match &error {
        poise::FrameworkError::EventHandler {
            error: event_err,
            event: poise::serenity_prelude::FullEvent::Ready { .. },
            framework,
            ..
        } => {
            tracing::error!("during startup: {event_err:?} - Shutting down!");
            framework.shard_manager().shutdown_all().await;
        }
        _ => poise::builtins::on_error(error).await?,
    }

    Ok(())
}

/// Handle scheduler related errors
///
/// This is a needed as well as the normal error handling in [crate::error::on_error] because
/// the scheduler is running in its own task
#[tracing::instrument(skip(error, ctx, db, repeater_handle, owners))]
pub async fn handle_schedule_error(
    error: &RunnerError,
    ctx: Arc<Context>,
    repeater_handle: RepeaterHandle<Schedule>,
    db: Database,
    owners: HashSet<UserId>,
) -> Result<ScheduleAction, Error> {
    let guild_id = error.schedule().guild_id();

    let guild_name = format!("{guild_id}: {}", guild_id.name(&ctx.cache).unwrap_or_default());

    let message = MessageBuilder::new()
        .push_bold("Error in guild: ")
        .push_mono_line_safe(&*guild_name)
        .push_codeblock(&*error.to_string(), Some("rust"))
        .build();

    dm_users(&ctx, owners.clone(), &message).await?;

    match error.source() {
        Error::Serenity(serenity_error) => match serenity_error {
            SerenityError::Http(http_error) => match http_error {
                SerenityHttpError::UnsuccessfulRequest(error_response) => {
                    match error_response.status_code {
                        StatusCode::FORBIDDEN => {
                            // the bot does not have permissions to change the banner.
                            // remove guild from queue
                            let _ = repeater_handle.remove(guild_id).await;
                            db.delete::<GuildSchedule>(error.schedule().guild_id().get())
                                .await?;
                            warn!("Missing permissions to change banner for {guild_id}. Unscheduling.");
                            return Ok(ScheduleAction::Abort);
                        }
                        StatusCode::NOT_FOUND => {
                            let _ = repeater_handle.remove(guild_id).await;
                            db.delete::<GuildSchedule>(error.schedule().guild_id().get())
                                .await?;
                            warn!("Guild does not exist: {guild_id}. Unscheduling.");
                            return Ok(ScheduleAction::Abort);
                        }
                        StatusCode::GATEWAY_TIMEOUT => {
                            warn!("Gateway timed out. Retrying once.");
                            return Ok(ScheduleAction::RetrySameImage);
                        }
                        _ => tracing::error!("unsuccessful http request: {error_response:?}"),
                    }
                }
                http_err => tracing::error!("unhandled http error: {http_err:?}"),
            },
            serenity_err => tracing::error!("unhandled serenity error: {serenity_err:?}"),
        },
        Error::SetBanner(banner_error) => {
            match banner_error {
                SetBannerError::Transport(err) => {
                    warn!("guild_id={guild_id}: {err}");
                }
                SetBannerError::DiscordApi(discord_err) => {
                    match discord_err {
                        SerenityError::Http(http_err) => match http_err {
                            SerenityHttpError::UnsuccessfulRequest(error_response) => {
                                match error_response.status_code {
                                    StatusCode::FORBIDDEN => {
                                        // the bot does not have permissions to change the banner.
                                        // remove guild from queue
                                        let _ = repeater_handle.remove(guild_id).await;
                                        db.delete::<GuildSchedule>(error.schedule().guild_id().get())
                                            .await?;
                                        warn!("Missing permissions to change banner for {guild_id}. Unscheduling.");
                                        return Ok(ScheduleAction::Abort);
                                    }
                                    StatusCode::NOT_FOUND => {
                                        let _ = repeater_handle.remove(guild_id).await;
                                        db.delete::<GuildSchedule>(error.schedule().guild_id().get())
                                            .await?;
                                        warn!("Guild does not exist: {guild_id}. Unscheduling.");
                                        return Ok(ScheduleAction::Abort);
                                    }
                                    StatusCode::GATEWAY_TIMEOUT => {
                                        warn!("Gateway timed out. Retrying once.");
                                        return Ok(ScheduleAction::RetrySameImage);
                                    }
                                    _ => tracing::error!("unsuccessful http request: {error_response:?}"),
                                }
                            }
                            http_err => tracing::error!("unhandled http error in set_banner: {http_err:?}"),
                        },
                        serenity_err => {
                            tracing::error!("unhandled serenity error in set_banner: {serenity_err:?}")
                        }
                    }
                }
                SetBannerError::CouldNotPickAUrl => warn!("guild_id={guild_id}: 'Could not pick a url'"),
                SetBannerError::CouldNotDeterminFileExtension => {
                    warn!("guild_id={guild_id}: 'Could not determine file extenstion'");
                }
                SetBannerError::MissingBannerFeature => {
                    let _ = repeater_handle.remove(guild_id).await;
                    db.delete::<GuildSchedule>(error.schedule().guild_id().get())
                        .await?;

                    let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                    let guild_owner = partial_guild.owner_id;
                    info!("Letting owner={guild_owner} of guild={guild_id} know about the missing banner feature");

                    dm_user(&ctx, guild_owner, "Server has lost the required boost level. Stopping schedule. You can restart the bot after gaining the required boost level.").await?;
                }
                SetBannerError::MissingAnimatedBannerFeature(url) => {
                    warn!("guild_id={guild_id} with channel={} was trying to set an animated banner but does not have the feature. url={url}", error.schedule().channel_id());
                    let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                    let guild_owner = partial_guild.owner_id;
                    info!("Letting owner={guild_owner} of guild={guild_id} know about the missing animated banner feature");

                    dm_user(&ctx, guild_owner, &format!("Tried to set an animated banner but the server '{}' does not have the required boost level for animated banners", partial_guild.name)).await?;
                }
                SetBannerError::ImageIsEmpty(url) => {
                    warn!(
                        "guild_id={guild_id} with channel={} has selected an image with 0 bytes. url={url}",
                        error.schedule().channel_id()
                    );
                }
                SetBannerError::ImageIsTooBig(url) => {
                    warn!(
                        "guild_id={guild_id} with channel={} has selecte an image that is too big. url={url}",
                        error.schedule().channel_id()
                    );

                    let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                    let guild_owner = partial_guild.owner_id;
                    info!(
                        "Letting owner={guild_owner} of guild={guild_id} know about an image that is too big"
                    );

                    dm_user(&ctx, guild_owner, &format!("The channel you've set contains an image that is too big for discord. Maximum size is 10mb. The image is: {url}")).await?;
                }
                SetBannerError::ImageUnkownSize(url) => {
                    warn!("guild_id={guild_id} with channel={} has selected an image with unknown size. url={url}", error.schedule().channel_id());
                }
            }
        }
        err => {
            tracing::error!("unhandled bot error: {err:?}");
        }
    }

    Ok(ScheduleAction::Continue)
}
