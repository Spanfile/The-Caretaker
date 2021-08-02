#![warn(clippy::if_not_else)]
#![warn(clippy::needless_pass_by_value)]
#![warn(clippy::non_ascii_literal)]
#![warn(clippy::panic_in_result_fn)]
#![warn(clippy::too_many_lines)]
#![warn(clippy::single_match_else)]

// ew what is this, rust 2015?
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod config;
mod error;
mod ext;
mod framework;
mod guild_settings;
mod handler;
mod logging;
mod matcher;
mod models;
mod module;
mod schema;
// separate the embedded migrations into their own module just to the panic_in_result_fn clippy lint can be allowed in
// the entire module
mod migrations {
    #![allow(clippy::panic_in_result_fn)]
    pub use embedded_migrations::*;
    embed_migrations!();
}

use chrono::{DateTime, Utc};
use config::Config;
use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool, PooledConnection},
};
use error::InternalError;
use ext::UserdataExt;
use framework::CaretakerFramework;
use log::*;
use matcher::MatcherResponse;
use module::{action::Action, cache::ModuleCache};
use serenity::{http::Http, model::prelude::*, prelude::*, CacheAndHttp, Client};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{broadcast, mpsc},
    time,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
struct ShardMetadata {
    id: u64,
    guilds: usize,
    latency: Option<Duration>,
    last_connected: Instant,
}

impl TypeMapKey for ShardMetadata {
    type Value = HashMap<u64, ShardMetadata>;
}

type DbConn = PooledConnection<ConnectionManager<PgConnection>>;
struct DbPool {}
impl TypeMapKey for DbPool {
    type Value = Pool<ConnectionManager<PgConnection>>;
}

struct BotUptime {}
impl TypeMapKey for BotUptime {
    type Value = DateTime<Utc>;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let start_time = Utc::now();

    dotenv::dotenv()?;
    let config = Config::load()?;
    logging::setup_logging(&config)?;

    info!("Starting Caretaker version {}", VERSION);
    debug!("{:#?}", config);

    let db_pool = build_db_pool(&config.database_url)?;
    info!(
        "Database connection established. Total connections: {}",
        db_pool.max_size()
    );

    let module_cache = ModuleCache::populate_from_db(&db_pool.get()?)?;

    let (msg_tx, _) = broadcast::channel(64);
    let (action_tx, action_rx) = mpsc::channel(8);

    let mut client = create_discord_client(&config.discord_token, msg_tx.clone()).await?;
    populate_userdata(&client, module_cache, db_pool, start_time).await?;

    matcher::spawn_message_matchers(msg_tx, action_tx, client.data.clone());
    spawn_action_handler(&client, action_rx).await?;
    spawn_shard_latency_ticker(&client, config.latency_update_freq_ms);
    spawn_termination_waiter(&client);

    info!("Starting client...");
    match client.start_autosharded().await {
        Ok(_) => info!("Client shut down succesfully!"),
        Err(e) => error!("Client returned error: {}", e),
    }

    Ok(())
}

fn build_db_pool(url: &str) -> anyhow::Result<Pool<ConnectionManager<PgConnection>>> {
    debug!("Establishing pooled database connection to {}...", url);

    let builder = Pool::builder();
    debug!("{:#?}", builder);
    Ok(builder.build(ConnectionManager::new(url))?)
}

async fn create_discord_client(token: &str, msg_tx: broadcast::Sender<Arc<Message>>) -> anyhow::Result<Client> {
    info!("Initialising Discord client...");

    let http = Http::new_with_token(token);
    let appinfo = http.get_current_application_info().await?;

    debug!("{:#?}", appinfo);
    info!(
        "Connected with application {} ({}). Owned by {} ({})",
        appinfo.name,
        appinfo.id,
        appinfo.owner.tag(),
        appinfo.owner.id
    );

    let framework = CaretakerFramework::new(msg_tx);
    let client = Client::builder(token)
        .event_handler(handler::Handler)
        .framework(framework)
        // specifying a stricter set of intents than literally all of them seems to disallow serenity's cache from
        // functioning, which in turn breaks a lot of other things
        //.intents(GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES)
        .await?;
    Ok(client)
}

async fn populate_userdata(
    client: &Client,
    module_cache: ModuleCache,
    db_pool: Pool<ConnectionManager<PgConnection>>,
    start_time: DateTime<Utc>,
) -> anyhow::Result<()> {
    debug!("Populating userdata...");
    let mut data = client.data.write().await;

    data.insert::<ModuleCache>(module_cache);
    data.insert::<ShardMetadata>(HashMap::default());
    data.insert::<DbPool>(db_pool);
    data.insert::<BotUptime>(start_time);

    Ok(())
}

fn spawn_shard_latency_ticker(client: &Client, update_freq: u64) {
    debug!("Spawning shard latency update ticker...");

    let shard_manager = client.shard_manager.clone();
    let client_data = client.data.clone();
    tokio::spawn(async move {
        debug!("Starting shard latency update loop");
        loop {
            time::sleep(Duration::from_millis(update_freq)).await;

            let manager = shard_manager.lock().await;
            let runners = manager.runners.lock().await;
            let mut data = client_data.write().await;
            let shard_meta_collection = if let Some(sm) = data.get_mut::<ShardMetadata>() {
                sm
            } else {
                error!("No shard collection in client userdata");
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
                    error!("No metadata object for shard {} found", id);
                }
            }
        }
    });
}

fn spawn_termination_waiter(client: &Client) {
    debug!("Spawning termination waiter...");

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("failed to listen for SIGTERM");
        info!("Caught SIGTERM, shutting down all shards");
        shard_manager.lock().await.shutdown_all().await;
    });
}

async fn spawn_action_handler(client: &Client, mut rx: mpsc::Receiver<MatcherResponse>) -> anyhow::Result<()> {
    debug!("Spawning action handler...");

    let data = client.data.read().await;
    let module_cache = data.get_userdata::<ModuleCache>()?.clone();
    let db_pool = data.get_userdata::<DbPool>()?.clone();
    let cache_http = Arc::clone(&client.cache_and_http);

    tokio::spawn(async move {
        debug!("Starting action handler loop");
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
            debug!(
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
                spawn_action_runner(action, Arc::clone(&cache_http), Arc::clone(&msg));
            }
        }
    });

    Ok(())
}

fn spawn_action_runner(action: Action<'static>, cache_http: Arc<CacheAndHttp>, msg: Arc<Message>) {
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
    });
}
