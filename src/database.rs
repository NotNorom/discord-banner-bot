//! here be database stuff

use std::collections::HashMap;

use anyhow::Context;
use fred::{
    error::RedisErrorKind,
    prelude::*,
    types::{RedisKey, RedisMap},
};
use poise::serenity_prelude::GuildId;

#[derive(Clone, Debug)]
pub struct DbEntry {
    guild_id: u64,
    album: String,
    interval: u64,
    last_run: u64,
    // /// The channel the bot will post messages to.
    // /// Will default to Guild system_channel_id if available.
    // /// Otherwise will use the channel from which the /start was last run
    // notification_channel: u64,
}

impl DbEntry {
    pub fn new(guild_id: u64, album: String, interval: u64, last_run: u64) -> Self {
        Self {
            guild_id,
            album,
            interval,
            last_run,
        }
    }

    /// Get a reference to the db entry's guild id.
    pub fn guild_id(&self) -> u64 {
        self.guild_id
    }

    /// Get a reference to the db entry's album.
    pub fn album(&self) -> &str {
        self.album.as_ref()
    }

    /// Get a reference to the db entry's interval.
    pub fn interval(&self) -> u64 {
        self.interval
    }

    /// Get a reference to the db entry's last run.
    pub fn last_run(&self) -> u64 {
        self.last_run
    }

    /// Inserts entry into database
    pub async fn insert(&self, redis: &RedisClient) -> Result<(), RedisError> {
        redis.hmset(key(format!("{}", self.guild_id)), self).await?;
        redis.sadd(key("known_guilds"), self.guild_id.to_string()).await?;
        Ok(())
    }
}

/// Remove entry from the  database
pub async fn remove(redis: &RedisClient, guild_id: GuildId) -> Result<(), RedisError> {
    redis.del(key(format!("{}", guild_id.0))).await?;
    redis.srem(key("known_guilds"), guild_id.0.to_string()).await?;
    Ok(())
}

impl From<DbEntry> for RedisMap {
    fn from(entry: DbEntry) -> Self {
        (&entry).into()
    }
}

impl From<&DbEntry> for RedisMap {
    fn from(entry: &DbEntry) -> Self {
        let mut map = HashMap::with_capacity(5);
        map.insert("guild_id", entry.guild_id.to_string());
        map.insert("album", entry.album.to_owned());
        map.insert("interval", entry.interval.to_string());
        map.insert("last_run", entry.last_run.to_string());
        map.insert("notification_channel", entry.guild_id.to_string());

        // this cannot fail
        RedisMap::try_from(map).unwrap()
    }
}

impl FromRedis for DbEntry {
    fn from_value(value: RedisValue) -> Result<Self, RedisError> {
        use RedisErrorKind::{NotFound, Unknown};

        let value = value.into_map()?;

        let guild_id = value
            .get(&RedisKey::from_static_str("guild_id"))
            .ok_or_else(|| RedisError::new(NotFound, "guild_id"))?
            .as_u64()
            .ok_or_else(|| RedisError::new(Unknown, "guild_id is not u64"))?;
        let album = value
            .get(&RedisKey::from_static_str("album"))
            .ok_or_else(|| RedisError::new(NotFound, "album"))?
            .as_string()
            .ok_or_else(|| RedisError::new(Unknown, "album is not string"))?;
        let interval = value
            .get(&RedisKey::from_static_str("interval"))
            .ok_or_else(|| RedisError::new(NotFound, "interval"))?
            .as_u64()
            .ok_or_else(|| RedisError::new(Unknown, "interval is not u64"))?;
        let last_run = value
            .get(&RedisKey::from_static_str("last_run"))
            .ok_or_else(|| RedisError::new(NotFound, "last_run"))?
            .as_u64()
            .ok_or_else(|| RedisError::new(Unknown, "last_run is not u64"))?;

        Ok(Self {
            guild_id,
            album,
            interval,
            last_run,
        })
    }
}

static REDIS_PREFIX: &str = "dbb"; // dbb => discord banner bot

pub fn key<K>(key: K) -> String
where
    K: Into<RedisKey>,
{
    let key = key.into();
    let key = key.into_string().unwrap();
    format!("{}:{}", REDIS_PREFIX, key)
}

pub async fn setup() -> Result<RedisClient, crate::Error> {
    let config = RedisConfig::default();
    let policy = ReconnectPolicy::new_exponential(0, 100, 30_000, 2);
    let client = RedisClient::new(config);
    let _ = client.connect(Some(policy));
    client.wait_for_connect().await.context("Redis connection setup")?;

    Ok(client)
}
