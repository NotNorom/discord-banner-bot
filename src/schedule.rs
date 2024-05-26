use std::{
    fmt::Debug,
    num::NonZeroUsize,
    time::{Duration, SystemTime},
};

use async_repeater::{Delay, RepeaterEntry};
use poise::serenity_prelude::{ChannelId, GuildId};
use tracing::debug;

use crate::{database::guild_schedule::GuildSchedule, utils::current_unix_timestamp};

#[derive(Clone)]
pub struct Schedule {
    guild_id: GuildId,
    channel_id: ChannelId,
    interval: Duration,
    offset: Delay,
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

    pub fn offset(&self) -> Delay {
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
    offset: Delay,
    message_limit: Option<NonZeroUsize>,
}

impl ScheduleBuilder {
    pub fn new(guild_id: GuildId, channel_id: ChannelId, interval: Duration) -> Self {
        Self {
            guild_id,
            channel_id,
            interval,
            offset: Delay::None,
            message_limit: None,
        }
    }

    pub fn offset(mut self, offset: Delay) -> Self {
        self.offset = offset;
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

    fn delay(&self) -> Delay {
        self.offset
    }

    fn reset_delay(&mut self) {
        self.offset = Delay::None;
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
        let start_at = guild_schedule.start_at();
        let message_limit = guild_schedule.message_limit();

        let now = current_unix_timestamp();
        // if the start_at time is now or in the future, the the start_at time
        // if the start_at time is in the past use the cyclic interval calculation
        let offset = if start_at >= now {
            debug!("setting offset using start_at = {start_at}s");
            start_at
        } else {
            let offset = interval - (now - last_run) % interval;
            debug!("setting offset using formula = {offset}s");
            offset
        };

        Schedule {
            interval: Duration::from_secs(interval),
            offset: Delay::Absolute(SystemTime::UNIX_EPOCH + Duration::from_secs(offset)),
            guild_id: GuildId::new(guild_id),
            channel_id: ChannelId::new(channel),
            message_limit: NonZeroUsize::new(message_limit.try_into().unwrap_or(usize::MAX)),
        }
    }
}
