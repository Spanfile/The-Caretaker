mod crosspost;
mod mass_ping;

use crate::module::{cache::ModuleCache, Module, ModuleKind};
use crosspost::Crosspost;
use log::*;
use mass_ping::MassPing;
use serenity::{async_trait, model::channel::Message, prelude::TypeMap};
use std::{sync::Arc, time::Instant};
use tokio::sync::{
    broadcast::{self, RecvError},
    mpsc, RwLock,
};

pub type MatcherResponse = (ModuleKind, Arc<Message>);

#[async_trait]
trait Matcher {
    async fn build(userdata: Arc<RwLock<TypeMap>>) -> (ModuleKind, Self);
    async fn is_match(&mut self, module: Module, msg: &Message) -> anyhow::Result<bool>;
}

// because the macro `matchers` always copies the action_tx, the original given action_tx isn't consumed, just cloned a
// bunch of times and dropped at the end as its ownership ends. this is wanted behaviour, since this way the only
// instances of the action_tx are in the matcher tasks and there won't be any dangling ones
#[allow(clippy::clippy::needless_pass_by_value)]
pub fn spawn_message_matchers(
    msg_tx: broadcast::Sender<Arc<Message>>,
    action_tx: mpsc::Sender<MatcherResponse>,
    userdata: Arc<RwLock<TypeMap>>,
) {
    macro_rules! matchers {
        ($($matcher:ty),+) => {
            $(
                let rx = msg_tx.subscribe();
                let tx = action_tx.clone();
                let data = userdata.clone();
                tokio::spawn(async move {
                    run_matcher::<$matcher>(rx, tx, data).await;
                });
            )+
        };
    }

    matchers!(Crosspost, MassPing);
}

async fn run_matcher<M>(
    mut rx: broadcast::Receiver<Arc<Message>>,
    mut tx: mpsc::Sender<MatcherResponse>,
    userdata: Arc<RwLock<TypeMap>>,
) where
    M: Matcher,
{
    let (kind, mut matcher) = M::build(Arc::clone(&userdata)).await;

    loop {
        let msg = match rx.recv().await {
            Ok(m) => m,
            Err(RecvError::Closed) => {
                error!("{}: message channel closed", kind);
                return;
            }
            Err(RecvError::Lagged(skipped)) => {
                warn!("{}: message rx lagged (skipped {} messages)", kind, skipped);
                continue;
            }
        };

        let guild_id = match msg.guild_id {
            Some(id) => id,
            None => {
                warn!("{}: missing guild in message (is the message a DM?)", kind);
                continue;
            }
        };

        let module = userdata
            .read()
            .await
            .get::<ModuleCache>()
            .expect("missing ModuleCache in userdata")
            .get(guild_id, kind)
            .await;
        if !module.enabled() {
            debug!("{}: module disabled, not matching", kind);
            continue;
        }

        let start = Instant::now();
        let result = matcher.is_match(module, &msg).await;
        debug!("{}: returned match {:?} in {:?}", kind, result, start.elapsed());

        match result {
            Ok(true) => {
                debug!(
                    "{}: matched message {} in channel {} in guild {:?} by {}",
                    kind, msg.id, msg.channel_id, msg.guild_id, msg.author.id,
                );

                if tx.send((kind, msg)).await.is_err() {
                    error!("{}: action channel closed", kind);
                    return;
                }
            }
            Err(e) => {
                error!("{}: matching failed: {:?}", kind, e);
                continue;
            }
            _ => (),
        };
    }
}
