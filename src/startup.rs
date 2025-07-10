use std::{collections::HashSet, sync::Arc};

use async_repeater::Repeater;
use poise::{
    insert_owners_from_http,
    serenity_prelude::{self, CacheHttp, Ready},
};
use tracing::{debug, error, info, instrument};

use crate::{Error, schedule_runner::schedule_callback, settings::Settings, state::State, utils::dm_users};

#[instrument(skip_all)]
pub(crate) async fn handle_event_ready(ctx: serenity_prelude::Context, _: &Ready) -> Result<(), Error> {
    debug!("handling ready event");
    let state: Arc<State> = ctx.data();
    if state.is_initialized() {
        error!("We are already initialized. Do not handle ready event");
        return Ok(());
    }

    debug!("setting owners");
    let mut owners = HashSet::new();
    insert_owners_from_http(ctx.http(), &mut owners, &None).await?;
    if let Err(err) = state.set_owners(owners) {
        error!("Owners have already been set before: {err:?}");
    }

    let repeater = Repeater::with_capacity(Settings::get().scheduler.capacity);
    let repeater_ctx = ctx.clone();

    state.set_repeater_handle(
        repeater.run_with_async_callback(move |schedule, _handle| schedule_callback(repeater_ctx, schedule)),
    )
    .expect("run only once");

    state.load_schedules_from_db().await?;

    // Notify that we're ready
    let bot_ready = "Bot ready!";
    dm_users(&ctx, state.owners(), bot_ready).await?;
    info!(bot_ready);
    Ok(())
}
