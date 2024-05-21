use async_repeater::{Repeater, RepeaterHandle};
use poise::serenity_prelude::{FullEvent, GuildId, Ready};
use reqwest::Client;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tracing::{debug, error, info};

use crate::{
    constants::USER_AGENT,
    database::{guild_schedule::GuildSchedule, Database},
    error::handle_schedule_error,
    schedule::Schedule,
    schedule_runner::ScheduleRunner,
    settings::Settings,
    utils::dm_users,
    Data, Error,
};

#[derive(Clone)]
#[allow(dead_code)]
/// The User data struct used in poise
pub struct State {
    /// Used to communicate with the scheduler without needing a &mut self
    repeater_handle: OnceLock<RepeaterHandle<Schedule>>,
    /// Client for http request
    reqw_client: Client,
    /// database pool
    database: Database,
    /// settings
    settings: &'static Settings,
}

impl State {
    /// Creates a new state but does not fully initialize it yet.
    /// initialization happens when the first READY event is received
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
            settings,
        })
    }

    /// Enqueue a schedule for the guild at interval
    ///
    /// # Panics
    /// Will panic if called before initialization is complete
    pub async fn enque(&self, schedule: Schedule) -> Result<(), Error> {
        info!("Inserting {schedule:?}");
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

    #[allow(dead_code)]
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
}

pub async fn event_handler(
    framework: poise::FrameworkContext<'_, Data, Error>,
    event: &FullEvent,
) -> Result<(), Error> {
    match event {
        FullEvent::Ready { data_about_bot } => {
            let data = framework.user_data();
            if data.repeater_handle.get().is_some() {
                info!("Already initialized, skipping setup.");
                return Ok(());
            }
            handle_event_ready(framework, data_about_bot).await
        }
        FullEvent::GuildDelete { incomplete, .. } => {
            info!("GuildDelete: {:?}", incomplete);

            let ctx = framework.serenity_context;
            let data: Arc<State> = ctx.data();

            // do nothing if guild has gone offline. could just be a temporary outtage
            if incomplete.unavailable {
                return Ok(());
            }

            // otherwise the bot might have been kicked
            data.deque(incomplete.id).await?;
            Ok(())
        },
        _ => Ok(()),
    }
}

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

    let callback = move |schedule, handle| async move {
        let task = ScheduleRunner::new(ctx.clone(), db.clone(), http.clone(), schedule);

        let Err(err) = task.run().await else {
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

    // schedule already existing guilds
    let known_guild_ids: Vec<u64> = data.database().active_schedules().await?;
    info!("Amount of schedules in db: {}", known_guild_ids.len());

    for id in known_guild_ids {
        let entry = match data.database().get::<GuildSchedule>(id).await {
            Ok(entry) => entry,
            Err(err) => {
                let msg =
                    format!(" - guild_id={id}, failed to fetch schedule from db: {err:#?} Skipping entry");
                error!(msg);
                dm_users(
                    &framework.serenity_context,
                    framework.options().owners.clone(),
                    &msg,
                )
                .await?;
                continue;
            }
        };

        let schedule = entry.into();

        info!(" - {schedule:?}");

        data.enque(schedule).await?;
    }

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
