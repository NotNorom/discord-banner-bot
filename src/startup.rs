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
    album_provider::Provider,
    banner_scheduler::{BannerQueue, ScheduleMessage},
    constants::{self, USER_AGENT},
    database::{Database, GuildSchedule},
    utils::{current_unix_timestamp, dm_users},
    Data, Error,
};

#[derive(Clone)]
#[allow(dead_code)]
/// The User data struct used in poise
pub struct UserData {
    /// Used to communicate with the scheduler without needing a &mut self
    scheduler: Sender<ScheduleMessage>,
    /// Client for http request
    reqw_client: Client,
    /// database pool
    database: Database,
    /// imgur_client_id
    imgur_client_id: String,
}

impl UserData {
    /// Enqueue a new new album at every interval
    pub async fn enque(
        &self,
        guild_id: GuildId,
        album: Url,
        provider: Provider,
        interval: u64,
        offset: Option<u64>,
    ) -> Result<(), Error> {
        let message = ScheduleMessage::new_enqueue(guild_id, album, provider, interval, offset);
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
        Ok(db_entry.album().to_string())
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

    let ctx = Arc::new(ctx.clone());
    let capacity = 128;

    let reqw_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build()?;

    let database = Database::setup(constants::REDIS_PREFIX).await?;
    let imgur_client_id = dotenv::var("IMGUR_CLIENT_ID")?;

    let (tx, banner_queue) = BannerQueue::new(
        ctx.clone(),
        framework.options().owners.clone(),
        database.clone(),
        reqw_client.clone(),
        capacity,
    );

    let user_data = UserData {
        scheduler: tx,
        reqw_client,
        database,
        imgur_client_id,
    };

    // ask for existing guild ids
    {
        let known_guild_ids: Vec<u64> = user_data.database().known_guilds().await?;
        info!("Known guild id's: {:?}", known_guild_ids);

        for id in known_guild_ids {
            let entry = user_data.database().get::<GuildSchedule>(id).await?;

            let album = Url::parse(entry.album()).context("has already been parsed before")?;
            let provider = Provider::try_from(&album).context("it's been in the db already")?;

            let interval = entry.interval();
            let last_run = entry.last_run();
            let current_time = current_unix_timestamp();
            let offset = interval - (current_time - last_run) % interval;

            info!(
                " - {} enqueing with interval={}, last_run={}, current_time={}, offset{}",
                entry.guild_id(),
                interval,
                last_run,
                current_time,
                offset
            );

            let _ = user_data
                .enque(
                    GuildId(entry.guild_id()),
                    album,
                    provider,
                    entry.interval(),
                    Some(offset),
                )
                .await;
        }
    }

    info!("Spawning scheduler task");
    // Spawn the scheduler in a separate task so it can concurrently
    tokio::spawn(banner_queue.scheduler());

    // Notify that we're ready
    let bot_ready = "Bot ready!";
    dm_users(&ctx, framework.options().owners.clone(), &bot_ready).await?;
    info!(bot_ready);

    Ok(user_data)
}
