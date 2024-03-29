use std::{collections::HashSet, sync::Arc};

use async_repeater::RepeaterHandle;
use poise::serenity_prelude::{
    Context, Error as SerenityError, GuildId, HttpError as SerenityHttpError, MessageBuilder, UserId,
};
use reqwest::StatusCode;

use tracing::{error, info, warn};

use crate::{
    album_provider::{ImageProviders, ProviderError},
    database::{guild_schedule::GuildSchedule, Database},
    guild_id_ext::{RandomBanner, SetBannerError},
    schedule::Schedule,
    utils::{current_unix_timestamp, dm_user, dm_users},
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
            .map_err(|err| ChangerError::new(err.into(), guild_id, self.schedule.clone()))?;

        let img_count = images.len();
        info!("Fetched {} images. Setting banner", img_count);
        guild_id
            .set_random_banner(self.ctx.http.clone(), &self.http_client, &images)
            .await
            .map_err(|err| ChangerError::new(err.into(), guild_id, self.schedule.clone()))?;

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
            .map_err(|err| ChangerError::new(err.into(), guild_id, self.schedule.clone()))?;

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
    source: crate::Error,
}

impl ChangerError {
    pub fn new(err: Error, guild_id: GuildId, schedule: Schedule) -> Self {
        Self {
            guild_id,
            schedule,
            source: err,
        }
    }

    /// Handle scheduler related errors
    ///
    /// This is a needed as well as the normal error handling in [crate::error::on_error] because
    /// the scheduler is running in its own task
    #[tracing::instrument(skip(self, ctx, db, repeater_handle, owners))]
    pub async fn handle_error(
        &self,
        ctx: Arc<Context>,
        repeater_handle: RepeaterHandle<Schedule>,
        db: Database,
        owners: HashSet<UserId>,
    ) -> Result<ScheduleAction, Error> {
        let guild_id = self.schedule.guild_id();

        let guild_name = format!("{guild_id}: {}", guild_id.name(&ctx).unwrap_or_default());

        let message = MessageBuilder::new()
            .push_bold("Error in guild: ")
            .push_mono_line_safe(&guild_name)
            .push_codeblock(self.to_string(), Some("rust"))
            .build();

        dm_users(&ctx, owners.clone(), &message).await?;

        match &self.source {
            Error::Serenity(error) => match error {
                SerenityError::Http(error) => match error.as_ref() {
                    SerenityHttpError::UnsuccessfulRequest(error_response) => {
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
                                db.delete::<GuildSchedule>(self.schedule.guild_id().0).await?;
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
            Error::Provider(error) => match error {
                ProviderError::Unsupported(name) => error!("Unsupported provider kind: {name}"),
                ProviderError::Inactive(kind) => error!("Inactive provider: {kind:?}"),
                ProviderError::ImgurIdExtraction(error) => error!("Could not extract imgur id: {error}"),
                ProviderError::Imgur(error) => match error {
                    imgurs::Error::SendApiRequest(send_api_err) => {
                        warn!("Error with imgur request: {send_api_err:#?}");
                    }
                    imgurs::Error::ApiError(code, message) => {
                        warn!("Imgur responded with status_code={code} and message={message}");
                    }
                    imgurs_err => error!("unhandled imgurs error: {imgurs_err}"),
                },
                ProviderError::InvalidDomain(url) => {
                    warn!("Command was called with invalid domain: {url}");
                }
            },
            Error::SetBanner(error) => {
                match error {
                    SetBannerError::Transport(err) => {
                        warn!("guild_id={guild_id}: {err}")
                    }
                    SetBannerError::DiscordApi(discord_err) => match discord_err {
                        SerenityError::Http(http_err) => match http_err.as_ref() {
                            SerenityHttpError::UnsuccessfulRequest(error_response) => {
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
                                        db.delete::<GuildSchedule>(self.schedule.guild_id().0).await?;
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
                            http_err => error!("unhandled http error in set_banner: {http_err:?}"),
                        },
                        serenity_err => error!("unhandled serenity error in set_banner: {serenity_err:?}"),
                    },
                    SetBannerError::CouldNotPickAUrl => warn!("guild_id={guild_id}: 'Could not pick a url'"),
                    SetBannerError::CouldNotDeterminFileExtension => {
                        warn!("guild_id={guild_id}: 'Could not determine file extenstion'")
                    }
                    SetBannerError::MissingBannerFeature => {
                        let _ = repeater_handle.remove(guild_id).await;
                        db.delete::<GuildSchedule>(self.schedule.guild_id().0).await?;

                        let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                        let guild_owner = partial_guild.owner_id;
                        info!("Letting owner={guild_owner} of guild={guild_id} know about the missing banner feature");

                        dm_user(&ctx, guild_owner, &"Server has lost the required boost level. Stopping schedule. You can restart the bot after gaining the required boost level.").await?;
                    }
                    SetBannerError::ImageIsEmpty(url) => {
                        warn!("guild_id={guild_id} with album={} has downloaded an image with 0 bytes. url={url}", self.schedule.album());
                    }
                    SetBannerError::ImageIsTooBig(url) => {
                        warn!("guild_id={guild_id} with album={} has downloaded an image that is too big. url={url}", self.schedule.album());

                        let partial_guild = guild_id.to_partial_guild(&ctx.http).await?;
                        let guild_owner = partial_guild.owner_id;
                        info!("Letting owner={guild_owner} of guild={guild_id} know about an image that is too big");

                        dm_user(&ctx, guild_owner, &format!("The album you've set contains an image that is too big for discord. Maximum size is 10mb. The image is: {url}")).await?;
                    }
                }
            }
            err => {
                error!("unhandled bot error: {err:?}");
            }
        }

        Ok(ScheduleAction::Continue)
    }
}
