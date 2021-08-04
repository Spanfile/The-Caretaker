mod interaction;
mod message;

use crate::{ext::UserdataExt, latency_counter::LatencyCounter, ShardMetadata, VERSION};
use chrono::Utc;
use log::*;
use serenity::{
    async_trait,
    client::{bridge::gateway::event::ShardStageUpdateEvent, Context, EventHandler},
    gateway::ConnectionStage,
    model::prelude::*,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;

pub struct Handler {
    msg_tx: broadcast::Sender<Arc<Message>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        debug!("{:#?}", ready);
        if let Some(s) = ready.shard {
            let (shard, shards) = (s[0], s[1]);
            info!(
                "Shard {} / {} ready! # of guilds: {}. Session ID: {}. Connected as {}",
                shard + 1,
                shards,
                ready.guilds.len(),
                ready.session_id,
                ready.user.tag()
            );

            interaction::build_commands(&ctx).await;

            self.set_info_activity(&ctx, shard, shards).await;
            self.insert_shard_metadata(&ctx, shard, ready.guilds.len()).await;
        } else {
            error!("Session ready, but shard was None");
        }
    }

    async fn resume(&self, ctx: Context, _: ResumedEvent) {
        debug!("Shard {}: resumed", ctx.shard_id);
    }

    async fn shard_stage_update(&self, ctx: Context, update: ShardStageUpdateEvent) {
        info!(
            "Shard {}: transitioned from {} to {}",
            update.shard_id, update.old, update.new
        );

        if let (ConnectionStage::Resuming, ConnectionStage::Connected) = (update.old, update.new) {
            info!("Shard {}: reconnected, resetting last connected time", update.shard_id);
            self.reset_shard_last_connected(&ctx, update.shard_id.0).await;
        }
    }

    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        debug!("Shard {}: cache ready. # of guilds: {}", ctx.shard_id, guilds.len());
        trace!("{:?}", guilds);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let delay = (Utc::now() - msg.timestamp).num_milliseconds();
        debug!("{:?}", msg);
        debug!(
            "Message handler called {}ms later from message timestamp ({})",
            delay, msg.timestamp
        );

        message::process(msg, &self.msg_tx).await;

        let data = ctx.data.read().await;
        match data.get_userdata::<LatencyCounter>() {
            Ok(latency) => latency.tick_message(Duration::from_millis(delay as u64)).await,
            Err(e) => error!("Failed to tick message handler latency: {:?}", e),
        }
    }

    async fn interaction_create(&self, ctx: Context, interact: Interaction) {
        // TODO: calculate delay and latency like above
        interaction::process(ctx, interact).await;
    }
}

impl Handler {
    pub fn new(msg_tx: broadcast::Sender<Arc<Message>>) -> Self {
        Self { msg_tx }
    }

    async fn set_info_activity(&self, ctx: &Context, shard: u64, shards: u64) {
        ctx.set_activity(Activity::playing(&format!("[{}] [{}/{}]", VERSION, shard + 1, shards)))
            .await;
    }

    async fn insert_shard_metadata(&self, ctx: &Context, shard: u64, guilds: usize) {
        let mut data = ctx.data.write().await;
        if let Some(shard_meta) = data.get_mut::<ShardMetadata>() {
            shard_meta.insert(
                shard,
                ShardMetadata {
                    id: shard,
                    guilds,
                    last_connected: Utc::now(),
                },
            );
        } else {
            error!("No shard collection in context userdata");
        }
    }

    async fn reset_shard_last_connected(&self, ctx: &Context, shard: u64) {
        let mut data = ctx.data.write().await;
        if let Some(meta_collection) = data.get_mut::<ShardMetadata>() {
            if let Some(shard_meta) = meta_collection.get_mut(&shard) {
                shard_meta.last_connected = Utc::now();
            } else {
                error!("No shard metadata for shard {}", shard);
            }
        } else {
            error!("No shard collection in context userdata");
        }
    }
}
