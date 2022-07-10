use std::process::exit;
use thiserror::Error;

use crate::banner_scheduler::ScheduleMessage;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Serenity(#[from] poise::serenity::Error),

    #[error(transparent)]
    Redis(#[from] fred::error::RedisError),

    #[error(transparent)]
    Dotenv(#[from] dotenv::Error),

    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),

    #[error("Scheduluer Error: {msg:?}")]
    Scheduler {
        msg: ScheduleMessage,
    },

    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("Extraction of imgur hash failed: {0}")]
    ImgurHashExtraction(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub async fn on_error<U, E: std::fmt::Display + std::fmt::Debug>(
    error: poise::FrameworkError<'_, U, E>,
) -> Result<(), poise::serenity::Error> {
    match error {
        poise::FrameworkError::Setup { error } => {
            println!("Error in user data setup: {}", error);
            exit(1);
        }
        poise::FrameworkError::Listener { error, event, .. } => println!(
            "User event listener encountered an error on {} event: {}",
            event.name(),
            error
        ),
        poise::FrameworkError::Command { ctx, error } => {
            let error = error.to_string();
            ctx.say(error).await?;
        }
        poise::FrameworkError::ArgumentParse { ctx, input, error } => {
            // If we caught an argument parse error, give a helpful error message with the
            // command explanation if available
            let usage = match ctx.command().multiline_help {
                Some(multiline_help) => multiline_help(),
                None => "Please check the help menu for usage information".into(),
            };
            let response = if let Some(input) = input {
                format!("**Cannot parse `{}` as argument: {}**\n{}", input, error, usage)
            } else {
                format!("**{}**\n{}", error, usage)
            };
            ctx.say(response).await?;
        }
        poise::FrameworkError::CommandStructureMismatch { ctx, description } => {
            println!(
                "Error: failed to deserialize interaction arguments for `/{}`: {}",
                ctx.command.name, description,
            );
        }
        poise::FrameworkError::CommandCheckFailed { ctx, error } => {
            println!(
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
        poise::FrameworkError::DynamicPrefix { error } => {
            println!("Dynamic prefix failed: {}", error);
        }
        poise::FrameworkError::__NonExhaustive => panic!(),
    }

    Ok(())
}
