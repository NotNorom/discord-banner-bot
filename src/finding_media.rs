use std::{fmt::Display, num::NonZeroUsize};

use poise::serenity_prelude::{
    CacheHttp, Error, GenericChannelId, Message, futures::stream as futures_stream,
    small_fixed_array::FixedString,
};
use tokio_stream::{Stream, StreamExt};
use tracing::instrument;

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
#[instrument(skip_all)]
pub fn find_media_in_channel<'a>(
    http: &'a impl CacheHttp,
    channel_id: &GenericChannelId,
    limit: usize,
) -> impl Stream<Item = Result<MediaWithMessage, Error>> + 'a {
    let stream =
        futures_stream::StreamExt::then(channel_id.messages_iter(http).take(limit), |message| async move {
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
                // only use embeds, don't use thumbnails
                // this is done to avoid requests to 3rd parties
                match (&embed.image, &embed.thumbnail) {
                    (None, _) => {}
                    (Some(img), _) => {
                        // tracing::trace!("{} - {} EMBED", message.link(), img.url);
                        result.push(Ok(MediaWithMessage::new(img.url.clone(), message.clone())));
                    }
                }
            }

            for attachment in &message.attachments {
                if attachment.content_type.as_ref().is_some_and(media_type_is_image) {
                    // tracing::trace!("{} - {} ATTACHMENT", message.link(), attachment.url);
                    result.push(Ok(MediaWithMessage::new(attachment.url.clone(), message.clone())));
                }
            }

            futures_stream::iter(result)
        });
    futures_stream::StreamExt::flatten(stream)
}

/// Return the last message the bot is gonna look at for that schedule
#[instrument(skip_all)]
pub async fn last_reachable_message(http: &impl CacheHttp, schedule: &Schedule) -> Option<Message> {
    let limit = schedule
        .message_limit()
        .map(NonZeroUsize::get)
        .unwrap_or_default();

    let messages: Vec<Message> = schedule
        .channel_id()
        .messages_iter(http)
        .take(limit)
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
