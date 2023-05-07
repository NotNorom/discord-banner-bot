use std::{sync::Arc, time::Duration};

use anyhow::Context;

use poise::{
    serenity_prelude::{self, GuildId},
    Framework,
};
use reqwest::Client;
use tokio::sync::mpsc::Sender;

use tracing::info;
use url::Url;

use crate::{
    album_provider::{Album, ProviderKind},
    banner_scheduler::{ScheduleQueue, ScheduleMessage},
    constants::USER_AGENT,
    database::{guild_schedule::GuildSchedule, Database},
    settings::Settings,
    utils::{current_unix_timestamp, dm_users},
    Data, Error,
};

#[derive(Clone)]
#[allow(dead_code)]
/// The User data struct used in poise
pub struct State {
    /// Used to communicate with the scheduler without needing a &mut self
    scheduler: Sender<ScheduleMessage>,
    /// Client for http request
    reqw_client: Client,
    /// database pool
    database: Database,
    /// settings
    settings: &'static Settings,
}

impl State {
    /// Enqueue an album for the guild at interval
    pub async fn enque(
        &self,
        guild_id: GuildId,
        album: Album,
        interval: u64,
        offset: Option<u64>,
    ) -> Result<(), Error> {
        let message = ScheduleMessage::new_enqueue(guild_id, album, interval, offset);
        self.scheduler
            .send(message)
            .await
            .map_err(|err| Error::Scheduler { msg: err.0 })
    }

    /// Dequeue a guild
    pub async fn deque(&self, guild_id: GuildId) -> Result<(), Error> {
        let message = ScheduleMessage::new_dequeue(guild_id);
        self.scheduler
            .send(message)
            .await
            .map_err(|err| Error::Scheduler { msg: err.0 })
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

/// Sets up the user data:
/// - Creates a task that handles the banner queue
/// - Sets up a reqwest client
/// - Sets up the database pool
pub async fn setup(
    ctx: &serenity_prelude::Context,
    _ready: &serenity_prelude::Ready,
    framework: &Framework<Data, Error>,
) -> Result<Data, Error> {
    info!("Setting up user data");
    let settings = Settings::get();

    let ctx = Arc::new(ctx.clone());
    let capacity = 128;

    let reqw_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build()?;

    let database = Database::setup(&settings.database).await?;

    let (tx, banner_queue) = ScheduleQueue::new(
        Arc::clone(&ctx),
        framework.options().owners.clone(),
        database.clone(),
        reqw_client.clone(),
        capacity,
        settings,
    );

    let state = State {
        scheduler: tx,
        reqw_client,
        database,
        settings,
    };

    // schedule already existing guilds

    let known_guild_ids: Vec<u64> = state.database().active_schedules().await?;
    info!("Known guild id's: {:?}", known_guild_ids);

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
            " - {} enqueing with interval={}, last_run={}, current_time={}, offset={}",
            entry.guild_id(),
            interval,
            last_run,
            current_time,
            offset
        );

        state
            .enque(GuildId(entry.guild_id()), album, entry.interval(), Some(offset))
            .await?;
    }

    info!("Spawning scheduler task");
    // Spawn the scheduler in a separate task so it can concurrently
    tokio::spawn(banner_queue.scheduler());

    // Notify that we're ready
    let bot_ready = "Bot ready!";
    dm_users(&ctx, framework.options().owners.clone(), &bot_ready).await?;
    info!(bot_ready);

    Ok(state)
}
