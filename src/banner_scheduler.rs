use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use fred::client::RedisClient;
use poise::serenity_prelude::GuildId;
use reqwest::{Client, Url};
use tokio::{select, sync::mpsc::Receiver};
use tokio_stream::StreamExt;
use tokio_util::time::DelayQueue;
use tracing::{error, info};

use crate::{
    album_provider::ProviderKind,
    database::{key as redis_key, DbEntry},
    guild_id_ext::RandomBanner,
};

#[derive(Debug)]
pub enum ScheduleMessage {
    /// discord guild id, album url, interval in minutes
    Enqueue(GuildId, Url, u64, ProviderKind),
    /// discord guild id
    Dequeue(GuildId),
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    guild_id: GuildId,
    album: Url,
    interval: Duration,
    provider: ProviderKind,
}

impl QueueItem {
    /// Creates a new QueueItem
    pub fn new(guild_id: GuildId, album: Url, interval: Duration, provider: ProviderKind) -> Self {
        Self {
            guild_id,
            album,
            interval,
            provider,
        }
    }

    /// Get a reference to the queue item's guild id.
    pub fn guild_id(&self) -> GuildId {
        self.guild_id
    }

    /// Get a reference to the queue item's album.
    pub fn album(&self) -> &Url {
        &self.album
    }

    /// Get a reference to the queue item's interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Get a reference to the queue item's provider.
    pub fn provider(&self) -> &ProviderKind {
        &self.provider
    }
}

pub async fn scheduler(
    ctx: Arc<poise::serenity_prelude::Context>,
    mut rx: Receiver<ScheduleMessage>,
    reqw_client: Client,
    redis_client: Arc<RedisClient>,
    capacity: usize,
) {
    let mut queue = DelayQueue::<QueueItem>::with_capacity(capacity);
    // maps the guild id to the key used in the queue
    let mut guild_id_to_key = HashMap::with_capacity(capacity);

    loop {
        // either handle an item from the queue:
        //   change the banner
        // or enqueue/ dequeue an item from the queue
        select!(
            // If a guild is ready to have their banner changed
            Some(Ok(item)) = queue.next() => {
                let inner = item.into_inner();
                let mut guild_id = inner.guild_id();
                let interval = inner.interval();
                let album = inner.album().clone();
                let provider = inner.provider().clone();

                // re-enqueue the item
                let key = queue.insert(inner, interval);
                guild_id_to_key.insert(guild_id, key);

                // get the images from the provider
                let images = match provider.images(&reqw_client, &album).await {
                    Ok(images) => images,
                    Err(e) => {
                        error!("Error: {:?}", e);
                        continue;
                    },
                };

                info!("Changing banner for {}", guild_id);

                // change the banner
                if let Err(e) = guild_id.set_random_banner(&ctx.http, &reqw_client, &images).await {
                    error!("Error: {:?}", e);
                }

                // insert into redis
                {
                    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
                    let redis_entry = DbEntry::new(guild_id.0, album.to_string(), interval.as_secs(), current_time);

                    let x: Result<(), _> = redis_client.hmset(redis_key(format!("{}", guild_id.0)), redis_entry).await;
                    info!("created new entry {:?}", x);

                    let x: Result<(), _> = redis_client.sadd(redis_key(":known_guilds"), guild_id.0.to_string()).await;
                    info!("added new guild {:?}", x)
                }
            },
            // If a guild is to be added or removed from the queue
            Some(msg) = rx.recv() => {
                match msg {
                    ScheduleMessage::Enqueue(mut guild_id, album, interval, provider) => {
                        info!("Starting schedule for: {}, with {}, every {} minutes", guild_id, album, interval);
                        // if we have a timer, cancel it
                        if let Some(key) = guild_id_to_key.get(&guild_id) {
                            queue.remove(key);
                        }

                        // change the banner manually once, before enqueing

                        // get the images from the provider
                        let images = match provider.images(&reqw_client, &album).await {
                            Ok(images) => images,
                            Err(e) => {
                                error!("Error: {:?}", e);
                                continue;
                            },
                        };

                        // change the banner
                        if let Err(e) = guild_id.set_random_banner(&ctx.http, &reqw_client, &images).await {
                            error!("Error: {:?}", e);
                        }

                        // now enqueue the new item
                        // interval is in minutes, so we multiply by 60 seconds
                        let interval = Duration::from_secs(interval * 60);
                        let key = queue.insert(QueueItem::new(guild_id, album, interval, provider), interval);
                        guild_id_to_key.insert(guild_id, key);
                    },
                    ScheduleMessage::Dequeue(guild_id) => {
                        info!("Stopping schedule for: {}", guild_id);
                        if let Some(key) = guild_id_to_key.remove(&guild_id) {
                            queue.remove(&key);
                        }
                    },
                };
            }
        );
    }
}
