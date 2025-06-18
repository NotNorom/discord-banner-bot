use std::sync::Arc;

use fred::error::RedisErrorKind;
use poise::serenity_prelude::{async_trait, Context, EventHandler, FullEvent};
use tracing::{debug, error, info, instrument, warn};

use crate::{startup::handle_event_ready, state::State, utils::dm_users, Error};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip_all)]
    async fn dispatch(&self, context: &Context, event: &FullEvent) {
        let Err(error) = handle_event(context, event).await else {
            return;
        };

        handle_event_handler_error(context, event, error).await;
    }
}

pub async fn handle_event(context: &Context, event: &FullEvent) -> Result<(), Error> {
    match event {
        FullEvent::Ready { data_about_bot, .. } => {
            if context.data::<State>().is_initialized() {
                debug!("Ready event fired but already initialized. Skipping setup, but reloading schedules");
                let result = context.data::<State>().load_schedules_from_db().await.unwrap();
                info!("{result}");
                return Ok(());
            }
            handle_event_ready(context.to_owned(), data_about_bot).await
        }
        FullEvent::GuildDelete { incomplete, .. } => {
            debug!("GuildDelete: {incomplete:?}");

            let data: Arc<State> = context.data();

            // do nothing if guild has gone offline. could just be a temporary outtage
            if incomplete.unavailable {
                return Ok(());
            }

            // otherwise the bot might have been kicked
            if !context.data::<State>().is_initialized() {
                error!("GuildDelete event fired before bot was initialized");
            }

            data.deque(incomplete.id).await
        }
        FullEvent::Resume { event, .. } => {
            debug!("Resume: {event:?}");
            let data = context.data::<State>();

            if !data.is_initialized() {
                warn!("Resume event fired before bot was initialized");
                return Ok(());
            }

            info!("Loading schedules from database");
            let result = data.load_schedules_from_db().await?;
            info!("{result}");
            Ok(())
        }
        FullEvent::ShardStageUpdate { event, .. } => {
            debug!("ShardStageUpdate: {event:?}");
            Ok(())
        }
        FullEvent::ShardsReady { total_shards, .. } => {
            debug!("ShardsReady: {total_shards:?}");
            Ok(())
        }
        FullEvent::ChannelDelete { channel, .. } => {
            // if the channel that contains the banners of a guild is deleted
            // then unschedule the guild

            debug!("ChannelDelete: {channel:?}");

            let data: Arc<State> = context.data();
            let schedule = match data.get_schedule(channel.base.guild_id).await {
                Ok(schedule) => schedule,
                Err(err) if *err.kind() == RedisErrorKind::NotFound => return Ok(()),
                Err(err) => return Err(err)?,
            };

            if channel.base.guild_id == schedule.guild_id() {
                data.deque(channel.base.guild_id).await?;
            }

            Ok(())
        }
        _ => Ok(()),
    }
}

#[instrument(skip_all)]
pub async fn handle_event_handler_error(context: &Context, event: &FullEvent, error: Error) {
    error!("Error handling event: {event:?}: {error}");
    let _ = dm_users(
        context,
        context.data::<State>().owners(),
        &format!("Error handling event: {event:?}.\nError:\n{error}"),
    )
    .await;
}
