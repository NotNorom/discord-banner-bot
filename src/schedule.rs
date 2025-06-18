use std::{
    fmt::Debug,
    num::{NonZeroU64, NonZeroUsize},
    time::Duration,
};

use async_repeater::{Delay, RepeaterEntry};
use poise::serenity_prelude::{GenericChannelId, GuildId};

use crate::{
    database::guild_schedule::GuildSchedule,
    utils::{current_unix_timestamp, next_run},
};

#[derive(Clone)]
pub struct Schedule {
    guild_id: GuildId,
    channel_id: GenericChannelId,
    interval: u64,
    start_at: u64,
    last_run: Option<NonZeroU64>,
    message_limit: Option<NonZeroUsize>,
}

impl Schedule {
    /// Which guild to change the banner from
    pub fn guild_id(&self) -> GuildId {
        self.guild_id
    }

    /// Which channel to source images from
    pub fn channel_id(&self) -> GenericChannelId {
        self.channel_id
    }

    /// How many seconds in between schedules
    pub fn interval(&self) -> u64 {
        self.interval
    }

    /// When the schedule is supposed to start
    pub fn start_at(&self) -> u64 {
        self.start_at
    }

    /// When the schedule last finished running
    pub fn last_run(&self) -> Option<NonZeroU64> {
        self.last_run
    }

    /// Message limit
    pub fn message_limit(&self) -> Option<NonZeroUsize> {
        self.message_limit
    }

    /// How many seconds the `last_run` is late
    pub fn lag(&self) -> Option<u64> {
        self.last_run.map(|x| x.get() % self.interval)
    }
}

pub struct ScheduleBuilder {
    guild_id: GuildId,
    channel_id: GenericChannelId,
    interval: u64,
    start_at: u64,
    last_run: Option<NonZeroU64>,
    message_limit: Option<NonZeroUsize>,
}

impl ScheduleBuilder {
    pub fn new(guild_id: GuildId, channel_id: GenericChannelId, interval: u64) -> Self {
        Self {
            guild_id,
            channel_id,
            interval,
            start_at: current_unix_timestamp(),
            last_run: None,
            message_limit: None,
        }
    }

    #[must_use]
    pub fn start_at(mut self, start_at: u64) -> Self {
        self.start_at = start_at;
        self
    }

    #[must_use]
    pub fn message_limit(mut self, message_limit: usize) -> Self {
        self.message_limit = NonZeroUsize::new(message_limit);
        self
    }

    pub fn build(self) -> Schedule {
        let ScheduleBuilder {
            guild_id,
            channel_id,
            interval,
            start_at,
            last_run,
            message_limit,
        } = self;
        Schedule {
            guild_id,
            channel_id,
            interval,
            start_at,
            last_run,
            message_limit,
        }
    }
}

impl RepeaterEntry for Schedule {
    type Key = GuildId;

    fn interval(&self) -> Duration {
        Duration::from_secs(self.interval)
    }

    fn key(&self) -> Self::Key {
        self.guild_id
    }

    fn delay(&self) -> Delay {
        let now = current_unix_timestamp();
        let next_run = next_run(self.start_at, now, self.interval);
        Delay::Relative(Duration::from_secs(next_run))
    }
}

impl Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            guild_id,
            channel_id,
            interval,
            start_at,
            last_run,
            message_limit,
        } = self;

        let last_run = last_run.map(NonZeroU64::get).unwrap_or_default();
        let message_limit = message_limit.map(NonZeroUsize::get).unwrap_or_default();

        write!(
            f,
            "Schedule(guild={guild_id}, channel={channel_id}, interval={interval}, start_at={start_at}, last_run={last_run}, message_limit={message_limit}",
        )
    }
}

impl From<GuildSchedule> for Schedule {
    fn from(guild_schedule: GuildSchedule) -> Self {
        let guild_id = guild_schedule.guild_id();
        let channel = guild_schedule.channel_id();
        let interval = guild_schedule.interval();
        let start_at = guild_schedule.start_at();
        let last_run = guild_schedule.last_run();
        let message_limit = guild_schedule.message_limit();

        Schedule {
            guild_id: GuildId::new(guild_id),
            channel_id: GenericChannelId::new(channel),
            interval,
            start_at,
            last_run: NonZeroU64::new(last_run),
            message_limit: NonZeroUsize::new(message_limit.try_into().unwrap_or(usize::MAX)),
        }
    }
}
