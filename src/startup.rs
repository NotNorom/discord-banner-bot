use std::{collections::HashSet, sync::Arc, time::Duration};

use async_repeater::Repeater;
use poise::{
    insert_owners_from_http,
    serenity_prelude::{self, CacheHttp, Ready},
};
use tokio::time::timeout;
use tracing::{debug, error, info, instrument};

use crate::{
    Error,
    error::handle_schedule_error,
    schedule::Schedule,
    schedule_runner::{RunnerError, ScheduleRunner},
    settings::Settings,
    state::State,
    utils::dm_users,
};

#[instrument(skip_all)]
pub(crate) async fn handle_event_ready(ctx: serenity_prelude::Context, _: &Ready) -> Result<(), Error> {
    debug!("handling ready event");
    if ctx.data::<State>().is_initialized() {
        error!("We are already initialized. Do not handle ready event");
        return Ok(());
    }

    // get owners
    debug!("setting owners");
    let mut owners = HashSet::new();
    insert_owners_from_http(ctx.http(), &mut owners, &None).await?;
    if let Err(err) = ctx.data::<State>().set_owners(owners) {
        error!("Owners have already been set before: {err:?}");
    };

    let ctx2 = ctx.clone();

    let repeater = Repeater::with_capacity(Settings::get().scheduler.capacity);
    let callback = |schedule: Schedule, handle| async move {
        let data: Arc<State> = ctx.data();
        let task = ScheduleRunner::new(
            ctx.clone(),
            data.database().to_owned(),
            data.reqw_client().to_owned(),
            schedule.clone(),
        );

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

        let error_handling_result =
            handle_schedule_error(&err, ctx, handle, data.database().to_owned(), data.owners()).await;

        match error_handling_result {
            Ok(action) => info!("Error was handled successfully. Recommended action={action:?}"),
            Err(critical_err) => error!("CRITICAL after handling previous error: {critical_err:?}"),
        }
    };

    let data: Arc<State> = ctx2.data();

    data.set_repeater_handle(repeater.run_with_async_callback(callback))
        .expect("run only once");

    data.load_schedules_from_db().await?;

    // Notify that we're ready
    let bot_ready = "Bot ready!";
    dm_users(&ctx2, data.owners(), bot_ready).await?;
    info!(bot_ready);
    Ok(())
}
