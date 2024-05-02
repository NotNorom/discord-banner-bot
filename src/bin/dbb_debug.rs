use discord_banner_bot::{
    error::Error, finding_media::find_media_in_channel, utils::start_logging, Settings,
};
use poise::serenity_prelude::{self, ChannelId};
use tokio_stream::StreamExt;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    Settings::init()?;
    let settings = Settings::get();
    info!("Using log level: {}", settings.bot.log_level);

    start_logging("dbb_debug=info,reqwest=info,poise=info,serenity=info,warn");

    let http = serenity_prelude::HttpBuilder::new(&settings.bot.token).build();

    #[allow(clippy::unreadable_literal)]
    let channel_id = ChannelId::new(1169436925350924390);

    info!("finding media in channel {}", channel_id);
    let messages = find_media_in_channel(&http, &channel_id)
        .take(100)
        .filter_map(Result::ok)
        .collect::<Vec<_>>()
        .await;

    for message in messages {
        info!("{message}");
    }

    Ok(())
}
