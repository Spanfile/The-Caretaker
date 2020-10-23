use diesel_derive_enum::DbEnum;
use serenity::model::id::ChannelId;
use std::borrow::Cow;
use strum::{Display, EnumMessage, EnumString};

#[derive(Debug, EnumString, EnumMessage, Display, Copy, Clone, DbEnum)]
#[strum(serialize_all = "kebab-case")]
#[DieselType = "Action_kind"]
pub enum ActionKind {
    /// Remove the user's message
    #[strum(message = "Remove the user's message")]
    RemoveMessage,
    /// Notify about the message in a certain channel
    #[strum(message = "Notify about the message")]
    Notify,
}

#[derive(Debug)]
pub struct Action<'a> {
    pub kind: ActionKind,
    pub channel: Option<ChannelId>,
    pub message: Option<Cow<'a, str>>,
}

impl<'a> Action<'a> {
    pub fn remove_message() -> Self {
        Self {
            kind: ActionKind::RemoveMessage,
            channel: None,
            message: None,
        }
    }

    pub fn notify(channel: Option<ChannelId>, message: Cow<'a, str>) -> Self {
        Self {
            kind: ActionKind::Notify,
            channel,
            message: Some(message),
        }
    }
}
