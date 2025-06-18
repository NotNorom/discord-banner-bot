use std::{collections::HashSet, fmt::Display, sync::OnceLock, time::Duration};

use async_repeater::RepeaterHandle;
use fred::error::RedisError;
use poise::serenity_prelude::{GuildId, UserId};
use reqwest::Client;
use tokio::sync::broadcast::Sender;
use tracing::{info, instrument};

use crate::{
    constants::USER_AGENT,
    database::{guild_schedule::GuildSchedule, Database},
    schedule::Schedule,
    Error, Settings,
};

/// The User data struct used in poise
pub struct State {
    /// Used to communicate with the scheduler without needing a &mut self
    repeater_handle: OnceLock<RepeaterHandle<Schedule>>,
    /// Client for http request
    reqw_client: Client,
    /// Database pool
    database: Database,
    /// Sender for shutdown message
    shutdown_messenger: Sender<()>,
    /// Owners
    owners: OnceLock<HashSet<UserId>>,
}

impl State {
    /// Creates a new state but does not fully initialize it yet.
    /// initialization happens when the first READY event is received
    #[instrument(skip_all)]
    pub async fn new(shutdown_messenger: Sender<()>) -> Result<Self, Error> {
        info!("Setting up state");
        let settings = Settings::get();

        let reqw_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .build()?;

        let database = Database::setup(&settings.database).await?;

        Ok(State {
            repeater_handle: OnceLock::default(),
            reqw_client,
            database,
            shutdown_messenger,
            owners: OnceLock::default(),
        })
    }

    /// Is the bot initialized?
    ///
    /// Returns true if it is
    pub fn is_initialized(&self) -> bool {
        self.repeater_handle.get().is_some() && self.owners.get().is_some()
    }

    /// Enqueue a schedule for the guild at interval
    ///
    /// # Panics
    /// Will panic if called before initialization is complete
    #[instrument(skip_all)]
    pub async fn enque(&self, schedule: Schedule) -> Result<(), Error> {
        info!("Inserting {schedule:?}");

        // insert into db here to make sure we don't loose it if we have to restart the bot
        // but the start_at time has not been reached yet
        let db_schedule = GuildSchedule::from(schedule.clone());
        self.database.insert(&db_schedule, db_schedule.guild_id()).await?;

        self.repeater_handle
            .get()
            .unwrap()
            .insert(schedule)
            .await
            .map_err(|err| Error::Scheduler { msg: err.to_string() })
    }

    /// Dequeue a guild
    ///
    /// # Panics
    /// Will panic if called before initialization is complete
    #[instrument(skip_all)]
    pub async fn deque(&self, guild_id: GuildId) -> Result<(), Error> {
        info!("Removing {guild_id:?}");
        self.repeater_handle
            .get()
            .unwrap()
            .remove(guild_id)
            .await
            .map_err(|err| Error::Scheduler { msg: err.to_string() })?;
        Ok(self.database.delete::<GuildSchedule>(guild_id.get()).await?)
    }

    /// Load all schedules from the database into the repeater
    ///
    /// # Panics
    /// Will panic if called before initialization is complete
    #[instrument(skip_all)]
    pub async fn load_schedules_from_db(&self) -> Result<LoadFromDbResult, Error> {
        // clear everything
        let _ = self.repeater_handle.get().unwrap().clear().await;

        let known_guild_ids: Vec<u64> = self.database().active_schedules().await?;
        info!("There are {} active schedules stored", known_guild_ids.len());

        let mut result = LoadFromDbResult::default();

        for id in known_guild_ids {
            let entry = match self.database().get::<GuildSchedule>(id).await {
                Ok(entry) => entry,
                Err(err) => {
                    result.failed.push((GuildId::new(id), err));
                    continue;
                }
            };
            result.successful.push(entry);
            let schedule = entry.into();
            self.enque(schedule).await?;
        }
        Ok(result)
    }

    /// Shut down the bot
    pub fn shutdown(&self) -> Result<(), Error> {
        self.shutdown_messenger
            .send(())
            .map(|_| ())
            .map_err(|err| Error::Scheduler { msg: err.to_string() })
    }

    /// Get a reference to the user data's reqwest client.
    pub fn reqw_client(&self) -> &Client {
        &self.reqw_client
    }

    /// Get a reference to the user data's redis client.
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Get the schedule for the guild
    pub async fn get_schedule(&self, guild_id: GuildId) -> Result<Schedule, RedisError> {
        let db_entry = self.database.get::<GuildSchedule>(guild_id.get()).await?;
        Ok(db_entry.into())
    }

    /// Get a clone of the repeater handle
    ///
    /// # Panics
    /// Will panic if called before initialization is complete
    pub fn repeater_handle(&self) -> RepeaterHandle<Schedule> {
        self.repeater_handle.get().unwrap().clone()
    }

    /// Set the repeater handle
    pub fn set_repeater_handle(
        &self,
        repeater_handle: RepeaterHandle<Schedule>,
    ) -> Result<(), RepeaterHandle<Schedule>> {
        self.repeater_handle.set(repeater_handle)
    }

    /// Get a clone of the owners
    ///
    /// # Panics
    /// Will panic if called before initialization is complete
    pub fn owners(&self) -> HashSet<UserId> {
        self.owners.get().cloned().unwrap()
    }

    /// Set the owners
    pub fn set_owners(&self, owners: HashSet<UserId>) -> Result<(), HashSet<UserId>> {
        self.owners.set(owners)
    }
}

#[derive(Debug, Default)]
pub struct LoadFromDbResult {
    successful: Vec<GuildSchedule>,
    failed: Vec<(GuildId, RedisError)>,
}

impl Display for LoadFromDbResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Successful:")?;
        for guild_schedule in &self.successful {
            writeln!(f, "- {guild_schedule:?}")?;
        }

        writeln!(f, "Failed:")?;
        for (guild_id, err) in &self.failed {
            writeln!(f, "{guild_id}, {err:#?}")?;
        }

        Ok(())
    }
}
