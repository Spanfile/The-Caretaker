use super::Matcher;
use crate::module::ModuleKind;
use serenity::{async_trait, model::channel::Message};

pub struct MassPing {}

#[async_trait]
impl Matcher for MassPing {
    fn build() -> (ModuleKind, Self) {
        (ModuleKind::MassPing, Self {})
    }

    async fn is_match(&mut self, msg: &Message) -> bool {
        // this catches both @everyone and @here
        msg.mention_everyone
    }
}
