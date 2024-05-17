use clap::{Parser, Subcommand, ValueEnum};
use poise::serenity_prelude::GuildId;

#[derive(Parser)]
pub struct BotCli {}

#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct UtilCli {
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
