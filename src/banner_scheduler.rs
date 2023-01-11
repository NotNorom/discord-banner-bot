use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use poise::serenity_prelude::{GuildId, MessageBuilder, UserId};
use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_stream::StreamExt;
use tokio_util::time::{delay_queue::Key, DelayQueue};
use tracing::{debug, error, info, warn};

use crate::{
    album_provider::{Album, Providers},
    database::{Database, GuildSchedule},
    guild_id_ext::RandomBanner,
    settings::Settings,
    utils::{current_unix_timestamp, dm_users},
    Error,
};

#[derive(Debug)]
pub struct ScheduleMessageEnqueue {
    guild_id: GuildId,
    album: Album,
    interval: u64,
    offset: Option<u64>,
}

#[derive(Debug)]
pub enum ScheduleMessage {
    /// discord guild id, album url, interval in minutes
    Enqueue(ScheduleMessageEnqueue),
    /// discord guild id
    Dequeue(GuildId),
}

impl ScheduleMessage {
    pub fn new_enqueue(guild_id: GuildId, album: Album, interval: u64, offset: Option<u64>) -> Self {
        Self::Enqueue(ScheduleMessageEnqueue {
            guild_id,
            album,
            interval,
            offset,
        })
    }

    pub fn new_dequeue(guild_id: GuildId) -> Self {
        Self::Dequeue(guild_id)
    }
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    guild_id: GuildId,
    album: Album,
    interval: Duration,
}

impl QueueItem {
    /// Creates a new `QueueItem`
    pub fn new(guild_id: GuildId, album: Album, interval: Duration) -> Self {
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
    pub fn album(&self) -> &Album {
        &self.album
    }

    /// Get a reference to the queue item's interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }
}

/// Responsible for changing the banners.
pub struct BannerQueue {
    /// Queue, that will yield items after they time out
    queue: DelayQueue<QueueItem>,
    /// Internal struct needed for the DelayQueue
    guild_id_to_key: HashMap<GuildId, Key>,
    /// Discord
    ctx: Arc<poise::serenity_prelude::Context>,
    /// Needed so owners get notified in case of errors
    owners: HashSet<UserId>,
    /// To save the queue state so the bot can resume when it restarts
    database: Database,
    /// For fetching images from the web
    http_client: reqwest::Client,
    /// For communication with commands
    rx: Receiver<ScheduleMessage>,
    /// For providing the images
    providers: Providers,
}

impl BannerQueue {
    pub fn new(
        ctx: Arc<poise::serenity_prelude::Context>,
        owners: HashSet<UserId>,
        database: Database,
        http_client: reqwest::Client,
        capacity: usize,
        settings: &Settings,
    ) -> (Sender<ScheduleMessage>, BannerQueue) {
        let queue = DelayQueue::<QueueItem>::with_capacity(capacity);
        // maps the guild id to the key used in the queue
        let guild_id_to_key = HashMap::with_capacity(capacity);

        let (tx, rx) = mpsc::channel::<ScheduleMessage>(capacity);

        let providers = Providers::new(&settings.provider, &http_client);

        (
            tx,
            Self {
                queue,
                guild_id_to_key,
                ctx,
                owners,
                database,
                http_client,
                rx,
                providers,
            },
        )
    }

    /// Start the scheduler task
    ///
    /// This function handles enqueue and dequeue commands.
    /// This needs to be run in a separate tokio task using e.g. [`tokio::task::spawn`]
    pub async fn scheduler(mut self) {
        loop {
            // either handle an item from the queue:
            //   change the banner
            // or enqueue/ dequeue an item from the queue
            select!(
                // If a guild is ready to have their banner changed
                Some(item) = self.queue.next() => {
                    debug!("Queue poped: {item:?}");

                    let inner = item.into_inner();
                    let mut guild_id = inner.guild_id();
                    let interval = inner.interval();
                    let album = inner.album().clone();

                    // re-enqueue the item
                    let key = self.queue.insert(inner, interval);
                    self.guild_id_to_key.insert(guild_id, key);

                    // get the images from the provider
                    let images = match self.providers.images(&album).await {
                        Ok(images) => images,
                        Err(e) => {
                            error!("Could not get images from provider: {e:?}. Not scheduling {guild_id} again");
                            continue;
                        },
                    };

                    info!("Trying to change the banner for {guild_id}");

                    // creating the redis entry just before the banner is set,
                    // because the timestamp must be when we _start_ setting the banner,
                    // not when the command finally returns from discord (which might take a few seconds)
                    let schedule = GuildSchedule::new(guild_id.0, album.url().to_string(), interval.as_secs(), current_unix_timestamp());

                    // change the banner
                    if let Err(e) = guild_id.set_random_banner(&self.ctx.http, &self.http_client, &images).await {
                        if let Err(handler_e) = self.handle_error(guild_id, &e).await {
                            error!("When handling {:?}, another error occurced {:?}", e, handler_e);
                        }
                    }

                    // insert into redis
                    if let Err(err) = self.database.insert(&schedule, guild_id.0).await {
                        error!("Could not insert db entry: {schedule:?}, {err}");
                    } else {
                        info!("Change succeeded. Updated entry {schedule:?}");
                    }
                },
                // If a guild is to be added or removed from the queue
                Some(msg) = self.rx.recv() => {
                    if let (guild_id, Err(e)) = match msg {
                        ScheduleMessage::Enqueue(enqueue_msg) => (enqueue_msg.guild_id, self.enqueue(enqueue_msg).await),
                        ScheduleMessage::Dequeue(guild_id) => (guild_id, self.dequeue(guild_id).await),
                    } {
                        if let Err(handler_e) = self.handle_error(guild_id, &e).await {
                            error!("When handling {:?}, another error occurced {:?}", e, handler_e);
                        }
                    }
                }
            );
        }
    }

