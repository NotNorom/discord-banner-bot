pub mod banner;
pub mod help;

use poise::serenity_prelude::json::Value;

use crate::{Context, Error};

/// Register application commands in this guild
#[poise::command(prefix_command, hide_in_help)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::samples::register_application_commands(ctx, false).await?;
    Ok(())
}

/// Register application commands globally
#[poise::command(prefix_command, hide_in_help, owners_only)]
pub async fn register_globally(ctx: Context<'_>) -> Result<(), Error> {
    poise::samples::register_application_commands(ctx, true).await?;
    Ok(())
}

/// Unregister application commands in this guild
#[poise::command(prefix_command, hide_in_help)]
pub async fn unregister(ctx: Context<'_>) -> Result<(), Error> {
    let guild = match ctx.guild() {
        Some(x) => x,
        None => {
            ctx.say("Must be called in guild").await?;
            return Ok(());
        }
    };
    let is_guild_owner = ctx.author().id == guild.owner_id;

    if !is_guild_owner {
        ctx.say("Can only be used by server owner").await?;
        return Ok(());
    }

    ctx.say("Deleting all commands...").await?;
    ctx.discord()
        .http
        .create_guild_application_commands(guild.id.0, &Value::Array(vec![]))
        .await?;
    Ok(())
}
