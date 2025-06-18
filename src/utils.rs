//! Collecting of functions that don't fit anywhere else

use std::time::SystemTime;

use poise::{
    futures_util::{StreamExt, stream::futures_unordered},
    serenity_prelude::{CacheHttp, CreateMessage, Message, UserId, UserPublicFlags},
};
use tracing::{debug, warn};

use crate::{Error, constants::DISCORD_MESSAGE_CONTENT_LIMIT, error::SendDm};

/// Returns the amount of seconds since UNIX 0.
///
/// # Panics
/// Could panic if timstamp lies before unix zero but like... why would that happen :V
pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Given the parameters returns the next run in seconds from now
///
/// - `start_at`: unix timestamp in seconds
/// - `now`: unix timestamp in seconds
/// - `interval`: in seconds
pub fn next_run(start_at: u64, now: u64, interval: u64) -> u64 {
    if start_at >= now {
        // seconds between now and stat_at
        start_at - now
    } else {
        // seconds between now and next run
        interval - (now - start_at) % interval
    }
}

/// Starts logging based on `log_level` passed in.
///
/// `log_level` should be defined the same like `RUST_ENV`
///
/// # Panics
/// When logging cannot be set up
pub fn start_logging(log_level: &str) {
    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt::Subscriber::builder()
        .compact()
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
    debug!(log_msg);

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
    let content = truncate_to_discord_limit(content);

    let msg = user
        .id
        .dm(cache_http.http(), CreateMessage::new().content(content))
        .await?;
    Ok(msg)
}

/// Truncates content to fit into discords message limit
///
/// The return value points to a subtring of `content`.
pub fn truncate_to_discord_limit(content: &str) -> &str {
    if content.len() > DISCORD_MESSAGE_CONTENT_LIMIT {
        let mut truncate_at = DISCORD_MESSAGE_CONTENT_LIMIT;
        while !content.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        &content[0..truncate_at]
    } else {
        content
    }
}
