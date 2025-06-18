pub mod cli;
pub mod commands;
pub mod constants;
pub mod database;
pub mod error;
pub mod event_handler;
pub mod finding_media;
pub mod schedule;
pub mod schedule_runner;
pub mod setting_banner;
pub mod settings;
pub mod shutdown;
pub mod startup;
pub mod state;
pub mod utils;

pub use error::Error;
pub use settings::Settings;
pub use state::State;

type Context<'a> = poise::Context<'a, State, Error>;
