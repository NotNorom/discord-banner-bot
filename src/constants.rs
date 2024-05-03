//! Constants
//!
//! See invidividual items' description

use poise::serenity_prelude;

/// The user agent for the reqwest instance that's talking to discord
pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_REPOSITORY"), " - ", env!("CARGO_PKG_VERSION"));

/// Minimum amount of minutes between banner changes
pub const MINIMUM_INTERVAL: u64 = 15;

/// Default amount of minutes between banner changes
pub const DEFAULT_INTERVAL: u64 = 30;

/// Maximum amount of minutes between banner changes
pub const MAXIMUM_INTERVAL: u64 = 60 * 48; // 48h

/// Default amount of messages to look back for
pub const DEFAULT_MESSAGE_LIMIT: usize = 100;

/// Maximum amount of messages to look back for
pub const MAXIMUM_MESSAGE_LIMIT: usize = 200;

/// Maximum image size in bytes for uploads to discord
pub const MAXIMUM_IMAGE_SIZE: usize = 1024 * 1024 * 10; // 10mb

/// Maximum message length for discord
pub const DISCORD_MESSAGE_CONTENT_LIMIT: usize = serenity_prelude::constants::MESSAGE_CODE_LIMIT;
