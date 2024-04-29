use config::{Config, ConfigError};
use serde::Deserialize;
use std::sync::OnceLock;

static SETTINGS: OnceLock<Settings> = OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error(transparent)]
    Config(#[from] ConfigError),
}

/// Wrapper for all settings
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bot: Bot,
    pub scheduler: Scheduler,
    pub database: Database,
}

impl Settings {
    /// Load and deserialize settings into static struct
    pub fn init() -> Result<(), SettingsError> {
        let settings = Config::builder()
            .add_source(config::File::with_name("settings"))
            .build()?
            .try_deserialize()?;

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
