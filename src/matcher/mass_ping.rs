use super::Matcher;
use crate::module::ModuleKind;
use serenity::{async_trait, model::channel::Message};

pub struct MassPing {}

#[async_trait]
impl Matcher for MassPing {
    fn for_module_kind() -> ModuleKind {
        ModuleKind::MassPing
    }

    async fn is_match(msg: &Message) -> bool {
        // this catches both @everyone and @here
        msg.mention_everyone
    }
}
