use anyhow::Context;
use config::Config;
use serde::Deserialize;
use std::sync::OnceLock;

use crate::Error;

static SETTINGS: OnceLock<Settings> = OnceLock::new();

/// Wrapper for all settings
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bot: Bot,
    pub scheduler: Scheduler,
    pub database: Database,
    pub provider: Provider,
}

impl Settings {
    /// Load and deserialize settings into static struct
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

    /// Get settings. Panics if called before [init].
    pub fn get() -> &'static Settings {
        SETTINGS.get().expect("Settings are not initialized")
    }
}

/// Bot settings
#[derive(Debug, Deserialize)]
pub struct Bot {
    pub log_level: String,
    pub prefix: String,
    pub token: String,
}

/// Scheduler settings
#[derive(Debug, Deserialize)]
pub struct Scheduler {
    pub capacity: usize,
    pub minimum_interval: u64,
    pub default_interval: u64,
    pub maximum_interval: u64,
}

/// Database settings
#[derive(Debug, Deserialize)]
pub struct Database {
    pub host: String,
    pub prefix: String,
}

/// Settings for every provider
#[derive(Debug, Deserialize, Clone)]
pub struct Provider {
    pub imgur: Option<Imgur>,
}

/// Imgur settings
#[derive(Debug, Deserialize, Clone)]
pub struct Imgur {
    pub client_id: String,
    pub secret: String,
}
