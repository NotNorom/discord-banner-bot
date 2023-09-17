use std::{sync::Arc, time::Duration};

use anyhow::Context;

use async_repeater::{Repeater, RepeaterHandle};
use poise::{
    serenity_prelude::{self, GuildId},
    Framework,
};
use reqwest::Client;

use tracing::{error, info};
use url::Url;

use crate::{
    album_provider::{Album, ImageProviders, ProviderKind},
    banner_changer::ChangerTask,
    constants::USER_AGENT,
    database::{guild_schedule::GuildSchedule, Database},
    schedule::Schedule,
    settings::Settings,
    utils::{current_unix_timestamp, dm_users},
    Data, Error,
};

#[derive(Clone)]
#[allow(dead_code)]
/// The User data struct used in poise
pub struct State {
    /// Used to communicate with the scheduler without needing a &mut self
    repeater_handle: RepeaterHandle<Schedule>,
    /// Client for http request
    reqw_client: Client,
    /// database pool
    database: Database,
    /// settings
    settings: &'static Settings,
}

impl State {
    /// Enqueue an album for the guild at interval
    pub async fn enque(&self, schedule: Schedule) -> Result<(), Error> {
        info!("Inserting {schedule:?}");
        self.repeater_handle
            .insert(schedule)
            .await
            .map_err(|err| Error::Scheduler { msg: err.to_string() })
    }

    /// Dequeue a guild
    pub async fn deque(&self, guild_id: GuildId) -> Result<(), Error> {
        info!("Removing {guild_id:?}");
        self.repeater_handle
            .remove(guild_id)
            .await
            .map_err(|err| Error::Scheduler { msg: err.to_string() })?;
        Ok(self.database.delete::<GuildSchedule>(guild_id.0).await?)
    }

    #[allow(dead_code)]
    /// Get a reference to the user data's reqwest client.
    pub fn reqw_client(&self) -> &Client {
        &self.reqw_client
    }

    /// Get a reference to the user data's redis client.
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Gets the current album link
    pub async fn get_album(&self, guild_id: GuildId) -> Result<String, Error> {
        let db_entry = self.database.get::<GuildSchedule>(guild_id.0).await?;
        Ok(db_entry.album().to_owned())
    }
}

/// Sets up the state:
/// - Creates a task that handles the banner queue
/// - Sets up a reqwest client
/// - Sets up the database pool
pub async fn setup(
    ctx: &serenity_prelude::Context,
    _ready: &serenity_prelude::Ready,
    framework: &Framework<Data, Error>,
) -> Result<Data, Error> {
    info!("Setting up state");
    let settings = Settings::get();
    let capacity = settings.scheduler.capacity;

    let reqw_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build()?;

    let database = Database::setup(&settings.database).await?;
    let providers = Arc::new(ImageProviders::new(&settings.provider, &reqw_client));
    let repeater = Repeater::with_capacity(capacity);
    let owners = framework.options().owners.clone();
    let ctx = Arc::new(ctx.clone());

    let state = {
        let db = database.clone();
        let ctx = ctx.clone();
        let http = reqw_client.clone();
        let owners = owners.clone();

        let callback = |schedule, handle| async move {
            info!("Creating changer task for schedule {schedule:?}");
            let task = ChangerTask::new(ctx.clone(), db.clone(), http.clone(), providers, schedule);

            let Err(err) = task.run().await else {
                info!("Task finished successfully");
                return;
            };
            error!("In changer task: {err:?}");

            let Err(critical_err) = err.handle_error(ctx, handle, db, owners).await else {
                info!("Error happend and was handled successfully");
                return;
            };
            error!("CRITICAL after handling previous error: {critical_err:?}")
        };

        State {
            repeater_handle: repeater.run_with_async_callback(callback),
            reqw_client,
            database,
            settings,
        }
    };

    // schedule already existing guilds
    let known_guild_ids: Vec<u64> = state.database().active_schedules().await?;
    info!("Amount of active schedules: {}", known_guild_ids.len());

    for id in known_guild_ids {
        let entry = state.database().get::<GuildSchedule>(id).await?;

        let album_url = Url::parse(entry.album()).context("album url has already been parsed before")?;
        let kind: ProviderKind = (&album_url)
            .try_into()
            .context("provider kind has already been parsed before")?;
        let album = Album::new(album_url, kind);

        let interval = entry.interval();
        let last_run = entry.last_run();
        let current_time = current_unix_timestamp();
        let offset = interval - (current_time - last_run) % interval;

        info!(
            " - guild_id={}, interval={}, last_run={}, next_run={}, in {} seconds",
            entry.guild_id(),
            interval,
            last_run,
            current_time + offset,
            offset,
        );

        let schedule = Schedule::with_offset(
            Duration::from_secs(entry.interval()),
            GuildId(entry.guild_id()),
            album,
            Duration::from_secs(offset),
        );

        state.enque(schedule).await?;
    }

    // Notify that we're ready
    let bot_ready = "Bot ready!";
    dm_users(&ctx, owners, &bot_ready).await?;
    info!(bot_ready);

    Ok(state)
}
