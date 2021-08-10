use crate::{
    error::InternalError,
    ext::UserdataExt,
    latency_counter::LatencyCounter,
    matcher::MatcherResponse,
    module::{action::Action, cache::ModuleCache},
    DbPool,
};
use log::*;
use serenity::{model::channel::Message, CacheAndHttp, Client};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::mpsc, time};

pub fn spawn_shard_latency_ticker(client: &Client, update_freq: u64) {
    info!("Spawning shard latency update ticker...");

    let shard_manager = client.shard_manager.clone();
    let client_data = client.data.clone();
    tokio::spawn(async move {
        info!("Starting shard latency update loop");
        loop {
            time::sleep(Duration::from_millis(update_freq)).await;

            let manager = shard_manager.lock().await;
            let runners = manager.runners.lock().await;
            let mut data = client_data.write().await;

            let latency_counter = if let Some(counter) = data.get_mut::<LatencyCounter>() {
                counter
            } else {
                error!("No latency counter in client userdata");
                continue;
            };

            for (id, runner) in runners.iter() {
                info!("Shard {} status: {}, latency: {:?}", id, runner.stage, runner.latency);

                if let Some(latency) = runner.latency {
                    latency_counter.tick_gateway(latency).await;
                } else {
                    warn!("Missed latency update for shard {}", id);
                }
            }
        }
    });
}

pub fn spawn_termination_waiter(client: &Client) {
    info!("Spawning termination waiter...");

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("failed to listen for SIGTERM");
        info!("Caught SIGTERM, shutting down all shards");
        shard_manager.lock().await.shutdown_all().await;
    });
}

pub async fn spawn_action_handler(client: &Client, mut rx: mpsc::Receiver<MatcherResponse>) -> anyhow::Result<()> {
    info!("Spawning action handler...");

    let data = client.data.read().await;
    let module_cache = data.get_userdata::<ModuleCache>()?.clone();
    let db_pool = data.get_userdata::<DbPool>()?.clone();
    let latency = data.get_userdata::<LatencyCounter>()?.clone();
    let cache_http = Arc::clone(&client.cache_and_http);

    tokio::spawn(async move {
        info!("Starting action handler loop");
        loop {
            let (kind, msg) = if let Some(r) = rx.recv().await {
                r
            } else {
                error!("Matcher response channel closed");
                return;
            };

            let guild_id = match msg.guild_id.ok_or(InternalError::MissingGuildID) {
                Ok(id) => id,
                Err(e) => {
                    error!("{}", e);
                    continue;
                }
            };

            let module = module_cache.get(guild_id, kind).await;
            info!(
                "Running actions for guild {} module {} message {}",
                guild_id, kind, msg.id
            );

            let db = match db_pool.get() {
                Ok(db) => db,
                Err(e) => {
                    error!("Failed to get database connection from pool: {}", e);
                    continue;
                }
            };

            let actions = match module.get_actions(&db) {
                Ok(a) => a,
                Err(e) => {
                    error!("Failed to get module actions: {}", e);
                    continue;
                }
            };

            for action in actions {
                spawn_action_runner(action, Arc::clone(&cache_http), Arc::clone(&msg), latency.clone());
            }
        }
    });

    Ok(())
}

fn spawn_action_runner(
    action: Action<'static>,
    cache_http: Arc<CacheAndHttp>,
    msg: Arc<Message>,
    latency: LatencyCounter,
) {
    tokio::spawn(async move {
        let action_dbg_display = format!("{:?}", action);
        let start = Instant::now();
        if let Err(e) = action.run(&cache_http, &msg).await {
            error!(
                "Failed to run {} against guild {:?} channel {} message {}: {}",
                action_dbg_display, msg.guild_id, msg.channel_id, msg.id, e
            );
        }

        debug!(
            "Running {} against guild {:?} channel {} message {} took {:?}",
            action_dbg_display,
            msg.guild_id,
            msg.channel_id,
            msg.id,
            start.elapsed()
        );

        latency.tick_action(start.elapsed()).await;
    });
}
