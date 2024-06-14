use std::collections::HashMap;

use clap::Parser;
use discord_banner_bot::{
    cli::{GuildOrGlobally, ServerOwners, UtilCli, UtilCommand},
    commands::commands,
    database::Database,
    error::Error,
    finding_media::{find_media_in_channel, MediaWithMessage},
    utils::{dm_user, start_logging},
    Settings,
};
use poise::serenity_prelude::{self, GuildId, Http, MessageBuilder, PartialGuild, UserId};
use tokio_stream::StreamExt;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = UtilCli::parse();
    println!("{cli:?}");

    Settings::init()?;
    let settings = Settings::get();
    println!("Using log level: {}", settings.bot.log_level);

    start_logging(&settings.bot.log_level);
    let http = serenity_prelude::HttpBuilder::new(&settings.bot.token).build();
    http.set_application_id(http.get_current_application_info().await?.id);
    let database = Database::setup(&settings.database).await?;

    match cli.command {
        UtilCommand::RegisterCommands { command } => register_commands(http, command).await?,
        UtilCommand::UnregisterCommands { command } => unregister_commands(http, command).await?,
        UtilCommand::DmServerOwners {
            who,
            message,
            mention_owned_guilds,
        } => dm_server_owners(&http, &database, who, message, mention_owned_guilds).await?,
        UtilCommand::FindMedia { channel_id, limit } => {
            let mut thingies: Vec<MediaWithMessage> = find_media_in_channel(&http, &channel_id, limit)
                .filter_map(Result::ok)
                .collect()
                .await;
            thingies.reverse();

            for media in thingies {
                println!("{}:\n\t{}\n", media.message.link(), media.media);
            }
        }
    };

    Ok(())
}

pub async fn register_commands(
    http: Http,
    guild_or_globally: GuildOrGlobally,
) -> Result<(), serenity_prelude::Error> {
    let commands = poise::builtins::create_application_commands(&commands());

    let result = match guild_or_globally {
        GuildOrGlobally::InGuild { guild } => guild.set_commands(&http, &commands).await,
        GuildOrGlobally::Globally => serenity_prelude::Command::set_global_commands(&http, &commands).await,
    };

    match &result {
        Ok(cmds) => match guild_or_globally {
            GuildOrGlobally::InGuild { guild } => {
                info!("{} commands have been set in guild: {}", cmds.len(), guild);
            }
            GuildOrGlobally::Globally => info!("{} commands have been set globally", cmds.len()),
        },
        Err(err) => error!("Failed to set commands: {err:#}"),
    };

    Ok(())
}

pub async fn unregister_commands(
    http: Http,
    guild_or_globally: GuildOrGlobally,
) -> Result<(), serenity_prelude::Error> {
    let result = match guild_or_globally {
        GuildOrGlobally::InGuild { guild } => guild.set_commands(&http, &[]).await,
        GuildOrGlobally::Globally => serenity_prelude::Command::set_global_commands(&http, &[]).await,
    };

    match &result {
        Ok(_) => match guild_or_globally {
            GuildOrGlobally::InGuild { guild } => {
                info!("All commands have been removed in guild: {}", guild);
            }
            GuildOrGlobally::Globally => info!("All commands have been removed globally"),
        },
        Err(err) => error!("Failed to remove commands: {err:#}"),
    };

    Ok(())
}

pub async fn dm_server_owners(
    http: &Http,
    database: &Database,
    who: ServerOwners,
    message: String,
    mention_owned_guilds: bool,
) -> Result<(), Error> {
    let owners = get_owners(http, database, who).await?;

    for (owner, guilds) in &owners {
        let mut content_builder = MessageBuilder::new();
        if mention_owned_guilds {
            content_builder = content_builder.push("Hi! You are receiving this message because you own: ");
            for guild in guilds {
                content_builder = content_builder.push_safe(&*format!("{}, ", guild.name));
            }
            content_builder = content_builder.push_bold_line("");
        }
        let content = content_builder.push(&*message).build();
        info!("sending dm to {}: {}", owner, content);
        dm_user(&http, *owner, &content).await?;
    }

    Ok(())
}

async fn get_owners(
    http: &Http,
    database: &Database,
    who: ServerOwners,
) -> Result<HashMap<UserId, Vec<PartialGuild>>, Error> {
    let mut owners = HashMap::with_capacity(100);

    let guild_ids: Box<dyn Iterator<Item = GuildId>> = match who {
        ServerOwners::AllOfThem => Box::new(get_all_owners(http).await?),
        ServerOwners::WithActiveSchedule => Box::new(get_owners_with_active_schedule(database).await?),
    };

    for guild_id in guild_ids {
        let guild = guild_id.to_partial_guild(&http).await?;
        owners
            .entry(guild.owner_id)
            .and_modify(|e: &mut Vec<PartialGuild>| e.push(guild.clone()))
            .or_insert(vec![guild]);
    }

    Ok(owners)
}

async fn get_all_owners(http: &Http) -> Result<impl Iterator<Item = GuildId>, Error> {
    Ok(http.get_guilds(None, None).await?.into_iter().map(|info| info.id))
}

async fn get_owners_with_active_schedule(
    database: &Database,
) -> Result<impl Iterator<Item = GuildId>, Error> {
    Ok(database.active_schedules().await?.into_iter().map(GuildId::new))
}
