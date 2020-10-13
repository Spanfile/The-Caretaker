mod logging;
mod management;

use std::collections::HashSet;

use log::*;
use management::Management;
use serde::Deserialize;
use serenity::{async_trait, http::Http, model::prelude::*, prelude::*, Client};

#[derive(Deserialize, Debug)]
struct Config {
    discord_token: String,
    log_level: logging::LogLevel,
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
        debug!("{:#?}", ready);
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

    let mgmt = Management::new();

    let mut client = Client::new(&config.discord_token)
        .token(&config.discord_token)
        .event_handler(Handler)
        .framework(mgmt)
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
