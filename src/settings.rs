use anyhow::Context;
use config::Config;
use once_cell::sync::OnceCell;
use serde::Deserialize;

use crate::Error;

static SETTINGS: OnceCell<Settings> = OnceCell::new();

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bot: Bot,
    pub provider: Provider,
}

#[derive(Debug, Deserialize)]
pub struct Bot {
    pub log_level: String,
    pub prefix: String,
    pub token: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Provider {
    pub imgur: Imgur,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Imgur {
    pub client_id: String,
    pub secret: String,
}

pub fn init() -> Result<(), Error> {
    let settings = Config::builder()
        .add_source(config::File::with_name("settings"))
        .build()
        .context("Settings could not be loaded")?
        .try_deserialize()
        .context("Settings could not be deserialized")?;

    let _ = SETTINGS.set(settings);

    Ok(())
}

pub fn settings() -> &'static Settings {
    SETTINGS.get().expect("Settings are not initialized")
}