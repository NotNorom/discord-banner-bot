use discord_banner_bot::{
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
    Settings::init()?;
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
                    if let Err(e) = error::on_error(err).await {
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

    // If there is an error starting up the client
    if let Err(e) = client.start_autosharded().await {
        error!("Startup Error: {:?}", e);
    }

    Ok(())
}
