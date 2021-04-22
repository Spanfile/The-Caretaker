use crate::{framework::DEFAULT_COMMAND_PREFIX, models, DbConn};
use diesel::prelude::*;
use log::*;
use serenity::model::id::GuildId;

#[derive(Debug, Clone)]
pub struct GuildSettings {
    guild: GuildId,
    prefix: Option<String>,
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

    pub fn get_for_guild(guild: GuildId, db: &DbConn) -> anyhow::Result<Self> {
        use crate::schema::guild_settings;

        let settings = guild_settings::table
            .filter(guild_settings::guild.eq(guild.0 as i64))
            .first::<models::GuildSettings>(db)
            .optional()?
            .map_or_else(|| GuildSettings::default_with_guild(guild), GuildSettings::from);

        Ok(settings)
    }

    pub fn get_prefix_or_default(&self) -> String {
        self.prefix
            .clone()
            .unwrap_or_else(|| String::from(DEFAULT_COMMAND_PREFIX))
    }

    pub fn set_guild_prefix(&mut self, prefix: Option<String>, db: &DbConn) -> anyhow::Result<()> {
        self.prefix = prefix;
        self.update_db(db)
    }

    fn update_db(&self, db: &DbConn) -> anyhow::Result<()> {
        use crate::schema::guild_settings;

        let new_settings = models::NewGuildSettings {
            guild: self.guild.0 as i64,
            prefix: self.prefix.as_deref(),
        };

        // return the inserted row's guild ID but don't store it anywhere, because this way diesel will error if the
        // insert affected no rows
        diesel::insert_into(guild_settings::table)
            .values(&new_settings)
            .on_conflict(guild_settings::guild)
            .do_update()
            .set(&new_settings)
            .returning(guild_settings::guild)
            .get_result::<i64>(db)?;

        debug!("{:?}: insert {:?}", self, new_settings);
        Ok(())
    }
}
