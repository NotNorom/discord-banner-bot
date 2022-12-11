#![allow(unused_imports)]
use std::time::Duration;

use poise::serenity_prelude::{GuildChannel, MessageBuilder};

use tokio::time::sleep;

use crate::{Context, Error};

/// Sets the channel where the bot is supposed to send updates
#[poise::command(prefix_command, slash_command, required_permissions = "MANAGE_GUILD")]

pub async fn notification_channel(
    ctx: Context<'_>,
    #[description = "Channel to send updates to"] _channel: GuildChannel,
) -> Result<(), Error> {
    use poise::serenity_prelude::ChannelType::*;

    let reply = ctx.say("This command is being worked on.").await?;
    sleep(Duration::from_secs(7)).await;
    reply.delete(ctx).await?;

    // if !matches!(
    //     channel.kind,
    //     Text | Private | News | NewsThread | PublicThread | PrivateThread
    // ) {
    //     let _ = ctx
    //         .say("Unsupported channel type. Please select a text channel or thread")
    //         .await?;
    //     return Ok(());
    // }

    // let message = MessageBuilder::new()
    //     .push("Channel ")
    //     .channel(channel)
    //     .push(" will be used when sending updates or error messages.")
    //     .build();

    // ctx.say(message).await?;

    Ok(())
}
