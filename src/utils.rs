use std::time::SystemTime;

use poise::serenity_prelude::{Http, UserId};

use crate::Error;

/// Returns the amount of seconds since UNIX 0.
pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// this is a dirty hack and I need a better way of figuring out how to get the bot owner at this point
// yes, that is _my_ user id
pub async fn say_to_owner(http: impl AsRef<Http>, content: impl std::fmt::Display) -> Result<(), Error> {
    let owner = UserId(160518747713437696)
        .create_dm_channel(http.as_ref())
        .await?;
    owner.say(http.as_ref(), content).await?;
    Ok(())
}
