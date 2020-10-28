mod mass_ping;

use crate::module::ModuleKind;
use log::*;
use serenity::{async_trait, model::channel::Message};
use std::{sync::Arc, time::Instant};
use tokio::sync::{
    broadcast::{self, RecvError},
    mpsc,
};

pub type MatcherResponse = (ModuleKind, Arc<Message>);

#[async_trait]
trait Matcher {
    fn build() -> (ModuleKind, Self);
    async fn is_match(&self, msg: &Message) -> bool;
}

pub fn spawn_message_matchers(msg_tx: &broadcast::Sender<Arc<Message>>, action_tx: mpsc::Sender<MatcherResponse>) {
    let rx = msg_tx.subscribe();
    let tx = action_tx;
    tokio::spawn(async move {
        run_matcher::<mass_ping::MassPing>(rx, tx).await;
    });
}

async fn run_matcher<M>(mut rx: broadcast::Receiver<Arc<Message>>, mut tx: mpsc::Sender<MatcherResponse>)
where
    M: Matcher,
{
    let (module, matcher) = M::build();

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

        let start = Instant::now();
        if matcher.is_match(&msg).await {
            debug!(
                "{}: matched '{}' by {} in {:?}",
                module,
                msg.content,
                msg.author.id,
                start.elapsed()
            );

            if tx.send((module, msg)).await.is_err() {
                error!("{}: action channel closed", module);
                return;
            }
        }
    }
}
