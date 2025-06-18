use std::sync::Arc;

use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::broadcast::Receiver,
};
use tracing::{error, info};

use crate::{Error, state::State};

/// Wait for signal and shut down bot, repeater and database connection in order.
///
/// Signals to be waited for:
///     - SIGINT
///     - SIGTERM
///     - SIGQUIT
///     - shutdown command
pub async fn shutdown(
    state: Arc<State>,
    shard_manager_shutdown_fn: impl FnOnce() -> bool + Send,
    mut internal_receiver: Receiver<()>,
) -> Result<(), Error> {
    let mut stream_interrupt = signal(SignalKind::interrupt()).unwrap();
    let mut stream_terminate = signal(SignalKind::terminate()).unwrap();
    let mut stream_quit = signal(SignalKind::quit()).unwrap();

    let received_signal = {
        select! {
            _ = stream_interrupt.recv() => {
                "SIGINT"
            },
            _ = stream_terminate.recv() => {
                "SIGTERM"
            },
            _ = stream_quit.recv() => {
                "SIGQUIT"
            },
            _ = internal_receiver.recv() => {
                "INTERNAL"
            }
        }
    };

    info!("Received signal {received_signal}, shutting down");

    // close connection to discord
    match shard_manager_shutdown_fn() {
        true => info!("Discord has shut down"),
        false => error!("Discord has shut down properly"),
    }

    // stop banner queue
    if let Err(err) = state.repeater_handle().stop().await {
        error!("Repeater did not shut down properly: {err:#}");
    }
    info!("Repeater shut down properly");

    // disconnect from database
    state.database().disconnect();
    info!("Database disconnected properly");

    info!("Shut down sequence done");
    Ok(())
}
