use std::fmt::Display;

use poise::serenity_prelude::{
    futures::TryStreamExt, small_fixed_array::FixedString, CacheHttp, ChannelId, Error, Message,
};
use tokio_stream::Stream;

#[derive(Debug)]
pub struct MessageWithMedia {
    pub message: Message,
    pub media: Vec<FixedString>,
}

impl MessageWithMedia {
    pub fn new(message: Message, media: impl Into<Vec<FixedString>>) -> Self {
        let media = media.into();
        Self { message, media }
    }
}

impl Display for MessageWithMedia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Message {
            author, timestamp, ..
        } = &self.message;

        write!(f, "[{} {}] {}\n", timestamp, author.name, self.message.link())?;
        for media in &self.media {
            writeln!(f, "  - {media}")?;
        }
        writeln!(f, "")
    }
}

pub fn find_media_in_channel<'a>(
    http: &'a impl CacheHttp,
    channel_id: &ChannelId,
) -> impl Stream<Item = Result<MessageWithMedia, Error>> + 'a {
    channel_id
        .messages_iter(http)
        .try_filter_map(|message| async move {
            let mut images = vec![];
            for embed in message.embeds.iter() {
                match (&embed.image, &embed.thumbnail) {
                    (None, None) => continue,
                    (None, Some(thumb)) => images.push(thumb.url.clone()),
                    (Some(img), _) => images.push(img.url.clone()),
                }
            }

            for attachment in message.attachments.iter() {
                if attachment
                    .content_type
                    .as_ref()
                    .map(media_type_is_image)
                    .unwrap_or(false)
                {
                    images.push(attachment.url.clone());
                }
            }

            if images.is_empty() {
                return Ok(None);
            }

            Ok(Some(MessageWithMedia::new(message, images)))
        })
}

pub fn media_type_is_image(media_type: impl AsRef<str>) -> bool {
    match media_type.as_ref().to_lowercase().as_str() {
        "image/png" | "image/jpg" | "image/jpeg" | "image/gif" => true,
        _ => false,
    }
}
