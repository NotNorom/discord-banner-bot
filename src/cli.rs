use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use poise::serenity_prelude::{GenericChannelId, GuildId};

#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct BotCli {
    #[arg(short, long, default_value = "./settings.toml")]
    pub settings_file: PathBuf,
}

#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct UtilCli {
    #[arg(short, long, default_value = "./settings.toml")]
    pub settings_file: PathBuf,
    #[command(subcommand)]
    pub command: UtilCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum UtilCommand {
    RegisterCommands {
        #[command(subcommand)]
        command: GuildOrGlobally,
    },
    UnregisterCommands {
        #[command(subcommand)]
        command: GuildOrGlobally,
    },
    DmServerOwners {
        #[arg(short, long)]
        who: ServerOwners,
        #[arg(short, long)]
        mention_owned_guilds: bool,
        message: String,
    },
    FindMedia {
        #[arg(short, long)]
        channel_id: GenericChannelId,
        #[arg(short, long, default_value = "200")]
        limit: usize,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum GuildOrGlobally {
    InGuild { guild: GuildId },
    Globally,
}

#[derive(Debug, Clone, Parser, ValueEnum)]
pub enum ServerOwners {
    AllOfThem,
    WithActiveSchedule,
}
