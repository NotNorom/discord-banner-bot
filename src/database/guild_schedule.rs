use std::{collections::HashMap, num::NonZeroUsize};

use fred::{
    error::{RedisError, RedisErrorKind},
    interfaces::{HashesInterface, KeysInterface, SetsInterface},
    types::{FromRedis, RedisKey, RedisMap, RedisValue},
};
use poise::async_trait;
use tracing::debug;

use super::{get_from_redis_map, Database, Entry};
use crate::{schedule::Schedule, utils::current_unix_timestamp};

/// How a schedule is stored in the database
#[derive(Debug, Clone, Copy)]
pub struct GuildSchedule {
    /// The Guilds ID
    guild_id: u64,
    /// Channel ID to fetch images from
    channel_id: u64,
    /// How frequent the schudle run. In seconds
    interval: u64,
    /// When to start the schedule (in seconds)
    start_at: u64,
    /// Unix timestamp since the banner was last changed (in seconds)
    last_run: u64,
    /// How many messages to look into the past for
    message_limit: u64,
}

impl GuildSchedule {
    pub fn new(
        guild_id: u64,
        channel_id: u64,
        interval: u64,
        last_run: u64,
        start_at: u64,
        message_limit: u64,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            interval,
            start_at,
            last_run,
            message_limit,
        }
    }

    /// Get the db entry's guild id.
    pub fn guild_id(&self) -> u64 {
        self.guild_id
    }

    /// Get the db entry's channel id.
    pub fn channel_id(&self) -> u64 {
        self.channel_id
    }

    /// Get the db entry's interval.
    pub fn interval(&self) -> u64 {
        self.interval
    }

    /// Get db entry's `last_run`.
    pub fn last_run(&self) -> u64 {
        self.last_run
    }

    /// Get db entry's `start_at`.
    pub fn start_at(&self) -> u64 {
        self.start_at
    }

    /// Get the db entry's message limit.
    pub fn message_limit(&self) -> u64 {
        self.message_limit
    }
}

impl From<Schedule> for GuildSchedule {
    fn from(schedule: Schedule) -> Self {
        let guild_id = schedule.guild_id().get();
        let channel_id = schedule.channel_id().get();
        let interval = schedule.interval();
        let start_at = schedule.start_at();
        let now = current_unix_timestamp();

        let last_run = if start_at > now { start_at } else { now };
        debug!("Setting start_at={start_at}, last_run={last_run}");

        let message_limit = schedule
            .message_limit()
            .map(NonZeroUsize::get)
            .unwrap_or_default()
            .try_into()
            .expect("If the limit does not fit in  a 64 bit uint may god help us all");

        Self {
            guild_id,
            channel_id,
            interval,
            start_at,
            last_run,
            message_limit,
        }
    }
}

impl From<GuildSchedule> for RedisMap {
    fn from(entry: GuildSchedule) -> Self {
        (&entry).into()
    }
}

impl From<&GuildSchedule> for RedisMap {
    fn from(entry: &GuildSchedule) -> Self {
        let mut map = HashMap::with_capacity(6);
        map.insert("guild_id", entry.guild_id.to_string());
        map.insert("channel_id", entry.channel_id.to_string());
        map.insert("interval", entry.interval.to_string());
        map.insert("last_run", entry.last_run.to_string());
        map.insert("start_at", entry.start_at.to_string());
        map.insert("message_limit", entry.message_limit.to_string());

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
        let channel_id = get_from_redis_map(&value, "channel_id")?;
        let interval = get_from_redis_map(&value, "interval")?;
        let last_run = get_from_redis_map(&value, "last_run")?;
        let start_at = get_from_redis_map(&value, "start_at")?;
        let message_limit = get_from_redis_map(&value, "message_limit")?;

        Ok(Self {
            guild_id,
            channel_id,
            interval,
            start_at,
            last_run,
            message_limit,
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
        db.client.hgetall(Self::key(db, id)).await
    }

    async fn delete(db: &Database, id: impl Into<RedisKey> + Send + Sync) -> Result<(), RedisError> {
        let id: RedisKey = id.into();
        debug!("Deleting id: {id:?}");
        db.client.del(Self::key(db, &id)).await?;
        db.client.srem(db.key("active_schedules"), id.clone()).await?;
        debug!("Deleted id: {id:?} successfully");

        Ok(())
    }

    fn namespace() -> &'static str {
        "active_schedule"
    }
}
