use std::{
    fmt::{Debug, Display},
    num::NonZeroU16,
};

use poise::serenity_prelude::User;
use thiserror::Error;
use url::ParseError;

use crate::{
    album_provider::ProviderError,
    constants::{MAXIMUM_INTERVAL, MINIMUM_INTERVAL},
    guild_id_ext::SetBannerError,
    settings::SettingsError,
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
    Provider(#[from] ProviderError),

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
    #[error("Server doesn't have a banner set")]
    GuildHasNoBannerSet,
    #[error("Server doesn't have the required boost level")]
    GuildHasNoBannerFeature,
    #[error("Album is not a valid URL")]
    InvalidUrl(ParseError),
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
pub async fn on_error<U, E: std::fmt::Display + std::fmt::Debug>(
    error: poise::FrameworkError<'_, U, E>,
) -> Result<(), Error> {
    match error {
        poise::FrameworkError::Setup { error, framework, .. } => {
            tracing::error!("Error during framework setup: {:#?}", error);
            framework.shard_manager().shutdown_all().await;
        }
        _ => poise::builtins::on_error(error).await?,
    }

    Ok(())
}
