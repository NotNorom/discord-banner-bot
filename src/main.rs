mod commands;
mod user_data;
mod utils;

use dotenv;
use poise::serenity_prelude::GatewayIntents;
use poise::serenity_prelude::UserId;
use poise::FrameworkOptions;
use user_data::UserData;

use crate::user_data::setup_user_data;

type Data = UserData;
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// Register application commands in this guild or globally
/// Run with no arguments to register in guild, run with argument "global" to register globally.
#[poise::command(prefix_command, owners_only)]
async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::samples::register_application_commands(ctx, false).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // get environment variables
    let _ = dotenv::dotenv().ok();
    let token = dotenv::var("DISCORD_TOKEN").expect("No token in env");
    let prefix = dotenv::var("PREFIX").unwrap_or("b!".to_string());
    let owners = dotenv::var("OWNERS")
        .unwrap_or("160518747713437696".to_string())
        .split_ascii_whitespace()
        .filter_map(|a| a.parse().ok())
        .map(|id: u64| UserId(id))
        .collect();

    // set up & start client
    let result = poise::Framework::build()
        .prefix(prefix)
        .token(&token)
        .user_data_setup(move |ctx, ready, framework| {
            Box::pin(setup_user_data(ctx, ready, framework))
        })
        .client_settings(|serenity_builder| {
            serenity_builder.intents(GatewayIntents::non_privileged())
        })
        .options(FrameworkOptions {
            on_error: |err, ctx| Box::pin(poise::samples::on_error(err, ctx)),
            owners,

            ..Default::default()
        })
        .command(register(), |f| f)
        .command(commands::start(), |f| f)
        .command(commands::stop(), |f| f)
        .command(commands::album(), |f| f)
        .command(commands::current(), |f| f)
        .run()
        .await;
    
    // If there is an error starting up the client
    if let Err(e) = result {
        eprintln!("Startup Error: {:?}", e);
    }

    Ok(())
}
