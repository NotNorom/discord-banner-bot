pub mod commands;
pub mod constants;
pub mod database;
pub mod error;
pub mod finding_media;
pub mod schedule;
pub mod schedule_runner;
pub mod setting_banner;
pub mod settings;
pub mod startup;
pub mod utils;

pub use error::Error;
pub use settings::Settings;
use startup::State;

type Data = State;
type Context<'a> = poise::Context<'a, Data, Error>;
