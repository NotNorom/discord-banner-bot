use std::{sync::Arc, time::Duration};

use fred::{
    clients::RedisClient,
    interfaces::{HashesInterface, SetsInterface},
};
use poise::{
    serenity_prelude::{self, GuildId},
    Framework,
};
use reqwest::Client;
use tokio::sync::mpsc::{self, Sender};

use tracing::{error, info};
use url::Url;

use crate::{
    album_provider::Provider,
    banner_scheduler::{scheduler, ScheduleMessage},
    constants::USER_AGENT,
    database::{self, key, DbEntry},
    utils::current_unix_timestamp,
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
    redis_client: RedisClient,
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
    ) -> Result<(), mpsc::error::SendError<ScheduleMessage>> {
        let message = ScheduleMessage::new_enqueue(guild_id, album, provider, interval, offset);
        self.scheduler.send(message).await
    }

    /// Dequeue a guild
    pub async fn deque(&self, guild_id: GuildId) -> Result<(), mpsc::error::SendError<ScheduleMessage>> {
        let message = ScheduleMessage::new_dequeue(guild_id);
        self.scheduler.send(message).await
    }

    #[allow(dead_code)]
    /// Get a reference to the user data's reqwest client.
    pub fn reqw_client(&self) -> &Client {
        &self.reqw_client
    }

    /// Get a reference to the user data's redis client.
    pub fn redis_client(&self) -> &RedisClient {
        &self.redis_client
    }

    /// Gets the current album link
    pub async fn get_album(&self, guild_id: GuildId) -> Result<String, Error> {
        let db_entry = self
            .redis_client
            .hgetall::<DbEntry, _>(key(format!("{}", guild_id.0)))
            .await?;
        Ok(db_entry.album().to_string())
    }
}

/// Sets up the user data:
/// - Creates a task that handles the banner queue
/// - Sets up a reqwest client
/// - Sets up the database pool
pub async fn setup_user_data(
    ctx: &serenity_prelude::Context,
    _ready: &serenity_prelude::Ready,
    _framework: &Framework<Data, Error>,
) -> Result<Data, Error> {
    info!("Setting up user data");

    let ctx = Arc::new(ctx.clone());
    let capacity = 128;

    let (tx, rx) = mpsc::channel::<ScheduleMessage>(capacity);

    let reqw_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build()?;

    let redis_client = database::setup().await?;
    let imgur_client_id = dotenv::var("IMGUR_CLIENT_ID").expect("No imgur client id");

    let user_data = UserData {
        scheduler: tx,
        reqw_client,
        redis_client,
        imgur_client_id,
    };

    // ask for existing guild ids
    {
        let known_guild_ids: Vec<u64> = user_data.redis_client().smembers(key("known_guilds")).await?;
        info!("Look at all these IDs: {:?}", known_guild_ids);

        for id in known_guild_ids {
            // @todo: enque existing entries
            match user_data
                .redis_client()
                .hgetall::<DbEntry, _>(key(format!("{}", id)))
                .await
            {
                Ok(entry) => {
                    let album = Url::parse(entry.album()).expect("has already been parsed before");
                    let provider = Provider::try_from(&album).expect("it's been in the db already");

                    let interval = entry.interval();
                    let last_run = entry.last_run();
                    let current_time = current_unix_timestamp();
                    let offset = interval - (current_time - last_run) % interval;

                    info!("{:?}", entry);
                    info!(
                        "guild_id: {}, interval: {}, last_run: {}, current_time: {}, offset: {}",
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
                Err(e) => error!("{:?}", e),
            }
        }
    }

    info!("Spawning scheduler task");
    // Spawn the scheduler in a separate task so it can concurrently
    tokio::spawn(scheduler(ctx, rx, user_data.clone(), capacity));

    Ok(user_data)
}
