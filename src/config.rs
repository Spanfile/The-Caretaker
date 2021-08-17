use crate::logging;
use serde::Deserialize;
use std::fmt::Display;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub discord_token: String,
    pub latency_update_freq_ms: u64,
    pub log_level: logging::LogLevel,
    pub log_timestamps: bool,
    pub log_colored: bool,

    #[serde(flatten)]
    pub database: DatabaseConfig,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct DatabaseConfig {
    pub db_endpoint: String,
    pub db_name: String,
    pub db_username: String,
    pub db_password: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            discord_token: Default::default(),
            // serenity seems to update a shard's latency every 40 seconds so round it up to a nice one minute
            latency_update_freq_ms: 60_000,
            log_level: Default::default(),
            log_timestamps: true,
            log_colored: true,
            database: Default::default(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Ok(envy::from_env::<Self>()?)
    }
}

impl DatabaseConfig {
    pub fn construct_database_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}/{}",
            self.db_username, self.db_password, self.db_endpoint, self.db_name
        )
    }
}

impl Display for DatabaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "postgres://{}:<redacted>@{}/{}",
            self.db_username, self.db_endpoint, self.db_password
        )
    }
}
