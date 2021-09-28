use std::{collections::HashMap, sync::Arc, time::Duration};

use poise::{
    serenity_prelude::{self, GuildId},
    Framework,
};
use tokio::{
    select,
    sync::mpsc::{self, Sender},
};
use tokio_stream::StreamExt;
use tokio_util::time::DelayQueue;
use url::Url;

use crate::{utils::set_random_image_for_guild, Data, Error};

#[derive(Debug)]
pub enum ScheduleMessage {
    /// discord guild id, album url, interval in minutes
    Enqueue(GuildId, Url, u64),
    /// discord guild id
    Dequeue(GuildId),
}

pub struct UserData {
    scheduler: Sender<ScheduleMessage>,
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

#[derive(Debug)]
pub(crate) struct QueueItem {
    guild_id: GuildId,
    album: Url,
}

impl QueueItem {
    pub(crate) fn new(guild_id: GuildId, album: Url) -> Self {
        Self { guild_id, album }
    }

    /// Get a reference to the queue item's guild id.
    pub(crate) fn guild_id(&self) -> &GuildId {
        &self.guild_id
    }

    /// Get a reference to the queue item's album.
    pub(crate) fn album(&self) -> &Url {
        &self.album
    }
}

pub async fn setup_user_data(
    ctx: &serenity_prelude::Context,
    _ready: &serenity_prelude::Ready,
    _framework: &Framework<Data, Error>,
) -> Result<Data, Error> {
    let ctx = Arc::new(ctx.clone());

    let (tx, mut rx) = mpsc::channel::<ScheduleMessage>(128);

    tokio::spawn(async move {
        let ctx = Arc::clone(&ctx);

        let capacity = 128;
        let mut queue = DelayQueue::<QueueItem>::with_capacity(capacity);
        let mut id_to_key = HashMap::with_capacity(capacity);

        loop {
            // either handle an item from the queue:
            //   change the banner
            // or enqueue/ dequeue an item from the queue
            select!(
                // If a guild is ready to have their banner changed
                Some(Ok(item)) = queue.next() => {
                    println!("Queue entry: {:?}", &item);
                    let inner = item.get_ref();
                    if let Err(e) = set_random_image_for_guild(ctx.http.as_ref(), inner.guild_id(), inner.album()).await {
                        eprintln!("Error: {:?}", e);
                    };
                    // todo: re-enqueue item with the same interval it has been enqueued with,
                    //   so uhhh, item.deadline() doesn't work... guess'll have to save
                    //   the interval in the QueueItem
                },
                // If a guild is to be added or removed from the queue
                Some(msg) = rx.recv() => {
                    println!("Message for the queue: {:#?}", &msg);
                    match msg {
                        // todo: what happens if this is called twice without a
                        //   dequeue in-between? Should I cancel the existing entry
                        //   and reschedule?
                        ScheduleMessage::Enqueue(guild_id, album, interval) => {
                            // interval is in minutes, so we multiply by 60 seconds
                            let interval = Duration::from_secs(interval);
                            let key = queue.insert(QueueItem::new(guild_id.clone(), album.clone()), interval);
                            id_to_key.insert(guild_id, key);
                        },
                        ScheduleMessage::Dequeue(guild_id) => {
                            if let Some(key) = id_to_key.remove(&guild_id) {
                                queue.remove(&key);
                            }
                        },
                    };
                }
            );
        }
    });

    Ok(UserData { scheduler: tx })
}
