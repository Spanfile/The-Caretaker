use super::Matcher;
use crate::{
    module::{cache::ModuleCache, settings::Settings, ModuleKind},
    DbConnection,
};
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
    prelude::TypeMap,
};
use std::{
    collections::{hash_map::Entry, HashMap},
    convert::TryInto,
    sync::Arc,
};
use tokio::sync::RwLock;

const HISTORY_SIZE: usize = 3;

pub struct Crosspost {
    msg_history: HashMap<(GuildId, UserId), History>,
    userdata: Arc<RwLock<TypeMap>>,
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
    async fn build(userdata: Arc<RwLock<TypeMap>>) -> (ModuleKind, Self) {
        (
            ModuleKind::Crosspost,
            Self {
                msg_history: HashMap::new(),
                userdata,
            },
        )
    }

    async fn is_match(&mut self, msg: &Message) -> bool {
        let userdata = self.userdata.read().await;
        let module_cache = userdata.get::<ModuleCache>().expect("missing ModuleCache in userdata");
        let db = userdata
            .get::<DbConnection>()
            .expect("missing DbConnection in userdata")
            .lock()
            .await;
        let module = module_cache
            .get(
                msg.guild_id.expect("missing guild ID in message"),
                ModuleKind::Crosspost,
            )
            .await;
        let settings = module.get_settings(&db).expect("failed to get settings for module");

        let content = &msg.content;

        // .len() on a string returns its length in bytes, not in graphemes, so messages such as 'äää' would be
        // considered since its length is six bytes, but only three characters
        if content.len()
            < settings
                .get("minimum_length")
                .expect("failed to get minimum length setting")
                .try_into()
                .expect("failed to read minimum length setting as usize")
        {
            debug!("Not matching a message of length {}", content.len());
            return false;
        }

        match self.msg_history.entry((msg.guild_id.unwrap(), msg.author.id)) {
            Entry::Occupied(mut entry) => {
                let history = entry.get_mut();

                if history.compare(
                    msg,
                    settings
                        .get("threshold")
                        .expect("failed to get threshold setting")
                        .try_into()
                        .expect("failed to read threshold setting as u16"),
                    Duration::seconds(
                        settings
                            .get("timeout")
                            .expect("failed to get timeout setting")
                            .try_into()
                            .expect("failed to read timeout setting as usize"),
                    ),
                ) {
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

    fn compare(&self, msg: &Message, threshold: i16, timeout: Duration) -> bool {
        let hash = hash(&msg.content);

        for hist in self
            .history
            .iter()
            .filter(|info| info.channel != msg.channel_id && (Utc::now() - info.timestamp) < timeout)
        {
            let comparison = nilsimsa::compare(&hash, &hist.hash);
            debug!("{} : {} -> {}", hash, hist.hash, comparison);

            if comparison >= threshold {
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
