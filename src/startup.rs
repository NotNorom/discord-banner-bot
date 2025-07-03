use std::{collections::HashSet, sync::Arc, time::Duration};

use async_repeater::{Repeater, RepeaterHandle};
use poise::{
    insert_owners_from_http,
    serenity_prelude::{self, CacheHttp, Ready},
};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, instrument};

use crate::{
    Error,
    error::evaluate_schedule_error,
    schedule::Schedule,
    schedule_runner::{RunnerError, ScheduleAction, ScheduleRunner},
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
    }

    let ctx2 = ctx.clone();

    let repeater = Repeater::with_capacity(Settings::get().scheduler.capacity);
    let callback = |schedule: Schedule, _handle: RepeaterHandle<Schedule>| async move {
        let data: Arc<State> = ctx.data();
        let task = ScheduleRunner::new(
            ctx.clone(),
            data.database().to_owned(),
            data.reqw_client().to_owned(),
            schedule.clone(),
        );

        let mut retries_left = 3;
        let mut avoid_list = Vec::with_capacity(1);
        let mut override_url = None;

        while retries_left > 0 {
            retries_left -= 1;

            // run the actual task of changing the banner
            let timeout_result = timeout(
                Duration::from_secs(60),
                task.run(&avoid_list, override_url.clone()),
            )
            .await;

            let result = match timeout_result {
                Ok(res) => res,
                Err(_) => Err(RunnerError::new(
                    Error::Timeout {
                        action: "Banner changer task".to_string(),
                    },
                    schedule.guild_id(),
                    schedule.clone(),
                )),
            };

            let Err(err) = result else {
                debug!("Task finished successfully");
                return;
            };

            error!("Task had an error: {err:?}");

            let error_handling_result = evaluate_schedule_error(&err, ctx.clone(), data.owners()).await;

            match error_handling_result {
                Ok(action) => {
                    info!("Error was handled successfully. Recommended action={action:?}");
                    match action {
                        ScheduleAction::Continue => return,
                        ScheduleAction::RetrySameImage => {
                            if let (Some(url), Some(message)) = err.attempted_url_and_message() {
                                override_url = Some((url, *message));
                            }
                        }
                        ScheduleAction::RetryNewImage => {
                            if let (Some(url), Some(_)) = err.attempted_url_and_message() {
                                avoid_list.push(url);
                            }
                        }
                        ScheduleAction::Abort => {
                            let _ = data.deque(schedule.guild_id()).await;
                            return;
                        }
                    }
                }
                Err(critical_err) => {
                    let message = format!("CRITICAL ERROR schedule={schedule:?}: {critical_err:?}");
                    error!(message);
                    // if we encounter an error _now_ it's over anyways
                    let _ = data.deque(schedule.guild_id()).await;
                    let _ = dm_users(&ctx, data.owners(), &message).await;

                    return;
                }
            }

            // don't retry instantly, give it a little tiiiime
            sleep(Duration::from_secs(3)).await;
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
