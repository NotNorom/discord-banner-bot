use std::{collections::HashMap, sync::Arc, time::Duration};

use poise::{
    serenity_prelude::{self, GuildId},
    Framework,
};
use reqwest::Client;
use tokio::{
    select,
    sync::mpsc::{self, Sender},
};
use tokio_stream::StreamExt;
use tokio_util::time::DelayQueue;
use tracing::{error, info};
use url::Url;

use crate::{utils::set_random_image_for_guild, Data, Error};

#[derive(Debug)]
pub enum ScheduleMessage {
    /// discord guild id, album url, interval in minutes
    Enqueue(GuildId, Url, u64),
    /// discord guild id
    Dequeue(GuildId),
}

/// The User data struct used in poise
pub struct UserData {
    /// Used to communicate with the scheduler without needing a &mut self
    scheduler: Sender<ScheduleMessage>,
    /// Client for http request
    reqw_client: Client,
}

impl UserData {
    /// Enqueue a new new album at every interval
    pub async fn enque(
        &self,
        guild_id: GuildId,
        album: Url,
        interval: u64,
    ) -> Result<(), mpsc::error::SendError<ScheduleMessage>> {
        let message = ScheduleMessage::Enqueue(guild_id, album, interval);
        self.scheduler.send(message).await
    }

    /// Dequeue a guild
    pub async fn deque(
        &self,
        guild_id: GuildId,
    ) -> Result<(), mpsc::error::SendError<ScheduleMessage>> {
        let message = ScheduleMessage::Dequeue(guild_id);
        self.scheduler.send(message).await
    }
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    guild_id: GuildId,
    album: Url,
    interval: Duration,
}

impl QueueItem {
    pub fn new(guild_id: GuildId, album: Url, interval: Duration) -> Self {
        Self {
            guild_id,
            album,
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
}

pub async fn setup_user_data(
    ctx: &serenity_prelude::Context,
    _ready: &serenity_prelude::Ready,
    _framework: &Framework<Data, Error>,
) -> Result<Data, Error> {
    let ctx = Arc::new(ctx.clone());
    let capacity = 128;

    let (tx, mut rx) = mpsc::channel::<ScheduleMessage>(capacity);

    let user_data_reqw_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Discord Banner Bot")
        .build()?;

    let reqw_client = Clone::clone(&user_data_reqw_client);

    tokio::spawn(async move {
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

                    info!("Changing the banner for {}", guild_id);

                    // change the banner
                    if let Err(e) = set_random_image_for_guild(
                        &ctx.http,
                        &reqw_client,
                        &mut guild_id,
                        &inner.album()).await
                    {
                        error!("Error: {:?}", e);
                    };

                    // re-enqueue the item
                    let key = queue.insert(inner, interval);
                    guild_id_to_key.insert(guild_id, key);
                },
                // If a guild is to be added or removed from the queue
                Some(msg) = rx.recv() => {
                    
                    match msg {
                        // todo: what happens if this is called twice without a
                        //   dequeue in-between? Should I cancel the existing entry
                        //   and reschedule?
                        ScheduleMessage::Enqueue(guild_id, album, interval) => {
                            info!("Starting schedule for: {}, with {}, every {} minutes", guild_id, album, interval * 60);
                            // if we have a timer, cancel it
                            if let Some(key) = guild_id_to_key.get(&guild_id) {
                                queue.remove(key);
                            }

                            // now enqueue the new item
                            // interval is in minutes, so we multiply by 60 seconds
                            let interval = Duration::from_secs(interval);
                            let key = queue.insert(QueueItem::new(guild_id, album, interval), interval);
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
    });

    Ok(UserData {
        scheduler: tx,
        reqw_client: user_data_reqw_client,
    })
}
