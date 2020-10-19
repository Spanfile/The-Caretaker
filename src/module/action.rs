use serenity::model::id::ChannelId;
use strum::{EnumMessage, EnumString};

#[derive(Debug, EnumString, EnumMessage)]
#[strum(serialize_all = "kebab-case")]
pub enum Action {
    #[strum(message = "Remove the user's message")]
    RemoveMessage,
    #[strum(message = "Notify the user with a message")]
    NotifyUser { message: String },
    #[strum(message = "Notify about the message in a channel")]
    NotifyIn { channel: ChannelId, message: String },
}
