use std::sync::Arc;

use super::Matcher;
use crate::module::{Module, ModuleKind};
use serenity::{async_trait, model::channel::Message, prelude::TypeMap};
use tokio::sync::RwLock;

pub struct MassPing {}

#[async_trait]
impl Matcher for MassPing {
    async fn build(_: Arc<RwLock<TypeMap>>) -> (ModuleKind, Self) {
        (ModuleKind::MassPing, Self {})
    }

    async fn is_match(&mut self, _: Module, msg: &Message) -> anyhow::Result<bool> {
        // this catches both @everyone and @here
        Ok(msg.mention_everyone)
    }
}
