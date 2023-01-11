mod album_provider;
mod banner_scheduler;
mod commands;
mod constants;
mod database;
mod error;
mod guild_id_ext;
mod settings;
mod startup;
mod utils;

use poise::{serenity_prelude::GatewayIntents, FrameworkOptions, PrefixFrameworkOptions};
use startup::UserData;
use tracing::{error, info};

pub use crate::error::Error;
use crate::startup::setup;

type Data = UserData;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // get environment variables
    // let _ = dotenv::dotenv().ok();
    // let token = dotenv::var("DISCORD_TOKEN").expect("No token in env");
    // let prefix = dotenv::var("PREFIX").unwrap_or_else(|_| "b!".to_string());

    settings::init()?;
    let settings = settings::settings();

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
                    if let Err(e) = crate::error::on_error(err).await {
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
