use crate::logging;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub discord_token: String,
    pub latency_update_freq_ms: u64,
    pub database_url: String,
    pub log_level: logging::LogLevel,
    pub log_timestamps: bool,
    pub log_colored: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            discord_token: String::default(),
            log_level: logging::LogLevel::default(),
            database_url: String::default(),
            // serenity seems to update a shard's latency every 40 seconds so round it up to a nice one minute
            latency_update_freq_ms: 60_000,
            log_timestamps: true,
            log_colored: true,
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Ok(envy::from_env::<Self>()?)
    }
}
