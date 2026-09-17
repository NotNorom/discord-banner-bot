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

    for command in &ctx.framework().options.commands {
        let Command {
            name,
            name_localizations,
            qualified_name,
            identifying_name,
            source_code_name,
            category,
            hide_in_help,
            description,
            description_localizations,
            help_text,
            default_member_permissions,
            required_permissions,
            required_bot_permissions,
            owners_only,
            guild_only,
            dm_only,
            parameters,
            aliases,
            invoke_on_edit,
            ..
        } = command;

        if *hide_in_help {
            continue;
        }

        message = message.push_bold_safe(name as &str);
        message = message.push_safe(": ");
        message = message
            .push_bold_line_safe(help_text.as_ref().unwrap_or(&Cow::Borrowed("no help available")) as &str);

        if *owners_only && ctx.data().owners().contains(&ctx.author().id) {
            message = message.push_bold_safe(name as &str);
            message = message.push_safe(" (Owner only): ");
            message = message.push_bold_line_safe(
                help_text.as_ref().unwrap_or(&Cow::Borrowed("no help available")) as &str,
            );
        }
    }

    poise::send_reply(
        ctx,
        CreateReply::default().content(message.build()).ephemeral(true),
    )
    .await?;

    Ok(())
}
