use std::collections::HashMap;

use fred::types::RedisMap;

#[derive(Debug, Clone)]
pub struct GuildSettings {
    guild_id: u64,
    /// The channel the bot will post messages to.
    /// Will default to Guild system_channel_id if available.
    /// Otherwise will use the channel from which the /start was last run
    notification_channel: u64,
}

impl From<GuildSettings> for RedisMap {
    fn from(entry: GuildSettings) -> Self {
        (&entry).into()
    }
}

impl From<&GuildSettings> for RedisMap {
    fn from(entry: &GuildSettings) -> Self {
        let mut map = HashMap::with_capacity(5);
        map.insert("guild_id", entry.guild_id.to_string());
        map.insert("notification_channel", entry.notification_channel.to_string());

        RedisMap::try_from(map).unwrap()
    }
}

