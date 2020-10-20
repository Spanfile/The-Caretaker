use serenity::model::id::ChannelId;
use structopt::StructOpt;
use strum::{Display, EnumMessage, EnumString};

#[derive(Debug, EnumString, EnumMessage, StructOpt, Display)]
#[structopt(no_version)]
#[strum(serialize_all = "kebab-case")]
pub enum Action {
    /// Remove the user's message
    #[strum(message = "Remove the user's message")]
    RemoveMessage,
    /// Notify the user with a given message in the same channel as their message
    #[strum(message = "Notify the user")]
    NotifyUser {
        /// The message to send
        message: String,
    },
    /// Notify about the message in a certain channel
    #[strum(message = "Notify about the message in a certain channel")]
    NotifyIn {
        /// The channel where to send message to
        channel: ChannelId,
        /// The message to send
        message: String,
    },
}
