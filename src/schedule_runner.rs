use std::num::NonZeroUsize;

use poise::serenity_prelude::GuildId;
use tokio_stream::StreamExt;
use tracing::{debug, error, instrument};
use url::Url;

use crate::{
    Error,
    database::{Database, guild_schedule::GuildSchedule},
    finding_media::{MediaWithMessage, find_media_in_channel},
    schedule::Schedule,
    setting_banner::RandomBanner,
};

pub struct ScheduleRunner {
    ctx: poise::serenity_prelude::Context,
    database: Database,
    http_client: reqwest::Client,
    schedule: Schedule,
}

impl ScheduleRunner {
    pub fn new(
        ctx: poise::serenity_prelude::Context,
        database: Database,
        http_client: reqwest::Client,
        schedule: Schedule,
    ) -> Self {
        Self {
            ctx,
            database,
            http_client,
            schedule,
        }
    }

    #[instrument(skip_all)]
    pub async fn run(self) -> Result<Url, RunnerError> {
        let schedule = self.schedule.clone();
        let mut guild_id = schedule.guild_id();
        let channel = schedule.channel_id();
        let limit = schedule.message_limit().map_or(usize::MAX, NonZeroUsize::get);

        debug!("Fetching images, limited to {} messages", limit);

        let stream_of_media = find_media_in_channel(&self.ctx, &channel, limit);

        let messages_with_media: Vec<MediaWithMessage> =
            stream_of_media.filter_map(Result::ok).collect::<Vec<_>>().await;

        let images = messages_with_media
            .into_iter()
            .filter_map(|media| Url::parse(&media.media).ok())
            .collect::<Vec<Url>>();

        let img_count = images.len();
        debug!("Fetched {} images. Setting banner", img_count);
        let new_banner = guild_id
            .set_random_banner(self.ctx.http.clone(), &self.http_client, &images)
            .await
            .map_err(|err| RunnerError::new(err.into(), guild_id, self.schedule.clone()))?;

        debug!("Inserting schedule into database");
        let schedule = GuildSchedule::from(schedule);

        self.database
            .insert(&schedule, schedule.guild_id())
            .await
            .map_err(|err| RunnerError::new(err.into(), guild_id, self.schedule.clone()))?;

        Ok(new_banner.to_owned())
    }
}

#[derive(Debug)]
pub enum ScheduleAction {
    Continue,
    RetrySameImage,
    RetryNewImage,
    Abort,
}

#[derive(Debug, thiserror::Error)]
#[error("{source}")]
pub struct RunnerError {
    guild_id: GuildId,
    schedule: Schedule,
    #[source]
    source: crate::Error,
}

impl RunnerError {
    pub fn new(err: Error, guild_id: GuildId, schedule: Schedule) -> Self {
        Self {
            guild_id,
            schedule,
            source: err,
        }
    }

    pub fn guild_id(&self) -> GuildId {
        self.guild_id
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn source(&self) -> &crate::Error {
        &self.source
    }
}
