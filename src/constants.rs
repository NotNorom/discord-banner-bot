//! Constants
//!
//! See invidividual items' description

use poise::serenity_prelude;

/// The user agent for the reqwest instance that's talking to discord
pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_REPOSITORY"), " - ", env!("CARGO_PKG_VERSION"));

/// Maximum image size in bytes for uploads to discord
pub const MAXIMUM_IMAGE_SIZE: usize = 1024 * 1024 * 10; // 10mb

/// Maximum message length for discord
pub const DISCORD_MESSAGE_CONTENT_LIMIT: usize = serenity_prelude::constants::MESSAGE_CODE_LIMIT;
