use std::{fmt::Debug, num::NonZeroUsize, time::Duration};

use async_repeater::RepeaterEntry;
use poise::serenity_prelude::{ChannelId, GuildId};

use crate::{database::guild_schedule::GuildSchedule, utils::current_unix_timestamp};

#[derive(Clone)]
pub struct Schedule {
    guild_id: GuildId,
    channel_id: ChannelId,
    interval: Duration,
    offset: Option<Duration>,
    message_limit: Option<NonZeroUsize>,
}

impl Schedule {
    pub fn guild_id(&self) -> GuildId {
        self.guild_id
    }

    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn offset(&self) -> Option<Duration> {
        self.offset
    }

    pub fn message_limit(&self) -> Option<NonZeroUsize> {
        self.message_limit
    }
}

pub struct ScheduleBuilder {
    guild_id: GuildId,
    channel_id: ChannelId,
    interval: Duration,
    offset: Option<Duration>,
    message_limit: Option<NonZeroUsize>,
}

impl ScheduleBuilder {
    pub fn new(guild_id: GuildId, channel_id: ChannelId, interval: Duration) -> Self {
        Self {
            guild_id,
            channel_id,
            interval,
            offset: None,
            message_limit: None,
        }
    }

    pub fn offset(mut self, offset: Duration) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn message_limit(mut self, message_limit: usize) -> Self {
        self.message_limit = NonZeroUsize::new(message_limit);
        self
    }

    pub fn build(self) -> Schedule {
        let ScheduleBuilder {
            guild_id,
            channel_id,
            interval,
            offset,
            message_limit,
        } = self;
        Schedule {
            guild_id,
            channel_id,
            interval,
            offset,
            message_limit,
        }
    }
}

impl RepeaterEntry for Schedule {
    type Key = GuildId;

    fn when(&self) -> std::time::Duration {
        self.interval
    }

    fn key(&self) -> Self::Key {
        self.guild_id
    }

    fn delay(&self) -> Option<Duration> {
        self.offset
    }

    fn reset_delay(&mut self) {
        self.offset = None;
    }
}

impl Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Schedule(guild_id = {}, interval = {:?}, offset = {:?}, channel = {}",
            self.guild_id, self.interval, self.offset, self.channel_id
        )
    }
}

impl From<GuildSchedule> for Schedule {
    fn from(guild_schedule: GuildSchedule) -> Self {
        let guild_id = guild_schedule.guild_id();
        let channel = guild_schedule.channel_id();
        let interval = guild_schedule.interval();
        let last_run = guild_schedule.last_run();
        let message_limit = guild_schedule.message_limit();

        let current_time = current_unix_timestamp();
        let offset = interval - (current_time - last_run) % interval;

        Schedule {
            interval: Duration::from_secs(interval),
            offset: Some(Duration::from_secs(offset)),
            guild_id: GuildId::new(guild_id),
            channel_id: ChannelId::new(channel),
            message_limit: NonZeroUsize::new(message_limit.try_into().unwrap_or(usize::MAX)),
        }
    }
}
