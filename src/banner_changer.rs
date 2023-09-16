use std::{collections::HashSet, sync::Arc};

use async_repeater::RepeaterHandle;
use poise::serenity_prelude::{Context, GuildId, MessageBuilder, UserId};
use reqwest::StatusCode;

use tracing::{error, info, warn};

use crate::{
    album_provider::{ImageProviders, ProviderError},
    database::{guild_schedule::GuildSchedule, Database},
    guild_id_ext::RandomBanner,
    schedule::Schedule,
    utils::{current_unix_timestamp, dm_users},
    Error,
};

pub enum ScheduleAction {
    Continue,
    Retry,
    Abort,
}

pub struct ChangerTask {
    ctx: Arc<poise::serenity_prelude::Context>,
    database: Database,
    http_client: reqwest::Client,
    providers: Arc<ImageProviders>,
    schedule: Schedule,
}

impl ChangerTask {
    pub async fn run(self) -> Result<(), ChangerError> {
        let schedule = self.schedule.clone();
        let mut guild_id = schedule.guild_id();
        let album = schedule.album();
        let interval = schedule.interval();

        info!("Fetching images");
        let images = self
            .providers
            .images(album)
            .await
            .map_err(|err| ChangerError::new(err, guild_id, self.schedule.clone()))?;

        let img_count = images.len();
        info!("Fetched {} images. Setting banner", img_count);
        guild_id
            .set_random_banner(self.ctx.http.clone(), &self.http_client, &images)
            .await
            .map_err(|err| ChangerError::new(err, guild_id, self.schedule.clone()))?;

        info!("Inserting schedule into database");
        let schedule = GuildSchedule::new(
            guild_id.0,
            album.url().to_string(),
            interval.as_secs(),
            current_unix_timestamp(),
        );

        self.database
            .insert(&schedule, schedule.guild_id())
            .await
            .map_err(|err| ChangerError::new(err, guild_id, self.schedule.clone()))?;

        Ok(())
    }
}

impl ChangerTask {
    pub fn new(
        ctx: Arc<poise::serenity_prelude::Context>,
        database: Database,
        http_client: reqwest::Client,
        providers: Arc<ImageProviders>,
        schedule: Schedule,
    ) -> Self {
        Self {
            ctx,
            database,
            http_client,
            providers,
            schedule,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{source}")]
pub struct ChangerError {
    guild_id: GuildId,
    schedule: Schedule,
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl ChangerError {
    pub fn new(
        err: impl std::error::Error + Send + Sync + 'static,
        guild_id: GuildId,
        schedule: Schedule,
    ) -> Self {
        Self {
            guild_id,
            schedule,
            source: Box::new(err),
        }
    }

    /// Handle scheduler related errors
    ///
    /// This is a needed as well as the normal error handling in [crate::error::on_error] because
    /// the scheduler is running in its own task
    #[tracing::instrument(skip(self, ctx, db))]
    pub async fn handle_error(
        &self,
        ctx: Arc<Context>,
        repeater_handle: RepeaterHandle<Schedule>,
        db: Database,
        owners: HashSet<UserId>,
    ) -> Result<ScheduleAction, Error> {
        use poise::serenity_prelude;

        let guild_id = self.schedule.guild_id();

        let guild_name = format!("{guild_id}: {}", guild_id.name(&ctx).unwrap_or_default());

        let message = MessageBuilder::new()
            .push_bold("Error in guild: ")
            .push_mono_line_safe(&guild_name)
            .push_codeblock(self.to_string(), Some("rust"))
            .build();

        dm_users(&ctx, owners.clone(), &message).await?;

        match self.source.as_ref().downcast_ref::<crate::Error>().unwrap() {
            Error::Serenity(error) => match error {
                serenity_prelude::Error::Http(error) => match error.as_ref() {
                    serenity_prelude::HttpError::UnsuccessfulRequest(error_response) => {
                        match error_response.status_code {
                            StatusCode::FORBIDDEN => {
                                // the bot does not have permissions to change the banner.
                                // remove guild from queue
                                let _ = repeater_handle.remove(guild_id).await;
                                db.delete::<GuildSchedule>(self.schedule.guild_id().0).await?;
                                warn!("Missing permissions to change banner for {guild_id}. Unscheduling.");
                                return Ok(ScheduleAction::Abort);
                            }
                            StatusCode::NOT_FOUND => {
                                let _ = repeater_handle.remove(guild_id).await;
                                warn!("Guild does not exist: {guild_id}. Unscheduling.");
                                return Ok(ScheduleAction::Abort);
                            }
                            StatusCode::GATEWAY_TIMEOUT => {
                                warn!("Gateway timed out. Retrying once.");
                                return Ok(ScheduleAction::Retry);
                            }
                            _ => error!("unsuccessful http request: {error_response:?}"),
                        }
                    }
                    http_err => error!("unhandled http error: {http_err:?}"),
                },
                serenity_err => error!("unhandled serenity error: {serenity_err:?}"),
            },
            Error::SchedulerTask(error) => match error {
                crate::error::SchedulerTask::GuildHasNoAnimatedBannerFeature => {
                    let _ = repeater_handle.remove(guild_id).await;
                    db.delete::<GuildSchedule>(self.schedule.guild_id().0).await?;
                    warn!("Trying to schedule, but guild has no animated banner feature. Removing schedule.");
                }
                crate::error::SchedulerTask::GuildHasNoBannerFeature => {
                    let _ = repeater_handle.remove(guild_id).await;
                    db.delete::<GuildSchedule>(self.schedule.guild_id().0).await?;
                    warn!("Trying to schedule, but guild has no banner feature. Removing schedule.");
                } // command_err => error!("unhandled scheduler task error: {command_err:?}"),
            },
            Error::Provider(error) => match error {
                ProviderError::Unsupported(name) => error!("Unsupported provider kind: {name}"),
                ProviderError::Inactive(kind) => error!("Inactive provider: {kind:?}"),
                ProviderError::ImgurIdExtraction(error) => error!("Could not extract imgur id: {error}"),
                ProviderError::Imgur(error) => match error {
                    imgurs::Error::SendApiRequest(send_api_err) => {
                        warn!("Error with imgur request: {send_api_err:#?}");
                    }
                    imgurs_err => error!("unhandled imgurs error: {imgurs_err}"),
                },
            },
            err => {
                error!("unhandled bot error: {err:?}");
            }
        }

        Ok(ScheduleAction::Continue)
    }
}
