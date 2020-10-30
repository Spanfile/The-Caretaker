use super::Matcher;
use crate::module::ModuleKind;
use circular_queue::CircularQueue;
use log::*;
use nilsimsa::Nilsimsa;
use serenity::{
    async_trait,
    model::{
        channel::Message,
        id::{GuildId, UserId},
    },
};
use std::collections::{hash_map::Entry, HashMap};

const HISTORY_SIZE: usize = 3;

pub struct Crosspost {
    msg_history: HashMap<(GuildId, UserId), History>,
}

#[derive(Debug)]
struct History {
    history: CircularQueue<String>,
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
        match self.msg_history.entry((msg.guild_id.unwrap(), msg.author.id)) {
            Entry::Occupied(mut entry) => {
                let history = entry.get_mut();

                if history.compare(content, 80) {
                    return true;
                } else {
                    history.push(content);
                }
            }
            Entry::Vacant(entry) => {
                let mut new_history = History::default();
                new_history.push(content);
                entry.insert(new_history);
            }
        }

        false
    }
}

impl History {
    fn hash(message: &str) -> String {
        let mut hasher = Nilsimsa::new();
        hasher.update(message);
        hasher.digest()
    }

    fn push(&mut self, message: &str) {
        self.history.push(History::hash(message));
    }

    fn compare(&self, message: &str, threshold: i16) -> bool {
        let hash = History::hash(message);
        for hist in self.history.iter() {
            let comparison = nilsimsa::compare(&hash, hist);
            debug!("{} : {} -> {}", hash, hist, comparison);

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
