pub mod album_provider;
pub mod banner_scheduler;
pub mod commands;
pub mod constants;
pub mod database;
pub mod error;
pub mod guild_id_ext;
pub mod settings;
pub mod startup;
pub mod utils;

pub use error::Error;
pub use settings::Settings;
use startup::State;

type Data = State;
type Context<'a> = poise::Context<'a, Data, Error>;
