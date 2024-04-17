//! Collecting of functions that don't fit anywhere else

use std::time::SystemTime;

use poise::{
    futures_util::{stream::futures_unordered, StreamExt},
    serenity_prelude::{CacheHttp, CreateMessage, Message, UserId, UserPublicFlags},
};
use tracing::{info, warn};

use crate::{constants::DISCORD_MESSAGE_CONTENT_LIMIT, error::SendDm, Error};

/// Returns the amount of seconds since UNIX 0.
pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Starts logging based on log_level passed in.
///
/// log_level should be defined the same like RUST_ENV
pub fn start_logging(log_level: &str) {
    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(log_level)
        .try_init()
        .expect("Set up logger");
}

/// Send a dm to all `users` with `content`.
pub async fn dm_users(
    cache_http: &impl CacheHttp,
    users: impl IntoIterator<Item = UserId>,
    content: &str,
) -> Result<(), Error> {
    use std::fmt::Write as _;

    let tasks = futures_unordered::FuturesUnordered::new();
    let mut log_msg = String::with_capacity(200);

    write!(log_msg, "Sending dm to users: (")?;

    for user in users {
        write!(log_msg, "{}, ", user.get())?;
        tasks.push(dm_user(&cache_http, user, content));
    }

    write!(log_msg, "). Content: {}", &content)?;
    info!(log_msg);

    let finished_tasks: Vec<_> = tasks.collect().await;

    for error in finished_tasks.iter().filter_map(|res| res.as_ref().err()) {
        warn!("Could not send dm: {error}");
    }

    Ok(())
}

/// Send a dm to a user
pub async fn dm_user(cache_http: &impl CacheHttp, user: UserId, content: &str) -> Result<Message, Error> {
    let user = user.to_user(cache_http.http()).await?;
    if user.bot() {
        return Err(SendDm::bot_user(Box::new(user)));
    }

    if let Some(flags) = user.public_flags {
        if flags.contains(UserPublicFlags::SYSTEM) || flags.contains(UserPublicFlags::TEAM_USER) {
            return Err(SendDm::pseudo_user(Box::new(user)));
        }
    }

    // truncate content
    let content = if content.len() > DISCORD_MESSAGE_CONTENT_LIMIT {
        let mut truncate_at = DISCORD_MESSAGE_CONTENT_LIMIT;
        while !content.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        &content[0..truncate_at]
    } else {
        content
    };

    let msg = user
        .dm(cache_http.http(), CreateMessage::new().content(content))
        .await?;
    Ok(msg)
}
