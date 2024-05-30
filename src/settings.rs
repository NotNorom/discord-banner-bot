use config::{Config, ConfigError};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::OnceLock,
};

static SETTINGS: OnceLock<Settings> = OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error(transparent)]
    Config(#[from] ConfigError),
}

/// Wrapper for all settings
#[derive(Debug, Deserialize)]
pub struct Settings {
    /// Bot settings
    pub bot: Bot,
    /// Scheduler settings
    pub scheduler: Scheduler,
    /// Database settings
    pub database: Database,
}

impl Settings {
    #[allow(clippy::missing_panics_doc)]
    /// Load and deserialize settings into static struct
    pub fn init() -> Result<(), SettingsError> {
        Self::init_from_path(&PathBuf::from_str("settings").expect("hard coded"))
    }

    /// Load and deserialize settings into static struct from path
    pub fn init_from_path(path: &Path) -> Result<(), SettingsError> {
        let path = path.to_string_lossy();

        let settings = Config::builder()
            .add_source(config::File::with_name(&path))
            .build()?
            .try_deserialize()?;

        let _ = SETTINGS.set(settings);

        Ok(())
    }

    /// Get settings.
    ///
    /// # Panics
    /// Panics if called before [init].
    pub fn get() -> &'static Settings {
        SETTINGS.get().expect("Settings are not initialized")
    }
}

/// Bot settings
#[derive(Debug, Deserialize)]
pub struct Bot {
    /// Log level
    pub log_level: String,
    /// Prefix
    pub prefix: String,
    /// Token
    pub token: String,
}

/// Scheduler settings
#[derive(Debug, Deserialize)]
pub struct Scheduler {
    /// How many schedules can run at the same time
    pub capacity: usize,
    /// Minimum amount of minutes between banner changes
    pub minimum_interval: u64,
    /// Default amount of minutes between banner changes
    pub default_interval: u64,
    /// Maximum amount of minutes between banner changes
    pub maximum_interval: u64,
    /// Default amount of messages to look back for
    pub default_message_limit: usize,
    /// Maximum amount of messages to look back for
    pub maximum_message_limit: usize,
}

/// Database settings
#[derive(Debug, Deserialize)]
pub struct Database {
    /// Host
    pub host: String,
    /// Prefix
    pub prefix: String,
}
