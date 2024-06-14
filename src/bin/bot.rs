use clap::Parser;
use discord_banner_bot::{
    cli::BotCli,
    commands::commands,
    error::{self, Error},
    shutdown::shutdown,
    startup::{event_handler, State},
    utils::start_logging,
    Settings,
};
use poise::{
    serenity_prelude::{self, GatewayIntents},
    FrameworkOptions, PrefixFrameworkOptions,
};
use tokio::sync::broadcast;
use tracing::{error, info, instrument};

#[tokio::main]
#[instrument]
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

    let (shutdown_sender, shutdown_receiver) = broadcast::channel(1);

    let mut client = serenity_prelude::ClientBuilder::new(
        &settings.bot.token,
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .data(State::new(shutdown_sender).await?.into())
    .framework(framework)
    .await?;

    let shutdown_task = tokio::spawn(shutdown(
        client.data(),
        client.shard_manager.clone(),
        shutdown_receiver,
    ));

    // If there is an error starting up the client
    if let Err(e) = client.start_autosharded().await {
        error!("Startup Error: {:?}", e);
    }

    // wait for shut down task to complete
    if let Err(err) = shutdown_task.await {
        error!("Could not shut down properly: {err:#}");
    }

    info!("Shut down complete. Goodbye.");

    Ok(())
}
