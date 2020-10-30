use super::Matcher;
use crate::module::ModuleKind;
use chrono::{DateTime, Duration, Utc};
use circular_queue::CircularQueue;
use log::*;
use nilsimsa::Nilsimsa;
use serenity::{
    async_trait,
    model::{
        channel::Message,
        id::{ChannelId, GuildId, UserId},
    },
};
use std::collections::{hash_map::Entry, HashMap};

const HISTORY_SIZE: usize = 3;

pub struct Crosspost {
    msg_history: HashMap<(GuildId, UserId), History>,
}

#[derive(Debug)]
struct History {
    history: CircularQueue<MessageInformation>,
}

#[derive(Debug)]
struct MessageInformation {
    hash: String,
    channel: ChannelId,
    timestamp: DateTime<Utc>,
}

#[async_trait]
impl Matcher for Crosspost {
    fn build() -> (ModuleKind, Self) {
        (
            ModuleKind::Crosspost,
            Self {
                msg_history: HashMap::new(),
            },
        )
    }

    async fn is_match(&mut self, msg: &Message) -> bool {
        let content = &msg.content;

        // .len() on a string returns its length in bytes, not in graphemes, so messages such as 'äää' would be
        // considered since its length is six bytes, but only three characters
        if content.len() < 5 {
            debug!("Not matching a message of length {}", content.len());
            return false;
        }

        match self.msg_history.entry((msg.guild_id.unwrap(), msg.author.id)) {
            Entry::Occupied(mut entry) => {
                let history = entry.get_mut();

                if history.compare(msg, 80) {
                    return true;
                } else {
                    history.push(msg);
                }
            }
            Entry::Vacant(entry) => {
                let mut new_history = History::default();
                new_history.push(msg);
                entry.insert(new_history);
            }
        }

        false
    }
}

impl History {
    fn push(&mut self, msg: &Message) {
        let info = MessageInformation {
            hash: hash(&msg.content),
            channel: msg.channel_id,
            timestamp: msg.timestamp,
        };
        self.history.push(info);
    }

    fn compare(&self, msg: &Message, threshold: i16) -> bool {
        let hash = hash(&msg.content);

        for hist in self
            .history
            .iter()
            .filter(|info| info.channel != msg.channel_id && (Utc::now() - info.timestamp) < Duration::seconds(3600))
        {
            let comparison = nilsimsa::compare(&hash, &hist.hash);
            debug!("{} : {} -> {}", hash, hist.hash, comparison);

            if comparison > threshold {
                return true;
            }
        }

        false
    }
}

impl Default for History {
    fn default() -> Self {
        Self {
            history: CircularQueue::with_capacity(HISTORY_SIZE),
        }
    }
}

fn hash(message: &str) -> String {
    let mut hasher = Nilsimsa::new();
    hasher.update(message);
    hasher.digest()
}
