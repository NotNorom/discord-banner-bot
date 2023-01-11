use poise::serenity_prelude::User;
use thiserror::Error;
use tracing::warn;

use crate::{
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

    #[error("Scheduler Error: {msg:?}. Please contact the developer. See /help")]
    Scheduler { msg: ScheduleMessage },

    #[error("Unsupported provider: {0}. For a list of supported providers see /help")]
    UnsupportedProvider(String),

    #[error("Extraction of imgur hash failed: {0}. Is the url correct?")]
    ImgurHashExtraction(String),

    #[error(transparent)]
    StdFmt(#[from] std::fmt::Error),

    #[error("Could not send dm to {:?}. Reason: {}", .0, .1)]
    SendDm(User, String),

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
    #[error("Server doesn't have a banner. Is the required boost level reached?")]
    GuildHasNoBanner,
}

pub async fn on_error<U, E: std::fmt::Display + std::fmt::Debug>(
    error: poise::FrameworkError<'_, U, E>,
) -> Result<(), Error> {
    match error {
        poise::FrameworkError::Setup {
            error,
            framework,
            data_about_bot: _,
            ctx: _,
        } => {
            tracing::error!("Error during framework setup: {}", error);
            let mut shard_manager = framework.shard_manager().lock().await;
            shard_manager.shutdown_all().await;
        }
        poise::FrameworkError::Command { ctx, error } => {
            let error = error.to_string();
            ctx.say(&error).await?;
            warn!("FrameworkCommand: {error}");
        }
        poise::FrameworkError::ArgumentParse { ctx, input, error } => {
            // If we caught an argument parse error, give a helpful error message with the
            // command explanation if available
            let usage = ctx
                .command()
                .description
                .to_owned()
                .unwrap_or_else(|| "Please check the help menu for usage information".to_string());
            let response = if let Some(input) = input {
                format!("**Cannot parse `{}` as argument: {}**\n{}", input, error, usage)
            } else {
                format!("**{}**\n{}", error, usage)
            };
            ctx.say(response).await?;
        }
        poise::FrameworkError::CommandStructureMismatch { ctx, description } => {
            warn!(
                "Error: failed to deserialize interaction arguments for `/{}`: {}",
                ctx.command.name, description,
            );
        }
        poise::FrameworkError::CommandCheckFailed { ctx, error } => {
            warn!(
                "A command check failed in command {} for user {}: {:?}",
                ctx.command().name,
                ctx.author().name,
                error,
            );
        }
        poise::FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
        } => {
            let msg = format!(
                "You're too fast. Please wait {} seconds before retrying",
                remaining_cooldown.as_secs()
            );
            ctx.send(|b| b.content(msg).ephemeral(true)).await?;
        }
        poise::FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
        } => {
            let msg = format!(
                "Command cannot be executed because the bot is lacking permissions: {}",
                missing_permissions,
            );
            ctx.send(|b| b.content(msg).ephemeral(true)).await?;
        }
        poise::FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
        } => {
            let response = if let Some(missing_permissions) = missing_permissions {
                format!(
                    "You're lacking permissions for `{}{}`: {}",
                    ctx.prefix(),
                    ctx.command().name,
                    missing_permissions,
                )
            } else {
                format!(
                    "You may be lacking permissions for `{}{}`. Not executing for safety",
                    ctx.prefix(),
                    ctx.command().name,
                )
            };
            ctx.send(|b| b.content(response).ephemeral(true)).await?;
        }
        poise::FrameworkError::NotAnOwner { ctx } => {
            let response = "Only bot owners can call this command";
            ctx.send(|b| b.content(response).ephemeral(true)).await?;
        }
        poise::FrameworkError::GuildOnly { ctx } => {
            let response = "You cannot run this command in DMs.";
            ctx.send(|b| b.content(response).ephemeral(true)).await?;
        }
        poise::FrameworkError::DmOnly { ctx } => {
            let response = "You cannot run this command outside DMs.";
            ctx.send(|b| b.content(response).ephemeral(true)).await?;
        }
        poise::FrameworkError::NsfwOnly { ctx } => {
            let response = "You cannot run this command outside NSFW channels.";
            ctx.send(|b| b.content(response).ephemeral(true)).await?;
        }
        poise::FrameworkError::DynamicPrefix { error, ctx: _, msg } => {
            warn!("Dynamic prefix failed: Error={error:?}, Msg={msg:?}");
        }
        poise::FrameworkError::EventHandler {
            error,
            ctx: _,
            event,
            framework: _,
        } => warn!("Eventhandler failed: {error:?} with event {event:?}"),
        poise::FrameworkError::UnknownCommand {
            ctx: _,
            msg,
            prefix,
            msg_content: _,
            framework: _,
            invocation_data: _,
            trigger: _,
        } => warn!("Unkown command encountered. Prefix={prefix:?}, Msg={msg:?}"),
        poise::FrameworkError::UnknownInteraction {
            ctx: _,
            framework: _,
            interaction,
        } => warn!("Unkown interaction encountered. Msg={interaction:?}"),
        unknown_err => {
            tracing::error!("Unkown error occurred: {unknown_err}")
        }
    }

    Ok(())
}
