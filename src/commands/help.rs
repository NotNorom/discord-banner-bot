use tracing::instrument;

use crate::{Context, Error};

/// Display a list of all available commands
#[poise::command(slash_command, prefix_command)]
#[instrument(skip_all)]
pub async fn help(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("If you need help, join: https://discord.gg/MMJFtCtYPP and ping 'norom'")
        .await?;

    Ok(())
}
