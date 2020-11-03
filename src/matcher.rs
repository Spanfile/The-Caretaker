mod crosspost;
mod mass_ping;

use crate::{
    error::InternalError,
    ext::Userdata,
    module::{
        cache::ModuleCache,
        settings::{ModuleSettings, Settings},
        ModuleKind,
    },
    DbPool,
};
use crosspost::Crosspost;
use log::*;
use mass_ping::MassPing;
use serenity::{async_trait, model::channel::Message, prelude::TypeMap};
use std::{convert::TryInto, sync::Arc, time::Instant};
use tokio::sync::{
    broadcast::{self, RecvError},
    mpsc, RwLock,
};

pub type MatcherResponse = (ModuleKind, Arc<Message>);

#[async_trait]
trait Matcher {
    type SettingsType: Settings;
    async fn build(userdata: Arc<RwLock<TypeMap>>) -> (ModuleKind, Self);
    async fn is_match(&mut self, settings: Self::SettingsType, msg: &Message) -> anyhow::Result<bool>;
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
            $(let rx = msg_tx.subscribe();
            let tx = action_tx.clone();
            let data = userdata.clone();
            tokio::spawn(async move {
                run_matcher::<$matcher>(rx, tx, data).await;
            });)+
        };
    }

    matchers!(Crosspost, MassPing);
}

async fn run_matcher<M>(
    rx: broadcast::Receiver<Arc<Message>>,
    tx: mpsc::Sender<MatcherResponse>,
    userdata: Arc<RwLock<TypeMap>>,
) where
    M: Matcher,
    ModuleSettings: TryInto<<M as Matcher>::SettingsType>,
    <ModuleSettings as TryInto<<M as Matcher>::SettingsType>>::Error: 'static + Send + Sync,
{
    let (kind, matcher) = M::build(Arc::clone(&userdata)).await;
    let runner = MatcherRunner {
        matcher,
        kind,
        rx,
        tx,
        userdata,
    };

    match runner.run().await {
        Ok(_) => debug!("{}: runner returned succesfully", kind),
        Err(e) => error!("{}: runner returned with error: {}", kind, e),
    }
}

struct MatcherRunner<M: Matcher> {
    matcher: M,
    kind: ModuleKind,
    rx: broadcast::Receiver<Arc<Message>>,
    tx: mpsc::Sender<MatcherResponse>,
    userdata: Arc<RwLock<TypeMap>>,
}

impl<M> MatcherRunner<M>
where
    M: Matcher,
    ModuleSettings: TryInto<<M as Matcher>::SettingsType>,
    <ModuleSettings as TryInto<<M as Matcher>::SettingsType>>::Error: 'static + Send + Sync,
{
    async fn run(mut self) -> anyhow::Result<()> {
        loop {
            let msg = match self.rx.recv().await {
                Ok(m) => m,
                Err(RecvError::Lagged(skipped)) => {
                    warn!("{}: message rx lagged (skipped {} messages)", self.kind, skipped);
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            let start = Instant::now();
            let result = self.is_match(&msg).await;
            debug!(
                "{}: returned match result {:?} in {:?}",
                self.kind,
                result,
                start.elapsed()
            );

            match result {
                Ok(true) => {
                    debug!(
                        "{}: matched message {} in channel {} in guild {:?} by {}",
                        self.kind, msg.id, msg.channel_id, msg.guild_id, msg.author.id,
                    );

                    self.tx.send((self.kind, msg)).await?;
                }
                Err(e) => {
                    error!("{}: matching failed: {:?}", self.kind, e);
                    continue;
                }
                _ => (),
            };
        }
    }

    async fn is_match(&mut self, msg: &Message) -> anyhow::Result<bool> {
        let guild_id = match msg.guild_id {
            Some(id) => id,
            None => {
                warn!("{}: missing guild in message (is the message a DM?)", self.kind);
                return Ok(false);
            }
        };

        let data = self.userdata.read().await;
        let module = data.get_userdata::<ModuleCache>()?.get(guild_id, self.kind).await;
        if !module.enabled() {
            debug!("{}: module disabled, not matching", self.kind);
            return Ok(false);
        }

        let settings = {
            let db = data.get_userdata::<DbPool>()?.get()?;
            // the author of enum_dispatch is an idiot so their TryInto impl returns a 'static &str as an error, which
            // is everything but (it doesn't impl Error)
            module.get_settings(&db)?.try_into().map_err(|_| {
                InternalError::ConversionFailed("tried to convert ModuleSettings variant to invalid type")
            })?
        };

        self.matcher.is_match(settings, &msg).await
    }
}