    /// Add a guild to be scheduled for banner changes.
    ///
    /// If a guild is already scheduled, the old schedule will be
    /// removed and a new one will be created.
    /// The banner will be changed once before a schedule is a created.
    /// If the banner can not changed, no new schedule will be created,
    /// this means that no schedule will be running.
    async fn enqueue(&mut self, enqueue_msg: ScheduleMessageEnqueue) -> Result<(), Error> {
        let ScheduleMessageEnqueue {
            mut guild_id,
            album,
            interval,
            offset,
        } = enqueue_msg;
        info!(
            "Enque: {guild_id}, with {album}, every {interval:>6} seconds. Next run at: {}",
            current_unix_timestamp() + offset.unwrap_or_default()
        );

        // if we have a timer, cancel it
        if let Some(key) = self.guild_id_to_key.get(&guild_id) {
            self.queue.remove(key);
        }

        // change the banner manually once, before enqueing IF the offset is None
        // the offset is None, when called through a command and Some when it's called on startup

        if offset.is_none() {
            debug!("Offset is none, setting banner once.");
            // get the images from the provider
            let images = self.providers.images(&album).await?;

            // try to change the banner, return when there is an error.
            // there is no further cleanup needed
            guild_id
                .set_random_banner(&self.ctx.http, &self.http_client, &images)
                .await?;
        }

        // now enqueue the new item
        let interval = Duration::from_secs(interval);
        let offset = Duration::from_secs(offset.unwrap_or_default());
        let key = self.queue.insert(
            QueueItem::new(guild_id, album.clone(), interval),
            interval + offset,
        );
        self.guild_id_to_key.insert(guild_id, key);

        Ok(())
    }

    /// Remove guild from schedule for banner changes
    async fn dequeue(&mut self, guild_id: GuildId) -> Result<(), Error> {
        if let Some(key) = self.guild_id_to_key.remove(&guild_id) {
            // removes entry from the delayqueue.
            // this unschedules the guild
            self.queue.remove(&key);
        }

        // remove the guild from the database
        // only scheduled guilds should be in the database

        if let Err(err) = self.database.delete::<GuildSchedule>(guild_id.0).await {
            error!("When deleting guild from database: {guild_id:?}, {err:?}")
        } else {
            info!("Removed guild {:?}", &guild_id.0);
        }

        Ok(())
    }

    /// Handle scheduler related errors
    ///
    /// This is a needed as well as the normal error handling in [crate::error::on_error] because
    /// the scheduler is running in its own task
    #[tracing::instrument(skip(self))]
    async fn handle_error(&mut self, guild_id: GuildId, error: &Error) -> Result<(), Error> {
        use poise::serenity_prelude;

        match error {
            Error::Serenity(error) => match error {
                serenity_prelude::Error::Http(error) => match error.as_ref() {
                    serenity_prelude::HttpError::UnsuccessfulRequest(error_response) => {
                        match error_response.status_code.as_u16() {
                            403 => {
                                // the bot does not have permissions to change the banner.
                                // remove guild from queue
                                self.dequeue(guild_id).await?;
                                warn!("Missing permissions to change banner for {guild_id}. Unscheduling.");
                            }
                            404 => {
                                self.dequeue(guild_id).await?;
                                warn!("Guild does not exist: {guild_id}. Unscheduling.");
                            }
                            _ => warn!("unsuccessful http request: {error_response:?}"),
                        }
                    }
                    http_err => error!("unhandled http error: {http_err:?}"),
                },
                serenity_err => error!("unhandled serenity error: {serenity_err:?}"),
            },
            Error::Command(error) => match error {
                crate::error::Command::GuildHasNoBanner => {
                    self.dequeue(guild_id).await?;
                    warn!("Guild has no banner feature");
                }
                command_err => error!("unhandled command error: {command_err:?}"),
            },
            Error::Imgur(error) => match error {
                imgurs::Error::SendApiRequest(send_api_err) => {
                    warn!("Error with imgur request: {send_api_err:#?}");
                }
                imgurs_err => error!("unhandled imgurs error: {imgurs_err}"),
            },
            err => {
                error!("unhandled bot error: {err:?}");
            }
        }

        let guild_name = format!("{}: {}", guild_id, guild_id.name(&self.ctx).unwrap_or_default());

        let message = MessageBuilder::new()
            .push_bold("Error in guild: ")
            .push_mono_line_safe(&guild_name)
            .push_codeblock(error.to_string(), Some("rust"))
            .build();

        dm_users(&self.ctx, self.owners.clone(), &message).await?;
        Ok(())
    }
}
