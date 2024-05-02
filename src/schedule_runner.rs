use std::sync::Arc;

use poise::serenity_prelude::GuildId;

use tokio_stream::StreamExt;
use tracing::{error, info};
use url::Url;

use crate::{
    database::{guild_schedule::GuildSchedule, Database},
    finding_media::{find_media_in_channel, MediaWithMessage},
    schedule::Schedule,
    setting_banner::RandomBanner,
    utils::current_unix_timestamp,
    Error,
};

pub struct ScheduleRunner {
    ctx: Arc<poise::serenity_prelude::Context>,
    database: Database,
    http_client: reqwest::Client,
    schedule: Schedule,
}

impl ScheduleRunner {
    pub fn new(
        ctx: Arc<poise::serenity_prelude::Context>,
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

    pub async fn run(self) -> Result<(), RunnerError> {
        let schedule = self.schedule.clone();
        let mut guild_id = schedule.guild_id();
        let channel = schedule.channel();
        let interval = schedule.interval();

        info!("Fetching images");

        let messages_with_media: Vec<MediaWithMessage> = find_media_in_channel(&self.ctx, channel)
            .take(100)
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .await;

        let images = messages_with_media
            .into_iter()
            .filter_map(|media| Url::parse(&media.media).ok())
            .collect::<Vec<Url>>();

        let img_count = images.len();
        info!("Fetched {} images. Setting banner", img_count);
        guild_id
            .set_random_banner(self.ctx.http.clone(), &self.http_client, &images)
            .await
            .map_err(|err| RunnerError::new(err.into(), guild_id, self.schedule.clone()))?;

        info!("Inserting schedule into database");
        let schedule = GuildSchedule::new(
            guild_id.get(),
            channel.get(),
            interval.as_secs(),
            current_unix_timestamp(),
        );

        self.database
            .insert(&schedule, schedule.guild_id())
            .await
            .map_err(|err| RunnerError::new(err.into(), guild_id, self.schedule.clone()))?;

        Ok(())
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
