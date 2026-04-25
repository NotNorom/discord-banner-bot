use std::{num::NonZeroU32, sync::Arc, time::Duration};

use poise::serenity_prelude::{self, GuildId, Message};
use tokio::{
    pin,
    time::{sleep, timeout},
};
use tokio_stream::StreamExt;
use tracing::{debug, error, info, instrument};
use url::Url;

use crate::{
    Error, State,
    database::{Database, guild_schedule::GuildSchedule},
    error::evaluate_schedule_error,
    finding_media::find_media_in_channel,
    schedule::Schedule,
    setting_banner::{BannerFromUrl, RandomBanner, SetBannerError},
    utils::dm_users,
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

    /// This is run every time a schedule is due
    #[instrument(skip_all)]
    pub async fn run(
        &self,
        avoid_list: &[Url],
        pick_this: Option<(Url, Message)>,
    ) -> Result<Url, RunnerError> {
        let schedule = self.schedule.clone();
        let mut guild_id = schedule.guild_id();

        // if we have an override image given, just use it and skip the rest of the function
        if let Some((url, message)) = pick_this {
            debug!("Using override image: {url}");
            guild_id
                .set_banner_from_url_and_message(self.ctx.http.clone(), &self.http_client, &url, &message)
                .await
                .map_err(|err| RunnerError::new(err.into(), guild_id, self.schedule.clone()))?;
            debug!("Inserting schedule into database");
            let schedule = GuildSchedule::from(schedule.clone());

            self.database
                .insert(&schedule, schedule.guild_id())
                .await
                .map_err(|err| RunnerError::new(err.into(), guild_id, self.schedule.clone()))?;
            return Ok(url);
        };

        let channel = schedule.channel_id();
        let limit = schedule.message_limit().map_or(u32::MAX, NonZeroU32::get);

        debug!("Fetching images, limited to {} messages", limit);

        let stream_of_media = find_media_in_channel(&self.ctx, &channel, limit as usize);
        pin!(stream_of_media);

        let mut images = Vec::new();
        while let Some(url_message_pair) = stream_of_media
            .try_next()
            .await
            .map_err(|err| RunnerError::new(err.into(), guild_id, self.schedule.clone()))?
            .map(|media| {
                (
                    Url::parse(&media.media).expect("every media should have a valid url"),
                    media.message,
                )
            })
        {
            if avoid_list.contains(&url_message_pair.0) {
                continue;
            }

            images.push(url_message_pair);
        }

        let img_count = images.len();
        debug!("Fetched {} images. Setting banner", img_count);
        let new_banner = guild_id
            .set_random_banner_with_message(self.ctx.http.clone(), &self.http_client, &images)
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
    /// Everything's fine
    Continue,
    /// Use this url next run
    RetrySameImage,
    /// Avoid this image
    RetryNewImage,
    /// OH GOD WHY
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

    pub fn attempted_url_and_message(&self) -> (Option<Url>, Option<Box<Message>>) {
        match &self.source {
            Error::SetBanner(set_banner_error) => match set_banner_error {
                SetBannerError::Transport(_) => (None, None),
                SetBannerError::DiscordApi(_) => (None, None),
                SetBannerError::CouldNotPickAUrl => (None, None),
                SetBannerError::CouldNotDeterminFileExtension(url) => (Some(url.clone()), None),
                SetBannerError::MissingBannerFeature => (None, None),
                SetBannerError::MissingAnimatedBannerFeature(url, message) => {
                    (Some(url.clone()), Some(message.clone()))
                }
                SetBannerError::ImageIsEmpty(url, message) => (Some(url.clone()), Some(message.clone())),
                SetBannerError::ImageIsTooBig(url, message) => (Some(url.clone()), Some(message.clone())),
                SetBannerError::ImageUnkownSize(url, message) => (Some(url.clone()), Some(message.clone())),
                SetBannerError::Base64Encoding(url, message) => (Some(url.clone()), Some(message.clone())),
            },
            _ => (None, None),
        }
    }

    pub fn source(&self) -> &crate::Error {
        &self.source
    }
}

pub async fn schedule_callback(ctx: serenity_prelude::Context, schedule: Schedule) {
    let state: Arc<State> = ctx.data();
    let task = ScheduleRunner::new(
        ctx.clone(),
        state.database().to_owned(),
        state.reqw_client().to_owned(),
        schedule.clone(),
    );

    let mut retries_left = 3;
    let mut avoid_list = Vec::with_capacity(1);
    let mut override_url = None;

    while retries_left > 0 {
        retries_left -= 1;

        // run the actual task of changing the banner
        let timeout_result = timeout(
            Duration::from_secs(60),
            task.run(&avoid_list, override_url.clone()),
        )
        .await;

        let result = match timeout_result {
            Ok(res) => res,
            Err(_) => Err(RunnerError::new(
                Error::Timeout {
                    action: "Banner changer task".to_string(),
                },
                schedule.guild_id(),
                schedule.clone(),
            )),
        };

        let Err(err) = result else {
            debug!("Task finished successfully");
            return;
        };

        error!("Task had an error: {err:?}");

        let error_handling_result = evaluate_schedule_error(&err, ctx.clone(), state.owners()).await;

        match error_handling_result {
            Ok(action) => {
                info!("Error was handled successfully. Recommended action={action:?}");
                match action {
                    ScheduleAction::Continue => return,
                    ScheduleAction::RetrySameImage => {
                        if let (Some(url), Some(message)) = err.attempted_url_and_message() {
                            override_url = Some((url, *message));
                        }
                    }
                    ScheduleAction::RetryNewImage => {
                        if let (Some(url), Some(_)) = err.attempted_url_and_message() {
                            avoid_list.push(url);
                        }
                    }
                    ScheduleAction::Abort => {
                        let _ = state.deque(schedule.guild_id()).await;
                        return;
                    }
                }
            }
            Err(critical_err) => {
                let message = format!("CRITICAL ERROR schedule={schedule:?}: {critical_err:?}");
                error!(message);
                // if we encounter an error _now_ it's over anyways
                let _ = state.deque(schedule.guild_id()).await;
                let _ = dm_users(&ctx, state.owners(), &message).await;

                return;
            }
        }

        // don't retry instantly, give it a little tiiiime
        sleep(Duration::from_secs(3)).await;
    }
}
