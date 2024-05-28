use std::sync::Arc;

use clap::Parser;
use discord_banner_bot::{
    cli::BotCli,
    commands::commands,
    error::{self, Error},
    startup::{event_handler, State},
    utils::start_logging,
    Settings,
};
use poise::{
    serenity_prelude::{self, GatewayIntents},
    FrameworkOptions, PrefixFrameworkOptions,
};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = BotCli::parse();
    Settings::init_from_path(&cli.settings_file)?;
    let settings = Settings::get();

    println!("Using log level: {}", settings.bot.log_level);
    start_logging(&settings.bot.log_level);

    info!("Setup: prefix={}", settings.bot.prefix);

    // set up & start client
    let framework = poise::Framework::builder()
        .options(FrameworkOptions {
            commands: commands(),
            on_error: |err| {
                Box::pin(async move {
                    if let Err(e) = error::handle_framework_error(err).await {
                        error!("{e:?}");
                    };
                })
            },
            prefix_options: PrefixFrameworkOptions {
                prefix: Some(settings.bot.prefix.as_str().into()),

                ..Default::default()
            },
            event_handler: |framework, event| Box::pin(event_handler(framework, event)),
            ..Default::default()
        })
        .build();

    let mut client = serenity_prelude::ClientBuilder::new(
        &settings.bot.token,
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .data(State::new().await?.into())
    .framework(framework)
    .await?;

    let shard_manager = client.shard_manager.clone();
    let state: Arc<State> = client.data();

    let shut_down_task = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("Received ctrl-c, shutting down");

        // close connection to discord
        shard_manager.shutdown_all().await;

        // stop banner queue
        if let Err(err) = state.repeater_handle().stop().await {
            error!("Repeater did not shut down properly: {err:#}");
        }
        info!("Repeater shut down properly");

        // disconnect from database
        if let Err(err) = state.database().disconnect().await {
            error!("Database did not shut down properly: {err:#}");
        };
        info!("Database shut down properly");
    });

    // If there is an error starting up the client
    if let Err(e) = client.start_autosharded().await {
        error!("Startup Error: {:?}", e);
    }

    // wait for shut down task to complete
    if let Err(err) = shut_down_task.await {
        error!("Could not shut down properly: {err:#}");
    }

    info!("Shut down complete. Goodbye.");

    Ok(())
}
