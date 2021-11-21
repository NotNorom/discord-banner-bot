use std::process::exit;

use poise::{say_reply, ArgumentParseError, CommandErrorContext, ErrorContext};
use tracing::error;

use crate::Error;

pub async fn on_error<D>(e: Error, ctx: ErrorContext<'_, D, Error>) {
    match ctx {
        ErrorContext::Setup => {
            // exit the bot if we encounter an error on setup
            error!("Setup error: {:?}", e);
            exit(-1)
        }
        ErrorContext::Listener(event) => {
            error!("Error processing event {:?}: {:?}", event, e);
        }
        ErrorContext::Command(ctx) => {
            // create the error message that should be displayed to the user
            let user_error_msg = if let Some(ArgumentParseError(e)) = e.downcast_ref() {
                // If we caught an argument parse error, give a helpful error message with the
                // command explanation if available

                let mut usage = "Please check the help menu for usage information".into();
                if let CommandErrorContext::Prefix(ctx) = &ctx {
                    if let Some(multiline_help) = &ctx.command.options.multiline_help {
                        usage = multiline_help();
                    }
                }
                format!("**{}**\n{}", e, usage)
            } else {
                e.to_string()
            };
            // send the error message to the user
            if let Err(e) = say_reply(ctx.ctx(), user_error_msg).await {
                error!("Error while user command error: {:?}", e);
            }
        }
        ErrorContext::Autocomplete(err) => {
            error!(
                "Error processing autocomplete. Wile checking: {}",
                err.while_checking
            );
        }
    }
}
