use std::time::SystemTime;

use anyhow::Context;
use poise::{
    futures_util::{stream::futures_unordered, StreamExt},
    serenity_prelude::{CacheHttp, Message, UserId, UserPublicFlags},
};
use tracing::{info, warn};

use crate::Error;

/// Returns the amount of seconds since UNIX 0.
pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Send a dm to all `users` with `content`.
pub async fn dm_users(
    cache_http: &impl CacheHttp,
    users: impl IntoIterator<Item = UserId>,
    content: &impl std::fmt::Display,
) -> Result<(), Error> {
    use std::fmt::Write as _;

    let tasks = futures_unordered::FuturesUnordered::new();
    let mut log_msg = String::with_capacity(200);

    write!(log_msg, "Sending dm to users: (")?;

    for user in users.into_iter() {
        write!(log_msg, "{}, ", user.0)?;
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
async fn dm_user(
    cache_http: &impl CacheHttp,
    user: UserId,
    content: &impl std::fmt::Display,
) -> Result<Message, Error> {
    let user = user.to_user(cache_http.http()).await?;
    if user.bot {
        return Err(Error::SendDm(user, "User is a bot".to_string()));
    }

    if let Some(flags) = user.public_flags {
        if flags.contains(UserPublicFlags::SYSTEM) || flags.contains(UserPublicFlags::TEAM_USER) {
            return Err(Error::SendDm(user, "User is a pseudo user".to_string()));
        }
    }

    let msg = user
        .dm(cache_http.http(), |msg| msg.content(content))
        .await
        .context(format!("User: {user}"))?;
    Ok(msg)
}
