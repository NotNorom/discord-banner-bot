pub mod banner;
pub mod help;

use poise::serenity_prelude::CacheHttp;

use crate::{Context, Error};

/// Register application commands in this guild
#[poise::command(prefix_command, hide_in_help, owners_only)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// Register application commands globally
#[poise::command(prefix_command, hide_in_help, owners_only)]
pub async fn register_globally(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands(ctx, true).await?;
    Ok(())
}

/// Unregister application commands in this guild
#[poise::command(prefix_command, hide_in_help, owners_only)]
pub async fn unregister(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = ctx.partial_guild().await else {
        ctx.say("Must be called in guild").await?;
        return Ok(());
    };

    let is_allowed = {
        let is_guild_owner = ctx.author().id == guild.owner_id;
        let is_bot_owner = ctx.framework().options().owners.contains(&ctx.author().id);
        is_guild_owner || is_bot_owner
    };

    if !is_allowed {
        ctx.say("Can only be used by server owner or bot owners").await?;
        return Ok(());
    }

    ctx.say("Deleting all commands...").await?;
    guild.id.set_commands(ctx.serenity_context().http(), &[]).await?;
    Ok(())
}

/// List all servers tis bot is in. Only public servers are shown by name.
#[poise::command(slash_command, prefix_command, hide_in_help, owners_only)]
pub async fn servers(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::servers(ctx).await.map_err(Into::into)
}

pub fn commands() -> Vec<poise::Command<crate::State, crate::Error>> {
    vec![
        banner::current_schedule(),
        banner::current_banner(),
        banner::start_for_guild(),
        banner::start(),
        banner::stop_for_guild(),
        banner::stop(),
        help::help(),
        register_globally(),
        register(),
        servers(),
        unregister(),
    ]
}
