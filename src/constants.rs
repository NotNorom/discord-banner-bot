//! Constants
//! 
//! See invidividual items' description

/// The user agent for the reqwest instance that's talking to e.g. imgur for the albums
pub const USER_AGENT: &str = concat!(
    "github.com/NotNorom/discord-banner-bot, ",
    env!("CARGO_PKG_VERSION")
);

/// Minimum amount of minutes between banner changes
pub const MINIMUM_INTERVAL: u64 = 15;

/// Default amount of minutes between banner changes
pub const DEFAULT_INTERVAL: u64 = 30;

/// Maximum amount of minutes between banner changes
pub const MAXIMUM_INTERVAL: u64 = 2880; // 48h
