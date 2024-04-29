pub mod banner_changer;
pub mod commands;
pub mod constants;
pub mod database;
pub mod messages_with_media;
pub mod error;
pub mod guild_id_ext;
pub mod schedule;
pub mod settings;
pub mod startup;
pub mod utils;

pub use error::Error;
pub use settings::Settings;
use startup::State;

type Data = State;
type Context<'a> = poise::Context<'a, Data, Error>;
