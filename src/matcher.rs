mod mass_ping;

use crate::module::ModuleKind;
use log::*;
use serenity::{
    async_trait,
    model::{
        channel::Message,
        id::{GuildId, MessageId},
    },
};
use tokio::sync::{
    broadcast::{self, RecvError},
    mpsc,
};

pub type MatcherResponse = (ModuleKind, GuildId, MessageId);

#[async_trait]
trait Matcher {
    fn for_module_kind() -> ModuleKind;
    async fn is_match(msg: &Message) -> bool;
}

pub fn spawn_message_matchers(msg_tx: &broadcast::Sender<Message>, action_tx: mpsc::Sender<MatcherResponse>) {
    let rx = msg_tx.subscribe();
    let tx = action_tx;
    tokio::spawn(async move {
        run_matcher::<mass_ping::MassPing>(rx, tx).await;
    });
}

async fn run_matcher<M>(mut rx: broadcast::Receiver<Message>, mut tx: mpsc::Sender<MatcherResponse>)
where
    M: Matcher,
{
    let module = M::for_module_kind();

    loop {
        let msg = match rx.recv().await {
            Ok(m) => m,
            Err(RecvError::Closed) => {
                error!("{}: message channel closed", module);
                return;
            }
            Err(RecvError::Lagged(skipped)) => {
                warn!("{}: message rx lagged (skipped {} messages)", module, skipped);
                continue;
            }
        };

        let guild_id = match msg.guild_id {
            Some(id) => id,
            None => {
                warn!("{}: received msg without guild ID", module);
                continue;
            }
        };

        if M::is_match(&msg).await {
            debug!("{}: matched '{}' by {}", module, msg.content, msg.author.id);
            if tx.send((module, guild_id, msg.id)).await.is_err() {
                error!("{}: action channel closed", module);
                return;
            }
        }
    }
}
