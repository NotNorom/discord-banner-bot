use std::fmt::{Debug, Display};

use poise::serenity_prelude::User;
use thiserror::Error;

use crate::{
    album_provider::ProviderKind,
    banner_scheduler::ScheduleMessage,
    constants::{MAXIMUM_INTERVAL, MINIMUM_INTERVAL},
};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Serenity(#[from] poise::serenity_prelude::Error),

    #[error(transparent)]
    Redis(#[from] fred::error::RedisError),

    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),

    #[error(transparent)]
    Imgur(#[from] imgurs::Error),

    #[error(transparent)]
    Command(#[from] Command),

    #[error(transparent)]
    SchedulerTask(#[from] SchedulerTask),

    #[error("Scheduler Error: {msg:?}. Please contact the developer. See /help")]
    Scheduler { msg: ScheduleMessage },

    #[error("Unsupported provider: {0}. For a list of supported providers see /help")]
    UnsupportedProvider(String),

    #[error(
        "Inactive provider: {0:?}. Provider is supported but inactive. Please contact the bot owner /help"
    )]
    InactiveProvider(ProviderKind),

    #[error("Extraction of imgur id failed: {0}. Is the url correct?")]
    ImgurIdExtraction(String),

    #[error(transparent)]
    StdFmt(#[from] std::fmt::Error),

    #[error(transparent)]
    SendDm(#[from] SendDm),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum Command {
    #[error("Command must be run in a server")]
    GuildOnly,
    #[error("Interval must be at least {} minutes", MINIMUM_INTERVAL)]
    BelowMinTimeout,
    #[error("Interval must be at most {} minutes", MAXIMUM_INTERVAL)]
    AboveMaxTimeout,
    #[error("Server doesn't have a banner set")]
    GuildHasNoBannerSet,
    #[error("Server doesn't have the required boost level")]
    GuildHasNoBannerFeature,
}

#[derive(Debug, thiserror::Error)]
pub enum SchedulerTask {
    #[error("Server doesn't have the required boost level for banners")]
    GuildHasNoBannerFeature,
    #[error("Server doesn't have the required boost level for animated banners")]
    GuildHasNoAnimatedBannerFeature,
}

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
    PseudoUser,
    BotUser,
    Other(String),
}

impl Display for SendDm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use SendDmKind::*;

        let user_name = &self.user.name;
        let discriminator = self.user.discriminator;
        let user_id = self.user.id;

        write!(f, "Could not send dm to {user_name}#{discriminator}, {user_id}: ")?;

        match self.kind {
            PseudoUser => write!(f, "Pseudo user"),
            BotUser => write!(f, "User is a bot"),
            Other(ref reason) => write!(f, "{reason}"),
        }
    }
}

pub async fn on_error<U, E: std::fmt::Display + std::fmt::Debug>(
    error: poise::FrameworkError<'_, U, E>,
) -> Result<(), Error> {
    match error {
        poise::FrameworkError::Setup { error, framework, .. } => {
            tracing::error!("Error during framework setup: {:#?}", error);
            let mut shard_manager = framework.shard_manager().lock().await;
            shard_manager.shutdown_all().await;
        }
        _ => poise::builtins::on_error(error).await?,
    }

    Ok(())
}
