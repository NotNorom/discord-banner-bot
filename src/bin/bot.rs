use discord_banner_bot::{
    commands::commands,
    error::{self, Error},
    startup::setup,
    utils::start_logging,
    Settings,
};
use poise::{serenity_prelude::GatewayIntents, FrameworkOptions, PrefixFrameworkOptions};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Error> {
    Settings::init()?;
    let settings = Settings::get();
    println!("Using log level: {}", settings.bot.log_level);

    start_logging(&settings.bot.log_level);

    info!("Setup: prefix={}", settings.bot.prefix);

    // set up & start client
    let result = poise::Framework::builder()
        .token(&settings.bot.token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .setup(move |ctx, ready, framework| Box::pin(setup(ctx, ready, framework)))
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
                prefix: Some(settings.bot.prefix.clone()),

                ..Default::default()
            },
            ..Default::default()
        })
        .run()
        .await;

    // If there is an error starting up the client
    if let Err(e) = result {
        error!("Startup Error: {:?}", e);
    }

    Ok(())
}
