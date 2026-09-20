use poise::{Command, CreateReply, serenity_prelude::MessageBuilder};
use tracing::instrument;

use crate::{Context, Error};

/// Display a list of all available commands
#[poise::command(slash_command, prefix_command)]
#[instrument(skip_all)]
pub async fn help(ctx: Context<'_>) -> Result<(), Error> {
    let mut message = MessageBuilder::new().push_bold_line_safe(
        "Quick help. For more support join: https://discord.gg/MMJFtCtYPP and ping 'norom'.",
    );

    const NO_HELP_AVAILABLE: Cow<str> = Cow::Borrowed("no help available");

    for command in &ctx.framework().options.commands {
        let Command {
            name,
            name_localizations: _,
            qualified_name: _,
            identifying_name: _,
            source_code_name: _,
            category: _,
            hide_in_help,
            description: _,
            description_localizations: _,
            help_text,
            default_member_permissions: _,
            required_permissions: _,
            required_bot_permissions: _,
            owners_only,
            guild_only: _,
            dm_only: _,
            parameters: _,
            aliases: _,
            invoke_on_edit: _,
            prefix_action: _,
            slash_action: _,
            context_menu_action: _,
            subcommands: _,
            subcommand_required: _,
            manual_cooldowns: _,
            cooldowns: _,
            cooldown_config: _,
            reuse_response: _,
            nsfw_only: _,
            on_error: _,
            checks: _,
            custom_data: _,
            track_deletion: _,
            broadcast_typing: _,
            context_menu_name: _,
            ephemeral: _,
            install_context: _,
            interaction_context: _,
            ..
        } = command;

        if *hide_in_help {
            continue;
        }

        message = message.push_bold_safe(name as &str);
        message = message.push_safe(": ");
        message = message.push_bold_line_safe(help_text.as_ref().unwrap_or(&NO_HELP_AVAILABLE) as &str);

        if *owners_only && ctx.data().owners().contains(&ctx.author().id) {
            message = message.push_bold_safe(name as &str);
            message = message.push_safe(" (Owner only): ");
            message = message.push_bold_line_safe(help_text.as_ref().unwrap_or(&NO_HELP_AVAILABLE) as &str);
        }
    }

    poise::send_reply(
        ctx,
        CreateReply::default().content(message.build()).ephemeral(true),
    )
    .await?;

    Ok(())
}
