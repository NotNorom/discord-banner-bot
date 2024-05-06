use std::{fmt::Display, num::NonZeroUsize};

use poise::serenity_prelude::{
    futures::stream as futures_stream, small_fixed_array::FixedString, CacheHttp, ChannelId, Error, Message,
};
use tokio_stream::{Stream, StreamExt};

use crate::schedule::Schedule;

#[derive(Debug)]
pub struct MediaWithMessage {
    pub media: FixedString,
    pub message: Message,
}

impl MediaWithMessage {
    pub fn new(media: impl Into<FixedString>, message: Message) -> Self {
        Self {
            media: media.into(),
            message,
        }
    }
}

impl Display for MediaWithMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Message {
            author, timestamp, ..
        } = &self.message;

        writeln!(
            f,
            "[{} {}] {}, {}",
            timestamp,
            author.name,
            self.message.link(),
            self.media
        )
    }
}

/// Creates a stream of media in a channel
pub fn find_media_in_channel<'a>(
    http: &'a impl CacheHttp,
    channel_id: &ChannelId,
) -> impl Stream<Item = Result<MediaWithMessage, Error>> + 'a {
    let stream = futures_stream::StreamExt::then(channel_id.messages_iter(http), |message| async move {
        let mut result = vec![];

        let message = match message {
            Ok(message) => message,
            Err(err) => {
                tracing::error!("fetching message: {err:?}");
                result.push(Err(err));
                return futures_stream::iter(result);
            }
        };

        for embed in &message.embeds {
            // read  the match block as:
            // only use a thumbnail if there is no embed
            match (&embed.image, &embed.thumbnail) {
                (None, None) => continue,
                (None, Some(thumb)) => {
                    result.push(Ok(MediaWithMessage::new(thumb.url.clone(), message.clone())));
                }
                (Some(img), _) => result.push(Ok(MediaWithMessage::new(img.url.clone(), message.clone()))),
            }
        }

        for attachment in &message.attachments {
            if attachment.content_type.as_ref().is_some_and(media_type_is_image) {
                result.push(Ok(MediaWithMessage::new(attachment.url.clone(), message.clone())));
            }
        }

        futures_stream::iter(result)
    });
    futures_stream::StreamExt::flatten(stream)
}

/// Return the last message the bot is gonna look at for that schedule
pub async fn last_reachable_message<'a>(http: &'a impl CacheHttp, schedule: &Schedule) -> Option<Message> {
    let messages: Vec<Message> = schedule
        .channel_id()
        .messages_iter(http)
        .take(
            schedule
                .message_limit()
                .map(NonZeroUsize::get)
                .unwrap_or_default(),
        )
        .filter_map(Result::ok)
        .collect()
        .await;
    messages.last().cloned()
}

pub fn media_type_is_image(media_type: impl AsRef<str>) -> bool {
    matches!(
        media_type.as_ref().to_lowercase().as_str(),
        "image/png" | "image/jpg" | "image/jpeg" | "image/gif"
    )
}
