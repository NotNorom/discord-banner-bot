use std::{
    collections::HashSet,
    fmt::{Debug, Display},
    num::NonZeroU16,
};

use chrono::{DateTime, Utc};
use poise::serenity_prelude::{
    Context, Error as SerenityError, HttpError as SerenityHttpError, JsonErrorCode, MessageBuilder, User,
    UserId,
};
use reqwest::StatusCode;
use thiserror::Error;
use tracing::{info, instrument, warn};

use crate::{
    Settings,
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
    Redis(#[from] fred::error::Error),

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

    #[error("Timeout during: {action}")]
    Timeout { action: String },
}

#[derive(Debug, thiserror::Error)]
pub enum Command {
    #[error("Command must be run in a server")]
    GuildOnly,

    #[error("Server doesn't have a banner set")]
    GuildHasNoBannerSet,

    #[error("Server doesn't have the required boost level")]
    GuildHasNoBannerFeature,

    #[error("Interval must be at least {} minutes", Settings::get().scheduler.minimum_interval)]
    BelowMinTimeout,

    #[error("Interval must be at most {} minutes", Settings::get().scheduler.maximum_interval)]
    AboveMaxTimeout,

    #[error("Message limit must be greater than 0")]
    MessageLimitIszero,

    #[error("Message limit must be at most {}", Settings::get().scheduler.maximum_message_limit)]
    AboveMaxMessageLimit,

    #[error("Start time cannot be in the past. Now={now}, given={given}")]
    StartTimeInThePast {
        now: DateTime<Utc>,
        given: DateTime<Utc>,
    },
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
#[instrument(skip_all)]
pub async fn handle_framework_error<U, E>(error: poise::FrameworkError<'_, U, E>) -> Result<(), Error>
where
    U: Send + Sync + 'static,
    E: std::fmt::Display + std::fmt::Debug,
{
    tracing::error!("{}", &error);

    poise::builtins::on_error(error).await?;

    Ok(())
}

/// Handle scheduler related errors
///
/// This is a needed as well as the normal error handling in [crate::error::on_error] because
/// the scheduler is running in its own task
#[instrument(skip_all)]
pub async fn evaluate_schedule_error(
    error: &RunnerError,
    ctx: Context,
    owners: HashSet<UserId>,
) -> Result<ScheduleAction, Error> {
    let guild_id = error.schedule().guild_id();
    let channel_id = error.schedule().channel_id();

    let guild_name = format!("{guild_id}: {}", guild_id.name(&ctx.cache).unwrap_or_default());

    let message = MessageBuilder::new()
        .push_bold("Error in guild: ")
        .push_mono_line_safe(&*guild_name)
        .push(&*error.to_string())
        .build();

    dm_users(&ctx, owners.clone(), &message).await?;

    match error.source() {
        Error::Serenity(serenity_error) => match serenity_error {
            SerenityError::Http(http_error) => match http_error {
                SerenityHttpError::UnsuccessfulRequest(error_response) => {
                    match error_response.status_code {
                        StatusCode::FORBIDDEN => {
                            // the bot does not have permissions to change the banner.
                            warn!("Missing permissions to change banner for {guild_id}. Unscheduling.");
                            return Ok(ScheduleAction::Abort);
                        }
                        StatusCode::NOT_FOUND => {
                            if error_response.error.code == JsonErrorCode::UnknownChannel {
                                warn!(
                                    "Channel {channel_id} does not exist in guild: {guild_id}. Unscheduling."
                                );
                            }

                            if error_response.error.code == JsonErrorCode::UnknownChannel {
                                warn!("Guild does not exist: {guild_id}. Unscheduling.");
                            }

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
                                        warn!(
                                            "Missing permissions to change banner for {guild_id}. Unscheduling."
                                        );
                                        return Ok(ScheduleAction::Abort);
                                    }
                                    StatusCode::NOT_FOUND => {
                                        warn!("Guild does not exist: {guild_id}. Unscheduling.");
                                        return Ok(ScheduleAction::Abort);
                                    }
                                    StatusCode::GATEWAY_TIMEOUT => {
                                        warn!("Gateway timed out. Retrying");
                                        return Ok(ScheduleAction::RetrySameImage);
                                    }
                                    _ => tracing::error!("unsuccessful http request: {error_response:?}"),
                                }
                            }
                            http_err => tracing::error!("unhandled http error in set_banner: {http_err:?}"),
                        },
                        serenity_err => {
                            tracing::error!("unhandled serenity error in set_banner: {serenity_err:?}");
                        }
                    }
                }
                SetBannerError::CouldNotPickAUrl => {
                    warn!("guild_id={guild_id}: 'Could not pick a url'. RNG failed")
                }
                SetBannerError::CouldNotDeterminFileExtension(url) => {
                    warn!("guild_id={guild_id}: 'Could not determine file extenstion. url={url}'");
                    return Ok(ScheduleAction::RetryNewImage);
                }
                SetBannerError::MissingBannerFeature => {
                    let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                    let guild_owner = partial_guild.owner_id;
                    warn!(
                        "Letting owner={guild_owner} of guild={guild_id} know about the missing banner feature"
                    );

                    dm_user(&ctx, guild_owner, "Server has lost the required boost level. Stopping schedule. You can restart the bot after gaining the required boost level.").await?;
                    return Ok(ScheduleAction::Abort);
                }
                SetBannerError::MissingAnimatedBannerFeature(url, ..) => {
                    warn!(
                        "guild_id={guild_id} with channel={channel_id} was trying to set an animated banner but does not have the feature. url={url}"
                    );
                    let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                    let guild_owner = partial_guild.owner_id;
                    warn!(
                        "Letting owner={guild_owner} of guild={guild_id} know about the missing animated banner feature"
                    );

                    dm_user(&ctx, guild_owner, &format!("Tried to set an animated banner but the server '{}' does not have the required boost level for animated banners", partial_guild.name)).await?;
                    return Ok(ScheduleAction::RetryNewImage);
                }
                SetBannerError::ImageIsEmpty(url, ..) => {
                    warn!(
                        "guild_id={guild_id} with channel={channel_id} has selected an image with 0 bytes. url={url}"
                    );
                    return Ok(ScheduleAction::RetryNewImage);
                }
                SetBannerError::ImageIsTooBig(url, ..) => {
                    warn!(
                        "guild_id={guild_id} with channel={channel_id} has selecte an image that is too big. url={url}"
                    );

                    let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                    let guild_owner = partial_guild.owner_id;
                    info!(
                        "Letting owner={guild_owner} of guild={guild_id} know about an image that is too big"
                    );

                    dm_user(&ctx, guild_owner, &format!("The channel you've set contains an image that is too big for discord. Maximum size is 10mb. The image is: {url}")).await?;
                    return Ok(ScheduleAction::RetryNewImage);
                }
                SetBannerError::ImageUnkownSize(url, ..) => {
                    warn!(
                        "guild_id={guild_id} with channel={channel_id} has selected an image with unknown size. url={url}"
                    );
                    return Ok(ScheduleAction::RetryNewImage);
                }
                SetBannerError::Base64Encoding(url, ..) => {
                    warn!(
                        "guild_id={guild_id} with channel={channel_id} has selected an image wich could be encoded into base 64. url={url}"
                    );
                    return Ok(ScheduleAction::RetryNewImage);
                }
            }
        }
        Error::Timeout { action } => {
            tracing::error!("Timeout in guild {guild_id} / {guild_name} with action = '{action}'.");
        }
        err => {
            tracing::error!("unhandled bot error: {err:?}");
        }
    }

    Ok(ScheduleAction::Continue)
}
