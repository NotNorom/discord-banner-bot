use std::collections::HashMap;

use fred::{
    prelude::{HashesInterface, KeysInterface, RedisError, RedisErrorKind, SetsInterface},
    types::{FromRedis, RedisKey, RedisMap, RedisValue},
};
use poise::async_trait;

use super::{get_from_redis_map, Database, Entry};

/// How a schedule is stored in the database
#[derive(Clone, Debug)]
pub struct GuildSchedule {
    /// The Guilds ID
    guild_id: u64,
    /// Url to the album
    album: String,
    /// How frequent the schudle run. In seconds
    interval: u64,
    /// Unix timestamp since the banner was last changed
    last_run: u64,
}

impl GuildSchedule {
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
}

impl From<GuildSchedule> for RedisMap {
    fn from(entry: GuildSchedule) -> Self {
        (&entry).into()
    }
}

impl From<&GuildSchedule> for RedisMap {
    fn from(entry: &GuildSchedule) -> Self {
        let mut map = HashMap::with_capacity(5);
        map.insert("guild_id", entry.guild_id.to_string());
        map.insert("album", entry.album.clone());
        map.insert("interval", entry.interval.to_string());
        map.insert("last_run", entry.last_run.to_string());

        // this cannot fail
        RedisMap::try_from(map).unwrap()
    }
}

impl From<GuildSchedule> for RedisKey {
    fn from(schedule: GuildSchedule) -> Self {
        schedule.guild_id.into()
    }
}

impl FromRedis for GuildSchedule {
    fn from_value(value: RedisValue) -> Result<Self, RedisError> {
        let value = value.into_map()?;

        let guild_id = get_from_redis_map(&value, "guild_id")?;
        let album = get_from_redis_map(&value, "album")?;
        let interval = get_from_redis_map(&value, "interval")?;
        let last_run = get_from_redis_map(&value, "last_run")?;

        Ok(Self {
            guild_id,
            album,
            interval,
            last_run,
        })
    }
}

#[async_trait]
impl Entry for GuildSchedule {
    async fn insert(&self, db: &Database, id: impl Into<RedisKey> + Send + Sync) -> Result<(), RedisError> {
        let id: RedisKey = id.into();

        db.client.hset(Self::key(db, &id), self).await?;
        db.client.sadd(db.key("active_schedules"), id).await?;

        Ok(())
    }

    async fn get<T>(db: &Database, id: impl Into<RedisKey> + Send + Sync) -> Result<T, RedisError>
    where
        T: FromRedis + Unpin + Send + 'static,
    {
        let id: RedisKey = id.into();

        if !db
            .client
            .sismember(db.key("active_schedules"), id.clone())
            .await?
        {
            return Err(RedisError::new(RedisErrorKind::NotFound, "No active schedule."));
        }
        db.client.hgetall(Self::key(&db, id)).await
    }

    async fn delete(db: &Database, id: impl Into<RedisKey> + Send + Sync) -> Result<(), RedisError> {
        let id: RedisKey = id.into();
        db.client.del(Self::key(db, &id)).await?;
        db.client.srem(db.key("active_schedules"), id).await?;

        Ok(())
    }

    fn namespace() -> &'static str {
        "active_schedule"
    }
}
