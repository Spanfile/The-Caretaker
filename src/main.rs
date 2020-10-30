#![warn(clippy::if_not_else)]
#![warn(clippy::needless_pass_by_value)]
#![warn(clippy::non_ascii_literal)]
#![warn(clippy::panic_in_result_fn)]

// ew what is this, rust 2015?
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod error;
mod ext;
mod framework;
mod logging;
mod matcher;
mod models;
mod module;
mod schema;
mod migrations {
    #![allow(clippy::panic_in_result_fn)]
    pub use embedded_migrations::*;
    embed_migrations!();
}

use diesel::{pg::PgConnection, prelude::*};
use error::InternalError;
use framework::CaretakerFramework;
use log::*;
use matcher::MatcherResponse;
use module::{action::Action, cache::ModuleCache};
use serde::Deserialize;
use serenity::{
    async_trait, client::bridge::gateway::event::ShardStageUpdateEvent, gateway::ConnectionStage, http::Http,
    model::prelude::*, prelude::*, CacheAndHttp, Client,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{broadcast, mpsc},
    time,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
            discord_token: String::default(),
            log_level: logging::LogLevel::default(),
            database_url: String::default(),
            // serenity seems to update a shard's latency every 40 seconds so round it up to a nice one minute
            latency_update_freq_ms: 60_000,
        }
    }
}

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

struct DbConnection {}
impl TypeMapKey for DbConnection {
    type Value = Arc<Mutex<PgConnection>>;
}

struct BotUptime {}
impl TypeMapKey for BotUptime {
    type Value = Instant;
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

            self.set_info_activity(&ctx, shard, shards).await;
            self.insert_shard_metadata(&ctx, shard, ready.guilds.len()).await;
        } else {
            error!("Session ready, but shard was None");
        }
    }

    // the ResumedEvent contains no useful information, which is to say it contains no information
    // async fn resume(&self, _: Context, resume: ResumedEvent) {
    //     info!("Resumed");
    //     debug!("{:#?}", resume);
    // }

    async fn shard_stage_update(&self, ctx: Context, update: ShardStageUpdateEvent) {
        info!(
            "Shard {} transitioned from {} to {}",
            update.shard_id, update.old, update.new
        );

        if let (ConnectionStage::Resuming, ConnectionStage::Connected) = (update.old, update.new) {
            info!("Shard {} reconnected, resetting last connected time", update.shard_id);
            self.reset_shard_last_connected(&ctx, update.shard_id.0).await;
        }
    }

    async fn cache_ready(&self, _: Context, guilds: Vec<GuildId>) {
        info!("Cache ready. {} guilds", guilds.len());
        debug!("{:?}", guilds);
    }
}

impl Handler {
    async fn set_info_activity(&self, ctx: &Context, shard: u64, shards: u64) {
        ctx.set_activity(Activity::playing(&format!(
            "{} [{}] [{}/{}]",
            framework::COMMAND_PREFIX,
            VERSION,
            shard + 1,
            shards
        )))
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
                    latency: None,
                    last_connected: Instant::now(),
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
                shard_meta.last_connected = Instant::now();
            } else {
                error!("No shard metadata for shard {}", shard);
            }
        } else {
            error!("No shard collection in context userdata");
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

    let (msg_tx, _) = broadcast::channel(64);
    let (action_tx, action_rx) = mpsc::channel(8);
    matcher::spawn_message_matchers(&msg_tx, action_tx);

    let mut client = create_discord_client(&config.discord_token, msg_tx).await?;
    let db_conn = establish_database_connection(&config.database_url)?;

    populate_userdata(&client, db_conn).await?;
    spawn_action_handler(&client, action_rx).await?;
    spawn_shard_latency_ticker(&client, config.latency_update_freq_ms);
    spawn_termination_waiter(&client);

    debug!("Starting autosharded client...");
    match client.start_autosharded().await {
        Ok(_) => info!("Client shut down succesfully!"),
        Err(e) => error!("Client returned error: {}", e),
    }

    Ok(())
}

fn establish_database_connection(url: &str) -> anyhow::Result<PgConnection> {
    debug!("Establishing database connection to {}...", url);

    let db_conn = PgConnection::establish(url)?;
    migrations::run(&db_conn)?;
    Ok(db_conn)
}

async fn create_discord_client(token: &str, msg_tx: broadcast::Sender<Arc<Message>>) -> anyhow::Result<Client> {
    debug!("Initialising Discord client...");

    let http = Http::new_with_token(token);
    let (owners, bot_id) = http.get_current_application_info().await.map(|info| {
        let mut owners = HashSet::new();
        owners.insert(info.owner.id);

        (owners, info.id)
    })?;

    debug!("Own ID: {}", bot_id);
    debug!("Owners: {:#?}", owners);

    let framework = CaretakerFramework::new(msg_tx);
    let client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .await?;
    Ok(client)
}

async fn populate_userdata(client: &Client, db: PgConnection) -> anyhow::Result<()> {
    debug!("Populating userdata...");
    let mut data = client.data.write().await;

    data.insert::<ModuleCache>(ModuleCache::populate_from_db(&db)?);
    data.insert::<ShardMetadata>(HashMap::default());
    data.insert::<DbConnection>(Arc::new(Mutex::new(db)));
    data.insert::<BotUptime>(Instant::now());

    Ok(())
}

fn spawn_shard_latency_ticker(client: &Client, update_freq: u64) {
    debug!("Spawning shard latency update ticker...");

    let shard_manager = client.shard_manager.clone();
    let client_data = client.data.clone();
    tokio::spawn(async move {
        debug!("Starting shard latency update loop");
        loop {
            time::delay_for(Duration::from_millis(update_freq)).await;

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
        tokio::signal::ctrl_c().await.expect("failed to listen for SIGINT");
        info!("Caught SIGINT, shutting down all shards");
        shard_manager.lock().await.shutdown_all().await;
    });
}

async fn spawn_action_handler(client: &Client, mut rx: mpsc::Receiver<MatcherResponse>) -> anyhow::Result<()> {
    debug!("Spawning action handler...");

    let data = client.data.read().await;
    let module_cache = data
        .get::<ModuleCache>()
        .ok_or(InternalError::MissingUserdata("ModuleCache"))?
        .clone();
    let db_arc = Arc::clone(
        data.get::<DbConnection>()
            .ok_or(InternalError::MissingUserdata("DbConnection"))?,
    );
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

            let guild_id = if let Some(id) = msg.guild_id {
                id
            } else {
                warn!("Missing guild in action handler message (is the message a DM?)");
                continue;
            };

            let module = module_cache.get(guild_id, kind).await;
            if module.enabled() {
                debug!(
                    "Running actions for guild {} module {} message {}",
                    guild_id, kind, msg.id
                );

                let db = db_arc.lock().await;
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
        }
    });

    Ok(())
}

fn spawn_action_runner(action: Action<'static>, cache_http: Arc<CacheAndHttp>, msg: Arc<Message>) {
    tokio::spawn(async move {
        let start = Instant::now();
        if let Err(e) = action.run(&cache_http, &msg).await {
            error!(
                "Failed to run {:?} against guild {:?} channel {} message {}: {}",
                action, msg.guild_id, msg.channel_id, msg.id, e
            );
        }

        debug!(
            "Running {:?} against guild {:?} channel {} message {} took {:?}",
            action,
            msg.guild_id,
            msg.channel_id,
            msg.id,
            start.elapsed()
        );
    });
}
