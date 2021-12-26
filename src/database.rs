//! here be database stuff

use fred::{error::RedisErrorKind, prelude::*};

#[derive(Clone, Debug)]
pub struct DbEntry {
    guild_id: u64,
    album: String,
    interval: u64,
    last_run: u64,
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

    pub fn next_run(&self) -> u64 {
        0
    }
}

impl From<DbEntry> for RedisMap {
    fn from(entry: DbEntry) -> Self {
        (&entry).into()
    }
}

impl From<&DbEntry> for RedisMap {
    fn from(entry: &DbEntry) -> Self {
        let mut map = RedisMap::new();
        map.insert(
            "guild_id".to_string(),
            RedisValue::String(entry.guild_id.to_string()),
        );
        map.insert("album".to_string(), RedisValue::String(entry.album.to_owned()));
        map.insert(
            "interval".to_string(),
            RedisValue::String(entry.interval.to_string()),
        );
        map.insert(
            "last_run".to_string(),
            RedisValue::String(entry.last_run.to_string()),
        );

        map
    }
}

impl RedisResponse for DbEntry {
    fn from_value(value: RedisValue) -> Result<Self, RedisError> {
        use RedisErrorKind::{NotFound, Unknown};

        let value = value.into_map()?;

        let guild_id = value
            .get("guild_id")
            .ok_or_else(|| RedisError::new(NotFound, "guild_id"))?
            .as_u64()
            .ok_or_else(|| RedisError::new(Unknown, "guild_id is not u64"))?;
        let album = value
            .get("album")
            .ok_or_else(|| RedisError::new(NotFound, "album"))?
            .as_string()
            .ok_or_else(|| RedisError::new(Unknown, "album is not string"))?;
        let interval = value
            .get("interval")
            .ok_or_else(|| RedisError::new(NotFound, "interval"))?
            .as_u64()
            .ok_or_else(|| RedisError::new(Unknown, "album is not u64"))?;
        let last_run = value
            .get("last_run")
            .ok_or_else(|| RedisError::new(NotFound, "last_run"))?
            .as_u64()
            .ok_or_else(|| RedisError::new(Unknown, "album is not u64"))?;

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
    let _ = client.wait_for_connect().await?;

    Ok(client)
}
