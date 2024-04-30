use std::{fmt::Debug, time::Duration};

use async_repeater::RepeaterEntry;
use poise::serenity_prelude::{ChannelId, GuildId};

#[derive(Clone)]
pub struct Schedule {
    interval: Duration,
    offset: Option<Duration>,
    guild_id: GuildId,
    channel: ChannelId,
}

impl Schedule {
    pub fn new(interval: Duration, guild_id: GuildId, channel: ChannelId) -> Self {
        Self {
            interval,
            offset: None,
            guild_id,
            channel,
        }
    }

    pub fn with_offset(interval: Duration, guild_id: GuildId, channel: ChannelId, offset: Duration) -> Self {
        Self {
            interval,
            offset: Some(offset),
            guild_id,
            channel,
        }
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn offset(&self) -> Option<Duration> {
        self.offset
    }

    pub fn guild_id(&self) -> GuildId {
        self.guild_id
    }

    pub fn channel(&self) -> &ChannelId {
        &self.channel
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
            self.guild_id, self.interval, self.offset, self.channel
        )
    }
}
