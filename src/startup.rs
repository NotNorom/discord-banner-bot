use std::{
    fmt::Display,
    sync::{Arc, OnceLock},
    time::Duration,
};

use async_repeater::{Repeater, RepeaterHandle};
use fred::error::RedisError;
use poise::serenity_prelude::{FullEvent, GuildId, Ready};
use reqwest::Client;
use tokio::time::timeout;
use tracing::{debug, error, info, instrument};

use crate::{
    constants::USER_AGENT,
    database::{guild_schedule::GuildSchedule, Database},
    error::handle_schedule_error,
    schedule::Schedule,
    schedule_runner::{RunnerError, ScheduleRunner},
    settings::Settings,
    utils::dm_users,
    Data, Error,
};

#[derive(Clone)]
/// The User data struct used in poise
pub struct State {
    /// Used to communicate with the scheduler without needing a &mut self
    repeater_handle: OnceLock<RepeaterHandle<Schedule>>,
    /// Client for http request
    reqw_client: Client,
    /// database pool
    database: Database,
}

impl State {
    /// Creates a new state but does not fully initialize it yet.
    /// initialization happens when the first READY event is received
    #[instrument(skip_all)]
    pub async fn new() -> Result<Self, Error> {
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
        })
    }

    /// Is the bot initialized?
    ///
    /// Returns true if it is
    pub fn is_initialized(&self) -> bool {
        self.repeater_handle.get().is_some()
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

    /// Get a reference to the user data's reqwest client.
    pub fn reqw_client(&self) -> &Client {
        &self.reqw_client
    }

    /// Get a reference to the user data's redis client.
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Get the schedule for the guild
    pub async fn get_schedule(&self, guild_id: GuildId) -> Result<Schedule, Error> {
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

#[instrument(skip_all)]
pub async fn event_handler(
    framework: poise::FrameworkContext<'_, Data, Error>,
    event: &FullEvent,
) -> Result<(), Error> {
    match event {
        FullEvent::Ready { data_about_bot } => {
            if framework.user_data().is_initialized() {
                debug!("Ready event fired but already initialized. Skipping setup, but reloading schedules");
                let result = framework.user_data().load_schedules_from_db().await?;
                info!("{result}");
                return Ok(());
            }
            handle_event_ready(framework, data_about_bot).await
        }
        FullEvent::GuildDelete { incomplete, .. } => {
            debug!("GuildDelete: {incomplete:?}");

            let ctx = framework.serenity_context;
            let data: Arc<State> = ctx.data();

            // do nothing if guild has gone offline. could just be a temporary outtage
            if incomplete.unavailable {
                return Ok(());
            }

            // otherwise the bot might have been kicked
            if !framework.user_data().is_initialized() {
                error!("GuildDelete event fired before bot was initialized");
            }

            data.deque(incomplete.id).await
        }
        FullEvent::Resume { event } => {
            debug!("Resume: {event:?}");
            let data = framework.user_data();
            if !data.is_initialized() {
                error!("Resume event fired before bot was initialized");
                return Ok(());
            }

            info!("Loading schedules from database");
            let result = data.load_schedules_from_db().await?;
            info!("{result}");
            Ok(())
        }
        FullEvent::ShardStageUpdate { event } => {
            debug!("ShardStageUpdate: {event:?}");
            Ok(())
        }
        FullEvent::ShardsReady { total_shards } => {
            debug!("ShardsReady: {total_shards:?}");
            Ok(())
        }
        _ => Ok(()),
    }
}

#[instrument(skip_all)]
async fn handle_event_ready(
    framework: poise::FrameworkContext<'_, Data, Error>,
    _: &Ready,
) -> Result<(), Error> {
    debug!("handling ready event");
    let ctx = framework.serenity_context;
    let settings = Settings::get();
    let data: Arc<State> = ctx.data();
    let db = data.database().clone();
    let http = data.reqw_client().clone();

    let repeater = Repeater::with_capacity(settings.scheduler.capacity);
    let owners = framework.options().owners.clone();
    let ctx = Arc::new(ctx.clone());

    let callback = move |schedule: Schedule, handle| async move {
        let task = ScheduleRunner::new(ctx.clone(), db.clone(), http.clone(), schedule.clone());

        let timeout_result = timeout(Duration::from_secs(60), task.run()).await;

        let result = match timeout_result {
            Ok(res) => res,
            Err(_) => Err(RunnerError::new(
                Error::Timeout {
                    action: "Banner changer task".to_string(),
                },
                schedule.guild_id(),
                schedule,
            )),
        };

        let Err(err) = result else {
            debug!("Task finished successfully");
            return;
        };

        error!("Task had an error: {err:?}");

        match handle_schedule_error(&err, ctx, handle, db.clone(), owners).await {
            Ok(action) => info!("Error was handled successfully. Recommended action={action:?}"),
            Err(critical_err) => error!("CRITICAL after handling previous error: {critical_err:?}"),
        }
    };

    data.repeater_handle
        .set(repeater.run_with_async_callback(callback))
        .expect("run only once");

    data.load_schedules_from_db().await?;

    // Notify that we're ready
    let bot_ready = "Bot ready!";
    dm_users(
        &framework.serenity_context,
        framework.options().owners.clone(),
        bot_ready,
    )
    .await?;
    info!(bot_ready);
    Ok(())
}
