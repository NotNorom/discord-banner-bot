mod album_provider;
mod banner_scheduler;
mod commands;
mod constants;
mod database;
mod error;
mod guild_id_ext;
mod startup;
mod utils;

use poise::{
    serenity_prelude::{GatewayIntents, UserId},
    FrameworkOptions, PrefixFrameworkOptions,
};
use startup::UserData;
use tracing::error;

use crate::startup::setup_user_data;

type Data = UserData;
type Error = crate::error::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // get environment variables
    let _ = dotenv::dotenv().ok();
    let token = dotenv::var("DISCORD_TOKEN").expect("No token in env");
    let prefix = dotenv::var("PREFIX").unwrap_or_else(|_| "b!".to_string());
    let owners = dotenv::var("OWNERS")
        .unwrap_or_else(|_| "160518747713437696".to_string())
        .split_ascii_whitespace()
        .filter_map(|a| a.parse().ok())
        .map(UserId)
        .collect();

    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt::init();
    // set up & start client
    let result = poise::Framework::build()
        .token(&token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .user_data_setup(move |ctx, ready, framework| Box::pin(setup_user_data(ctx, ready, framework)))
        .options(FrameworkOptions {
            on_error: |err| {
                Box::pin(async move {
                    if let Err(e) = crate::error::on_error(err).await {
                        error!("{e:?}");
                    };
                })
            },
            owners,
            prefix_options: PrefixFrameworkOptions {
                prefix: Some(prefix),

                ..Default::default()
            },
            commands: vec![
                commands::help::help(),
                commands::register(),
                commands::register_globally(),
                commands::unregister(),
                commands::banner::start(),
                commands::banner::stop(),
                commands::banner::album(),
                commands::banner::current(),
                commands::banner::start_for_guild(),
            ],

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
