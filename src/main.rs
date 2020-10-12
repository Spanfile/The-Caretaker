mod commands;
mod logging;

use std::collections::HashSet;

use commands::management::*;
use log::*;
use serde::Deserialize;
use serenity::{
    async_trait,
    framework::{standard::macros::group, StandardFramework},
    http::Http,
    model::prelude::*,
    prelude::*,
    Client,
};

const COMMAND_PREFIX: &str = "-ct ";

#[derive(Deserialize, Debug)]
struct Config {
    discord_token: String,
    log_level: logging::LogLevel,
}

#[group]
#[commands(ping)]
struct Management;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let config = envy::from_env::<Config>()?;
    logging::setup_logging(config.log_level)?;

    info!("Starting...");
    debug!("{:#?}", config);

    let http = Http::new_with_token(&config.discord_token);
    let (owners, _bot_id) = http.get_current_application_info().await.map(|info| {
        let mut owners = HashSet::new();
        owners.insert(info.owner.id);

        (owners, info.id)
    })?;

    let framework = StandardFramework::new()
        .configure(|c| c.owners(owners).prefix(COMMAND_PREFIX))
        .group(&MANAGEMENT_GROUP);

    let mut client = Client::new(&config.discord_token)
        .token(&config.discord_token)
        .framework(framework)
        .event_handler(Handler)
        .await?;

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
