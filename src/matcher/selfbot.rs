use super::Matcher;
use crate::module::{settings::SelfbotSettings, ModuleKind};
use serenity::{async_trait, model::channel::Message, prelude::TypeMap};
use std::sync::Arc;
use tokio::sync::RwLock;

const RICH_EMBED: &str = "rich";

pub struct Selfbot {}

#[async_trait]
impl Matcher for Selfbot {
    type SettingsType = SelfbotSettings;
    async fn build(_: Arc<RwLock<TypeMap>>) -> (ModuleKind, Self) {
        (ModuleKind::Selfbot, Self {})
    }

    async fn is_match(&mut self, _: Self::SettingsType, msg: &Message) -> anyhow::Result<bool> {
        if let Some(embed) = msg.embeds.first() {
            // TODO: this isn't exactly a good method
            // the embed type is really just a loose nudge indicating what the embed might be, and it might be removed
            // in a future Discord API version. in any case, every embed not generated by Discord is "rich", this
            // includes embed's posted via the API (i.e. if a user's message has a rich embed in their message, they've
            // very likely posted it through the API which is selfbotting)
            Ok(embed.kind == RICH_EMBED)
        } else {
            Ok(false)
        }
    }
}
