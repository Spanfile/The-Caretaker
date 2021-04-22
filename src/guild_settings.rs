use crate::models;
use serenity::model::id::GuildId;

#[derive(Debug, Clone)]
pub struct GuildSettings {
    pub guild: GuildId,
    pub prefix: Option<String>,
}

impl From<models::GuildSettings> for GuildSettings {
    fn from(m: models::GuildSettings) -> Self {
        Self {
            guild: GuildId(m.guild as u64),
            prefix: m.prefix,
        }
    }
}

impl GuildSettings {
    pub fn default_with_guild(guild: GuildId) -> Self {
        Self { guild, prefix: None }
    }
}
