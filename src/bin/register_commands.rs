use discord_banner_bot::{commands::commands, error::Error, utils::start_logging, Settings};
use poise::serenity_prelude;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Error> {
    Settings::init()?;
    let settings = Settings::get();
    println!("Using log level: {}", settings.bot.log_level);

    start_logging(&settings.bot.log_level);

    let http = serenity_prelude::HttpBuilder::new(&settings.bot.token).build();
    let commands = poise::builtins::create_application_commands(&commands());

    match serenity_prelude::Command::set_global_commands(&http, &commands).await {
        Ok(set_cmds) => {
            info!("{} commands have been set globally", set_cmds.len());
        }
        Err(err) => {
            error!("{err}");
        }
    }

    Ok(())
}
