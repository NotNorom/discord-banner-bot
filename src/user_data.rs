use std::{sync::Arc, time::Duration};

use fred::client::RedisClient;
use poise::{
    serenity_prelude::{self, GuildId},
    Framework,
};
use reqwest::Client;
use tokio::sync::mpsc::{self, Sender};

use tracing::info;
use url::Url;

use crate::{
    album_provider::ProviderKind,
    banner_scheduler::{scheduler, ScheduleMessage},
    constants::USER_AGENT,
    database::{self, key},
    Data, Error,
};

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
        interval: u64,
        provider: ProviderKind,
    ) -> Result<(), mpsc::error::SendError<ScheduleMessage>> {
        let message = ScheduleMessage::Enqueue(guild_id, album, interval, provider);
        self.scheduler.send(message).await
    }

    /// Dequeue a guild
    pub async fn deque(&self, guild_id: GuildId) -> Result<(), mpsc::error::SendError<ScheduleMessage>> {
        let message = ScheduleMessage::Dequeue(guild_id);
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

    let user_data_reqw_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build()?;

    let reqw_client = Clone::clone(&user_data_reqw_client);

    let redis_client = database::setup().await?;

    info!("Spawning scheduler task");
    // Spawn the scheduler in a separate task so it can concurrently
    tokio::spawn(scheduler(ctx, rx, reqw_client, redis_client.clone(), capacity));

    // ask for existing guild ids
    {
        let known_guild_ids: Vec<u64> = redis_client.smembers(key(":known_guilds")).await?;

        for id in known_guild_ids {
            // @todo: enque existing entries
            let entry = redis_client.hgetall(key(format!(":{}", id))).await?;
            info!("{:?}", entry);
        }
    }

    let imgur_client_id = dotenv::var("IMGUR_CLIENT_ID").expect("No imgur client id");

    Ok(UserData {
        scheduler: tx,
        reqw_client: user_data_reqw_client,
        redis_client,
        imgur_client_id,
    })
}
