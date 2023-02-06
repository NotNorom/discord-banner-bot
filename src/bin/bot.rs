use discord_banner_bot::{
    commands,
    error::{self, Error},
    startup::setup,
    Settings,
};
use poise::{serenity_prelude::GatewayIntents, FrameworkOptions, PrefixFrameworkOptions};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Error> {
    Settings::init()?;
    let settings = Settings::get();

    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(settings.bot.log_level.clone())
        .try_init()
        .expect("Set up logger");

    info!("Setting up framework. prefix={}", settings.bot.prefix);

    // set up & start client
    let result = poise::Framework::builder()
        .token(&settings.bot.token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .setup(move |ctx, ready, framework| Box::pin(setup(ctx, ready, framework)))
        .options(FrameworkOptions {
            commands: vec![
                commands::banner::album(),
                commands::banner::current(),
                commands::banner::start_for_guild(),
                commands::banner::start(),
                commands::banner::stop(),
                commands::help::help(),
                commands::notifications::notification_channel(),
                commands::register_globally(),
                commands::register(),
                commands::servers(),
                commands::unregister(),
            ],
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
