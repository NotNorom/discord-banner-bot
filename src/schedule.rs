use std::{fmt::Debug, time::Duration};

use async_repeater::RepeaterEntry;
use poise::serenity_prelude::GuildId;

use crate::album_provider::Album;

#[derive(Clone)]
pub struct Schedule {
    interval: Duration,
    offset: Option<Duration>,
    guild_id: GuildId,
    album: Album,
}

impl Schedule {
    pub fn new(interval: Duration, guild_id: GuildId, album: Album) -> Self {
        Self {
            interval,
            offset: None,
            guild_id,
            album,
        }
    }

    pub fn with_offset(interval: Duration, guild_id: GuildId, album: Album, offset: Duration) -> Self {
        Self {
            interval,
            offset: Some(offset),
            guild_id,
            album,
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

    pub fn album(&self) -> &Album {
        &self.album
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
        self.offset = None
    }
}

impl Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Schedule(guild_id = {}, interval = {:?}, offset = {:?}, album = {}",
            self.guild_id,
            self.interval,
            self.offset,
            self.album.url()
        )
    }
}
