use std::{collections::HashMap, env::args};

use discord_banner_bot::{
    database::Database,
    error::Error,
    utils::{dm_user, start_logging},
    Settings,
};
use poise::serenity_prelude::{self, GuildId, MessageBuilder, PartialGuild};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    Settings::init()?;
    let settings = Settings::get();
    info!("Using log level: {}", settings.bot.log_level);

    start_logging("dm_server_owners=info,reqwest=info,poise=info,serenity=info,info");

    let http = serenity_prelude::HttpBuilder::new(&settings.bot.token).build();

    let database = Database::setup(&settings.database).await?;

    let active_schedules: Vec<u64> = database.active_schedules().await?;
    println!("Known guilds = {active_schedules:?}");

    let mut owners = HashMap::with_capacity(active_schedules.len());

    for guild_id in active_schedules.into_iter().map(GuildId::new) {
        let guild = guild_id.to_partial_guild(&http).await?;
        owners
            .entry(guild.owner_id)
            .and_modify(|e: &mut Vec<PartialGuild>| e.push(guild.clone()))
            .or_insert(vec![guild]);
    }

    for (owner, guilds) in &owners {
        let mut content_builder = MessageBuilder::new()
            .push_bold_line("Hey there!")
            .push("You are using this bot inside of these servers: ");
        for guild in guilds {
            content_builder = content_builder.push_safe(&*format!("{}, ", guild.name));
        }
        let content = content_builder
            .push_line("")
            .push(&*args().nth(1).unwrap_or_default())
            .build();
        info!("sending dm to {}: {}", owner, content);
        dm_user(&http, *owner, &content).await?;
    }

    Ok(())
}
