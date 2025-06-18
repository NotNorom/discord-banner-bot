use clap::Parser;
use discord_banner_bot::{
    Settings, State,
    cli::BotCli,
    commands::commands,
    error::{self, Error},
    event_handler::Handler,
    shutdown::shutdown,
    utils::start_logging,
};
use poise::{
    FrameworkOptions, PrefixFrameworkOptions,
    serenity_prelude::{self, GatewayIntents},
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

    let framework_options = FrameworkOptions {
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
        initialize_owners: true,
        initialized_team_roles: None,
        ..Default::default()
    };

    // set up & start client
    let framework = poise::Framework::builder().options(framework_options).build();

    let (shutdown_sender, shutdown_receiver) = broadcast::channel(1);

    let mut client = serenity_prelude::ClientBuilder::new(
        settings.bot.token.clone(),
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .data(State::new(shutdown_sender).await?.into())
    .framework(framework)
    .event_handler(Handler)
    .await?;

    let shutdown_task = tokio::spawn(shutdown(
        client.data(),
        client.shard_manager.get_shutdown_trigger(),
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
