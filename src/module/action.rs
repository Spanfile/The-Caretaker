use diesel_derive_enum::DbEnum;
use serde_json::json;
use serenity::{
    http::Http,
    model::id::{ChannelId, MessageId},
};
use std::borrow::Cow;
use strum::{Display, EnumMessage, EnumString};

// the database schema holds its own version of this enum, remember to modify it as well if modying this one
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

    pub fn friendly_name(&self) -> &str {
        self.kind
            .get_message()
            .unwrap_or_else(|| panic!("missing message for action kind {}", self.kind))
    }

    pub fn description(&self) -> String {
        match self.kind {
            ActionKind::Notify => match (&self.message, self.channel) {
                (None, _) => panic!(format!("invalid action: kind is {} but message is None", self.kind)),
                (Some(msg), None) => format!("In the same channel with `{}`", msg),
                (Some(msg), Some(channel)) => format!("In <#{}> with `{}`", channel, msg),
            },
            ActionKind::RemoveMessage => {
                // Discord requires the embed field to always have *some* value but they don't document the requirement
                // anywhere. omitting the value has Discord respond with a very unhelpful error message that Serenity
                // can't do anything with, other than complain about invalid JSON
                String::from("Remove the message, nothing special about it")
            }
        }
    }

    pub async fn run(&self, http: &Http, channel: ChannelId, msg: MessageId) -> anyhow::Result<()> {
        match self.kind {
            ActionKind::RemoveMessage => {
                http.delete_message(channel.0, msg.0).await?;
            }
            ActionKind::Notify => {
                let message = self.message.clone().expect("missing message in action");
                let value = json!({ "content": message });

                match self.channel {
                    Some(notify_channel) => http.send_message(notify_channel.0, &value),
                    None => http.send_message(channel.0, &value),
                }
                .await?;
            }
        }

        Ok(())
    }
}
