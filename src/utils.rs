use std::time::SystemTime;

use poise::{
    futures_util::{stream::futures_unordered, StreamExt},
    serenity_prelude::{self, CacheHttp, Message, UserId},
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
) -> Result<Message, serenity_prelude::Error> {
    let user = user.to_user(cache_http.http()).await?;
    user.dm(cache_http.http(), |msg| msg.content(content)).await
}
