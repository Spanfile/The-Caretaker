// ew what is this, rust 2015?
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod error;
mod logging;
mod management;
mod models;
mod module;
mod schema;

use diesel::{pg::PgConnection, prelude::*};
use log::*;
use management::Management;
use serde::Deserialize;
use serenity::{
    async_trait, client::bridge::gateway::event::ShardStageUpdateEvent, http::Http, model::prelude::*, prelude::*,
    Client,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::time;

const VERSION: &str = env!("CARGO_PKG_VERSION");

embed_migrations!();

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Config {
    discord_token: String,
    log_level: logging::LogLevel,
    latency_update_freq_ms: u64,
    database_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            discord_token: Default::default(),
            log_level: Default::default(),
            database_url: Default::default(),
            // serenity seems to update a shard's latency every 40 seconds so round it up to a nice one minute
            latency_update_freq_ms: 60_000,
        }
    }
}

#[derive(Debug, Default)]
struct ShardMetadata {
    id: u64,
    guilds: usize,
    latency: Option<Duration>,
}

impl TypeMapKey for ShardMetadata {
    type Value = HashMap<u64, ShardMetadata>;
}

struct DbConnection {}
impl TypeMapKey for DbConnection {
    type Value = Arc<Mutex<PgConnection>>;
}

struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        debug!("{:#?}", ready);
        if let Some(s) = ready.shard {
            let (shard, shards) = (s[0], s[1]);
            info!(
                "Shard {}/{} ready! # of guilds: {}. Session ID: {}",
                shard + 1,
                shards,
                ready.guilds.len(),
                ready.session_id,
            );

            Handler::set_info_activity(&ctx, shard, shards).await;
            Handler::insert_shard_metadata(&ctx, shard, ready.guilds.len()).await;
        } else {
            warn!("Session ready, but shard was None");
        }
    }

    async fn resume(&self, _: Context, resume: ResumedEvent) {
        info!("Resumed");
        debug!("{:#?}", resume);
    }

    async fn shard_stage_update(&self, _: Context, update: ShardStageUpdateEvent) {
        info!(
            "Shard {} transitioned from {} to {}",
            update.shard_id, update.old, update.new
        );
    }

    async fn cache_ready(&self, _: Context, guilds: Vec<GuildId>) {
        info!("Cache ready. {} guilds", guilds.len());
        debug!("{:?}", guilds);
    }
}

impl Handler {
    async fn set_info_activity(ctx: &Context, shard: u64, shards: u64) {
        ctx.set_activity(Activity::playing(&format!(
            "{} [{}] [{}/{}]",
            management::COMMAND_PREFIX,
            VERSION,
            shard + 1,
            shards
        )))
        .await;
    }

    async fn insert_shard_metadata(ctx: &Context, shard: u64, guilds: usize) {
        let mut data = ctx.data.write().await;
        if let Some(shard_meta) = data.get_mut::<ShardMetadata>() {
            shard_meta.insert(
                shard,
                ShardMetadata {
                    id: shard,
                    guilds,
                    latency: None,
                },
            );
        } else {
            warn!("No shard collection in context userdata");
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let config = envy::from_env::<Config>()?;
    logging::setup_logging(config.log_level)?;

    info!("Starting...");
    debug!("{:#?}", config);

    debug!("Establishing database connection to {}...", config.database_url);
    let db_conn = PgConnection::establish(&config.database_url)?;
    embedded_migrations::run(&db_conn)?;

    debug!("Initialising Discord client...");
    let http = Http::new_with_token(&config.discord_token);
    let (owners, bot_id) = http.get_current_application_info().await.map(|info| {
        let mut owners = HashSet::new();
        owners.insert(info.owner.id);

        (owners, info.id)
    })?;

    debug!("Own ID: {}", bot_id);
    debug!("Owners: {:#?}", owners);

    let mgmt = Management::new();
    let mut client = Client::builder(&config.discord_token)
        .token(&config.discord_token)
        .event_handler(Handler)
        .framework(mgmt)
        .await?;

    {
        let mut data = client.data.write().await;
        data.insert::<ShardMetadata>(Default::default());
        data.insert::<DbConnection>(Arc::new(Mutex::new(db_conn)));
    }

    let shard_manager = client.shard_manager.clone();
    let client_data = client.data.clone();
    tokio::spawn(async move {
        debug!("Starting shard latency update ticker");
        loop {
            time::delay_for(Duration::from_millis(config.latency_update_freq_ms)).await;

            let manager = shard_manager.lock().await;
            let runners = manager.runners.lock().await;
            let mut data = client_data.write().await;
            let shard_meta_collection = if let Some(sm) = data.get_mut::<ShardMetadata>() {
                sm
            } else {
                warn!("No shard collection in client userdata");
                continue;
            };

            for (id, runner) in runners.iter() {
                debug!("Shard {} status: {}, latency: {:?}", id, runner.stage, runner.latency);

                if let Some(meta) = shard_meta_collection.get_mut(&id.0) {
                    match (meta.latency, runner.latency) {
                        (_, Some(latency)) => meta.latency = Some(latency),
                        (Some(prev), None) => {
                            warn!("Missing latency update for shard {} (previous latency {:?})", id, prev)
                        }
                        (None, None) => warn!("Missing first latency update for shard {}", id),
                    }
                } else {
                    warn!("No metadata object for shard {} found", id);
                }
            }
        }
    });

    debug!("Starting autosharded...");
    tokio::select! {
        res = client.start_autosharded() => {
            info!("Client returned");
            debug!("{:#?}", res);
            res?;
        }
        _ = term_signal() => {
            info!("Caught SIGINT, shutting down all shards");
            client.shard_manager.lock().await.shutdown_all().await;
        }
    };
    Ok(())
}

async fn term_signal() {
    tokio::signal::ctrl_c().await.expect("failed to listen for SIGINT");
}
