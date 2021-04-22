use crate::error::{ArgumentError, InternalError};
use diesel_derive_enum::DbEnum;
use dynfmt::{Format, SimpleCurlyFormat};
use erased_serde::Serialize;
use serenity::{
    model::{channel::Message, id::ChannelId},
    prelude::*,
    CacheAndHttp,
};
use std::{borrow::Cow, collections::HashMap};
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
            ActionKind::RemoveMessage => {
                // Discord requires the embed field to always have *some* value but they don't document the requirement
                // anywhere. omitting the value has Discord respond with a very unhelpful error message that Serenity
                // can't do anything with, other than complain about invalid JSON
                String::from("Remove the message, nothing special about it")
            }
            ActionKind::Notify => match (&self.message, self.channel) {
                (None, _) => panic!("invalid action: kind is {} but message is None", self.kind),
                (Some(msg), None) => format!("In the same channel with `{}`", msg),
                (Some(msg), Some(channel)) => format!("In <#{}> with `{}`", channel, msg),
            },
        }
    }

    pub async fn run(self, cache_http: &CacheAndHttp, msg: &Message) -> anyhow::Result<()> {
        match self.kind {
            ActionKind::RemoveMessage => {
                msg.delete(cache_http).await?;
            }
            ActionKind::Notify => {
                let message = self
                    .message
                    .ok_or_else(|| InternalError::ImpossibleCase(String::from("missing message in action")))?;
                let formatted = SimpleCurlyFormat
                    .format(message.as_ref(), build_format_args(msg))
                    .map_err(|e| ArgumentError::InvalidNotifyFormat(e.to_string()))?;

                match self.channel {
                    Some(notify_channel) => notify_channel,
                    None => msg.channel_id,
                }
                .send_message(&cache_http.http, |m| m.content(formatted))
                .await?;
            }
        }

        Ok(())
    }
}

fn build_format_args<'a>(msg: &'a Message) -> HashMap<&'static str, Box<dyn Serialize + 'a>> {
    let mut args: HashMap<&'static str, Box<dyn Serialize>> = HashMap::new();
    args.insert("user", Box::new(msg.author.mention().to_string()));
    args.insert("channel", Box::new(msg.channel_id.mention().to_string()));
    args.insert("timestamp", Box::new(msg.timestamp));
    args.insert("link", Box::new(msg.link()));
    args
}
