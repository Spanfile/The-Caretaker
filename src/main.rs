#![warn(clippy::if_not_else)]
#![warn(clippy::needless_pass_by_value)]
#![warn(clippy::non_ascii_literal)]
#![warn(clippy::panic_in_result_fn)]
#![warn(clippy::clippy::too_many_lines)]
#![warn(clippy::clippy::single_match_else)]

// ew what is this, rust 2015?
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod config;
mod error;
mod ext;
mod framework;
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

use config::Config;
use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool, PooledConnection},
};
use ext::UserdataExt;
use framework::CaretakerFramework;
use log::*;
use matcher::MatcherResponse;
use module::{action::Action, cache::ModuleCache};
use serenity::{
    async_trait, client::bridge::gateway::event::ShardStageUpdateEvent, gateway::ConnectionStage, http::Http,
    model::prelude::*, prelude::*, CacheAndHttp, Client,
};
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
                "Shard {}/{} ready! # of guilds: {}. Session ID: {}. Connected as {}",
                shard + 1,
                shards,
                ready.guilds.len(),
                ready.session_id,
                ready.user.tag()
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

    async fn cache_ready(&self, _ctx: Context, guilds: Vec<GuildId>) {
        debug!("Cache ready. # of guilds: {}", guilds.len());
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
    let config = Config::load()?;
    logging::setup_logging(&config)?;

    info!("Starting...");
    debug!("{:#?}", config);

    let db_pool = build_db_pool(&config.database_url)?;
    let module_cache = ModuleCache::populate_from_db(&db_pool.get()?)?;

    let (msg_tx, _) = broadcast::channel(64);
    let (action_tx, action_rx) = mpsc::channel(8);

    let mut client = create_discord_client(&config.discord_token, msg_tx.clone()).await?;
    populate_userdata(&client, module_cache, db_pool).await?;

    matcher::spawn_message_matchers(msg_tx, action_tx, client.data.clone());
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

fn build_db_pool(url: &str) -> anyhow::Result<Pool<ConnectionManager<PgConnection>>> {
    debug!("Establishing pooled database connection to {}...", url);

    let builder = Pool::builder();
    debug!("{:#?}", builder);
    Ok(builder.build(ConnectionManager::new(url))?)
}

async fn create_discord_client(token: &str, msg_tx: broadcast::Sender<Arc<Message>>) -> anyhow::Result<Client> {
    debug!("Initialising Discord client...");

    let http = Http::new_with_token(token);
    let appinfo = http.get_current_application_info().await?;

    debug!("{:#?}", appinfo);
    info!("Connected with application {}. Own ID: {}", appinfo.name, appinfo.id);

    let framework = CaretakerFramework::new(msg_tx);
    let client = Client::builder(token)
        .event_handler(Handler)
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
) -> anyhow::Result<()> {
    debug!("Populating userdata...");
    let mut data = client.data.write().await;

    data.insert::<ModuleCache>(module_cache);
    data.insert::<ShardMetadata>(HashMap::default());
    data.insert::<DbPool>(db_pool);
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

            let guild_id = msg.guild_id.expect("no guild ID in message");
            let module = module_cache.get(guild_id, kind).await;
            debug!(
                "Running actions for guild {} module {} message {}",
                guild_id, kind, msg.id
            );

            let db = db_pool.get().expect("failed to get db connection from pool");
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
