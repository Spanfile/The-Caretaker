#![warn(clippy::if_not_else)]
#![warn(clippy::needless_pass_by_value)]
#![warn(clippy::non_ascii_literal)]
#![warn(clippy::panic_in_result_fn)]
#![warn(clippy::too_many_lines)]
#![warn(clippy::single_match_else)]
#![warn(clippy::unused_async)]
#![warn(clippy::unused_self)]

// ew what is this, rust 2015?
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod config;
mod error;
mod ext;
mod guild_settings;
mod handler;
mod latency_counter;
mod logging;
mod matcher;
mod models;
mod module;
mod schema;
mod tasks;
// separate the embedded migrations into their own module just so the panic_in_result_fn clippy lint can be allowed in
// the entire module
mod migrations {
    #![allow(clippy::panic_in_result_fn)]
    pub use embedded_migrations::*;
    embed_migrations!();
}

use chrono::{DateTime, Utc};
use config::{Config, DatabaseConfig};
use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool, PooledConnection},
};
use handler::Handler;
use latency_counter::LatencyCounter;
use log::*;
use module::cache::ModuleCache;
use serenity::{http::Http, model::prelude::*, prelude::*, Client};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, mpsc};

const VERSION: &str = env!("CARGO_PKG_VERSION");

type DbConn = PooledConnection<ConnectionManager<PgConnection>>;

#[derive(Debug)]
struct ShardMetadata {
    id: u64,
    guilds: usize,
    last_connected: DateTime<Utc>,
}

impl TypeMapKey for ShardMetadata {
    type Value = HashMap<u64, ShardMetadata>;
}

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

    info!("Starting Caretaker version {} at {}", VERSION, start_time);
    debug!("{:#?}", config);

    let db_pool = build_db_pool(&config.database)?;
    let module_cache = ModuleCache::populate_from_db(&db_pool.get()?)?;

    let (msg_tx, _) = broadcast::channel(64);
    let (action_tx, action_rx) = mpsc::channel(8);

    let mut client = create_discord_client(&config.discord_token, msg_tx.clone()).await?;
    populate_userdata(&client, module_cache, db_pool, start_time).await?;

    matcher::spawn_message_matchers(msg_tx, action_tx, client.data.clone());
    tasks::spawn_action_handler(&client, action_rx).await?;
    tasks::spawn_shard_latency_ticker(&client, config.latency_update_freq_ms);
    tasks::spawn_termination_waiter(&client);

    info!("Starting client...");
    match client.start_autosharded().await {
        Ok(_) => info!("Client shut down succesfully!"),
        Err(e) => error!("Client returned error: {}", e),
    }

    Ok(())
}

fn build_db_pool(config: &DatabaseConfig) -> anyhow::Result<Pool<ConnectionManager<PgConnection>>> {
    info!("Establishing pooled database connection to {}...", config);

    let builder = Pool::builder();
    debug!("{:#?}", builder);

    let url = config.construct_database_url();
    let pool = builder.build(ConnectionManager::new(url))?;
    info!(
        "Database connection established. Total connections: {}",
        pool.max_size()
    );
    Ok(pool)
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

    info!("Initialising handler and client...");
    let handler = Handler::new(msg_tx);
    let client = Client::builder(token)
        .event_handler(handler)
        .application_id(appinfo.id.0) // this ID is technically the bot user ID but it also works as the application ID
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
    info!("Populating userdata...");
    let mut data = client.data.write().await;

    data.insert::<ModuleCache>(module_cache);
    data.insert::<ShardMetadata>(HashMap::default());
    data.insert::<DbPool>(db_pool);
    data.insert::<BotUptime>(start_time);
    data.insert::<LatencyCounter>(LatencyCounter::new());

    Ok(())
}
