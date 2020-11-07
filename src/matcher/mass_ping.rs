use super::Matcher;
use crate::module::{settings::MassPingSettings, ModuleKind};
use serenity::{async_trait, model::channel::Message, prelude::TypeMap};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MassPing {}

#[async_trait]
impl Matcher for MassPing {
    type SettingsType = MassPingSettings;
    async fn build(_: Arc<RwLock<TypeMap>>) -> (ModuleKind, Self) {
        (ModuleKind::MassPing, Self {})
    }

    async fn is_match(&mut self, _: Self::SettingsType, msg: &Message) -> anyhow::Result<bool> {
        // this catches both @everyone and @here
        Ok(msg.mention_everyone)
    }
}
