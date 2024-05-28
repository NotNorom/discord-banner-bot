use poise::serenity_prelude::{AttachmentType, ChannelId, CreateEmbed, EmbedMessageBuilding};

/// Command to submit an image for the banner rotation
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    required_bot_permissions = "SEND_MESSAGES",
)]
pub async fn submit_image(
    ctx: Context<'_>,
    #[description = "Image to submit"] image: AttachmentType,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;

    // Define the private channel ID where submissions will be sent
    let private_channel_id = ChannelId(123456789012345678); // Replace with your private channel ID

    // Create an embed with the submitted image
    let embed = CreateEmbed::new()
        .title("New Image Submission")
        .description(format!("Submitted by <@{}>", ctx.author().id))
        .image(image.url())
        .colour((0, 255, 0));

    // Send the embed to the private channel
    private_channel_id
        .send_message(ctx.http(), |m| m.set_embed(embed))
        .await?;

    // Acknowledge the submission
    poise::send_reply(
        ctx,
        CreateReply::default()
            .content("Image submitted for review.")
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Command to approve an image
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    required_permissions = "MANAGE_GUILD",
)]
pub async fn approve_image(
    ctx: Context<'_>,
    #[description = "Image URL to approve"] image_url: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(CommandErr::GuildOnly)?;

    // Add the approved image to the rotation (implement your own storage for approved images)
    // For simplicity, we'll just log it here
    println!("Approved image: {}", image_url);

    poise::send_reply(
        ctx,
        CreateReply::default()
            .content("Image approved.")
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
