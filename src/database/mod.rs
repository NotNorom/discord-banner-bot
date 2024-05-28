//! here be database stuff

pub mod guild_schedule;
pub mod guild_settings;

use std::{borrow::Cow, sync::Arc};

use fred::{
    prelude::*,
    types::{ConnectHandle, RedisKey, RedisMap},
};

use poise::async_trait;
use tracing::info;

use crate::settings;

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

    /// Same as [Database::key](Database::key) but namespace aware
    fn key<K>(db: &Database, key: K) -> String
    where
        K: Into<RedisKey>,
    {
        let key = key.into();
        let key = key.into_string().unwrap();
        format!("{}:{}:{}", db.prefix, Self::namespace(), key)
    }
}

/// The database used
#[derive(Clone)]
pub struct Database {
    /// The redis client
    client: RedisClient,
    /// Handle to the connection
    connection_handle: Arc<ConnectHandle>,
    /// Every redis key is prefixed with this string.
    /// This helps identifying this program in case multiple prgrams are using the same
    /// redis instance.
    prefix: Cow<'static, str>,
}

impl Database {
    /// Sets up database connections
    pub async fn setup(settings: &settings::Database) -> Result<Self, crate::Error> {
        let config = RedisConfig::from_url(&settings.host)?;
        let connection = ConnectionConfig::default();
        let policy = ReconnectPolicy::new_exponential(1, 20, 100, 2);
        let client = RedisClient::new(config, None, Some(connection), Some(policy));
        info!("Connecting to database at {}", settings.host);

        let connection = client.init().await?;
        info!("Database connected");

        Ok(Self {
            client,
            connection_handle: Arc::new(connection),
            prefix: Cow::from(settings.prefix.clone()),
        })
    }

    /// Shut down database
    pub async fn disconnect(&self) -> Result<(), RedisError> {
        self.connection_handle.abort();
        Ok(())
    }

    /// Returns a reference to the `RedisClient`
    pub fn client(&self) -> &RedisClient {
        &self.client
    }

    /// Get the database version
    pub async fn db_version(&self) -> Result<String, RedisError> {
        self.client.get(self.key("db_version")).await
    }

    /// Set the database version
    pub async fn set_db_version(&self, version: &str) -> Result<(), RedisError> {
        self.client
            .set(self.key("db_version"), version, None, None, false)
            .await
    }

    /// Get the bot version
    pub async fn bot_version(&self) -> Result<String, RedisError> {
        self.client.get(self.key("bot_version")).await
    }

    /// Set the bot version
    pub async fn set_bot_version(&self, version: &str) -> Result<String, RedisError> {
        self.client
            .set(self.key("bot_version"), version, None, None, false)
            .await
    }

    /// Manipulats the database keys to have the correct prefix
    pub(self) fn key<K>(&self, key: K) -> String
    where
        K: Into<RedisKey>,
    {
        let key = key.into();
        let key = key.into_string().unwrap();
        format!("{}:{}", self.prefix, key)
    }

    /// List of guild ids that have an active schedule going
    pub async fn active_schedules(&self) -> Result<Vec<u64>, RedisError> {
        self.client.smembers(self.key("active_schedules")).await
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
        .clone()
        .convert()
}
