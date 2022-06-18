use std::{collections::HashMap, sync::Arc, time::Duration};

use fred::interfaces::{HashesInterface, KeysInterface, SetsInterface};
use poise::serenity_prelude::GuildId;
use reqwest::Url;
use tokio::{select, sync::mpsc::Receiver};
use tokio_stream::StreamExt;
use tokio_util::time::{delay_queue::Key, DelayQueue};
use tracing::{error, info};

use crate::{
    album_provider::Provider,
    database::{key as redis_key, DbEntry},
    guild_id_ext::RandomBanner,
    utils::timestamp_seconds,
    Data, Error,
};

#[derive(Debug)]
pub struct ScheduleMessageEnqueue {
    guild_id: GuildId,
    album: Url,
    interval: u64,
    provider: Provider,
    offset: u64,
}

#[derive(Debug)]
pub enum ScheduleMessage {
    /// discord guild id, album url, interval in minutes
    Enqueue(ScheduleMessageEnqueue),
    /// discord guild id
    Dequeue(GuildId),
}

impl ScheduleMessage {
    pub fn new_enqueue(
        guild_id: GuildId,
        album: Url,
        provider: Provider,
        interval: u64,
        offset: Option<u64>,
    ) -> Self {
        Self::Enqueue(ScheduleMessageEnqueue {
            guild_id,
            album,
            provider,
            interval,
            offset: offset.unwrap_or_default(),
        })
    }

    pub fn new_dequeue(guild_id: GuildId) -> Self {
        Self::Dequeue(guild_id)
    }
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    guild_id: GuildId,
    album: Url,
    provider: Provider,
    interval: Duration,
}

impl QueueItem {
    /// Creates a new QueueItem
    pub fn new(guild_id: GuildId, album: Url, provider: Provider, interval: Duration) -> Self {
        Self {
            guild_id,
            album,
            provider,
            interval,
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
    pub fn provider(&self) -> &Provider {
        &self.provider
    }
}

pub async fn scheduler(
    ctx: Arc<poise::serenity_prelude::Context>,
    mut rx: Receiver<ScheduleMessage>,
    user_data: Data,
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
            Some(item) = queue.next() => {
                let inner = item.into_inner();
                let mut guild_id = inner.guild_id();
                let interval = inner.interval();
                let album = inner.album().clone();
                let provider = inner.provider().clone();

                // re-enqueue the item
                let key = queue.insert(inner, interval);
                guild_id_to_key.insert(guild_id, key);

                // get the images from the provider
                let images = match provider.images(user_data.reqw_client(), &album).await {
                    Ok(images) => images,
                    Err(e) => {
                        error!("Error: {:?}", e);
                        continue;
                    },
                };

                info!("Changing banner for {}", guild_id);

                // creating the redis entry just before the banner is set,
                // because the timestamp must be when we _start_ setting the banner,
                // not when the command finally returns from discord (which might take a few seconds)
                let redis_entry = DbEntry::new(guild_id.0, album.to_string(), interval.as_secs(), timestamp_seconds());

                // change the banner
                if let Err(e) = guild_id.set_random_banner(&ctx.http, user_data.reqw_client(), &images).await {
                    error!("Error: {:?}", e);
                }

                // insert into redis
                let _: Result<(), _> = user_data.redis_client().hmset(redis_key(format!("{}", guild_id.0)), &redis_entry).await;
                let _: Result<(), _> = user_data.redis_client().sadd(redis_key("known_guilds"), guild_id.0.to_string()).await;
                info!("updated entry {:?}", redis_entry);
            },
            // If a guild is to be added or removed from the queue
            Some(msg) = rx.recv() => {
                if let Err(e) = match msg {
                    ScheduleMessage::Enqueue(enqueue_msg) => enqueue(ctx.clone(), user_data.clone(), enqueue_msg, &mut queue, &mut guild_id_to_key, ).await,
                    ScheduleMessage::Dequeue(guild_id) => dequeue(ctx.clone(), user_data.clone(), guild_id, &mut queue, &mut guild_id_to_key).await,
                } {
                    error!("{:?}", e);
                }
            }
        );
    }
}

async fn enqueue(
    ctx: Arc<poise::serenity_prelude::Context>,
    user_data: Data,
    enqueue_msg: ScheduleMessageEnqueue,
    queue: &mut DelayQueue<QueueItem>,
    guild_id_to_key: &mut HashMap<GuildId, Key>,
) -> Result<(), Error> {
    let ScheduleMessageEnqueue {
        mut guild_id,
        album,
        interval,
        provider,
        offset,
    } = enqueue_msg;
    info!(
        "Starting schedule for: {}, with {}, every {:>6} seconds. Next run at: {}",
        guild_id,
        album,
        interval,
        timestamp_seconds() + offset
    );

    // if we have a timer, cancel it
    if let Some(key) = guild_id_to_key.get(&guild_id) {
        queue.remove(key);
    }

    // change the banner manually once, before enqueing

    // get the images from the provider
    let images = provider.images(user_data.reqw_client(), &album).await?;

    // change the banner
    if let Err(e) = guild_id
        .set_random_banner(&ctx.http, user_data.reqw_client(), &images)
        .await
    {
        error!("Error: {:?}", e);
    }

    // now enqueue the new item
    let interval = Duration::from_secs(interval);
    let offset = Duration::from_secs(offset);
    let key = queue.insert(
        QueueItem::new(guild_id, album.clone(), provider, interval),
        offset,
    );
    guild_id_to_key.insert(guild_id, key);

    Ok(())
}

async fn dequeue(
    _ctx: Arc<poise::serenity_prelude::Context>,
    user_data: Data,
    guild_id: GuildId,
    queue: &mut DelayQueue<QueueItem>,
    guild_id_to_key: &mut HashMap<GuildId, Key>,
) -> Result<(), Error> {
    if let Some(key) = guild_id_to_key.remove(&guild_id) {
        queue.remove(&key);
    }

    let _: Result<(), _> = user_data
        .redis_client()
        .del(redis_key(format!("{}", guild_id.0)))
        .await;

    let _: Result<(), _> = user_data
        .redis_client()
        .srem(redis_key("known_guilds"), guild_id.0.to_string())
        .await;
    info!("Removed guild {:?}", &guild_id.0);

    Ok(())
}
