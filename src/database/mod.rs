//! here be database stuff

pub mod guild_schedule;
pub mod guild_settings;

use anyhow::Context;
use fred::{
    prelude::*,
    types::{RedisKey, RedisMap},
};

pub use guild_schedule::GuildSchedule;
use poise::async_trait;

/// Describes how a struct is interacting with the database
///
/// Different struct have different ways of being inserted or fetched from the
/// database. This trait allows each struct to specify how to do that.
#[async_trait]
pub trait Entry {
    /// Insert entry into database
    async fn insert(&self, db: &Database, id: impl Into<RedisKey> + Send + Sync) -> Result<(), RedisError>;
    /// Get entry from database
    async fn get<T>(db: &Database, id: impl Into<RedisKey> + Send + Sync) -> Result<T, RedisError>
    where
        T: FromRedis + Unpin + Send + 'static;
    /// Delete Entry from database
    async fn delete(db: &Database, id: impl Into<RedisKey> + Send + Sync) -> Result<(), RedisError>;
    /// The namespace for the type
    fn namespace() -> &'static str;
}

/// The database used
#[derive(Clone)]
pub struct Database {
    client: RedisClient,
    /// Every redis key is prefixed with this string.
    /// This helps identifying this program in case multiple prgrams are using the same
    /// redis instance.
    prefix: &'static str,
}

impl Database {
    /// Sets up database connections
    pub async fn setup(prefix: &'static str) -> Result<Self, crate::Error> {
        let config = RedisConfig::default();
        let policy = ReconnectPolicy::new_exponential(0, 100, 30_000, 2);
        let client = RedisClient::new(config);
        let _ = client.connect(Some(policy));
        client
            .wait_for_connect()
            .await
            .context("Redis connection setup")?;

        Ok(Self { client, prefix })
    }

    /// Manipulats the database keys to have the correct prefix
    fn key<K>(&self, key: K) -> String
    where
        K: Into<RedisKey>,
    {
        let key = key.into();
        let key = key.into_string().unwrap();
        format!("{}:{}", self.prefix, key)
    }

    /// List of guild ids that have an active schedule going
    pub async fn known_guilds(&self) -> Result<Vec<u64>, RedisError> {
        self.client.smembers(self.key("known_guilds")).await
    }

    /// Insert entry into database
    pub async fn insert<T>(&self, entry: &T, id: impl Into<RedisKey> + Send + Sync) -> Result<(), RedisError>
    where
        T: Entry + Unpin + Send,
    {
        entry.insert(self, id).await
    }

    /// Get entry from database
    pub async fn get<T>(&self, id: impl Into<RedisKey> + Send + Sync) -> Result<T, RedisError>
    where
        T: FromRedis + Entry + Unpin + Send + 'static,
    {
        T::get(self, id).await
    }

    /// Delete entry from database
    pub async fn delete<T>(&self, id: impl Into<RedisKey> + Send + Sync) -> Result<(), RedisError>
    where
        T: FromRedis + Entry + Unpin + Send + 'static,
    {
        T::delete(self, id).await
    }
}

/// Get the value with `key` from a [RedisMap](RedisMap) `map`
fn get_from_redis_map<T: FromRedis>(map: &RedisMap, key: &str) -> Result<T, RedisError> {
    use RedisErrorKind::NotFound;
    map.get(&RedisKey::from(key))
        .ok_or_else(|| RedisError::new(NotFound, format!("Key {key} not found in RedisMap")))?
        .to_owned()
        .convert()
}