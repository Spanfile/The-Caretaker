use crate::{models, schema};
use diesel::{prelude::*, PgConnection};
use log::*;
use strum::{EnumString, EnumVariantNames, ToString};

#[derive(Debug, EnumString, EnumVariantNames, ToString, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum Module {
    MassPing,
    Crosspost,
}

impl Module {
    pub fn set_enabled_for_guild(&self, guild: i64, enabled: bool, db: &PgConnection) -> anyhow::Result<()> {
        use models::ModuleEnableStatus;
        use schema::enabled_modules;

        let module_enabled = ModuleEnableStatus {
            guild,
            enabled,
            module: self.to_string(),
        };

        let rows = diesel::insert_into(enabled_modules::table)
            .values(&module_enabled)
            .on_conflict((enabled_modules::guild, enabled_modules::module))
            .do_update()
            .set(enabled_modules::enabled.eq(enabled))
            .execute(db)?;
        debug!("{:?} -> {} rows", module_enabled, rows);
        Ok(())
    }

    pub fn get_enabled_for_guild(&self, guild: i64, db: &PgConnection) -> anyhow::Result<bool> {
        use schema::enabled_modules;

        let enable_status = enabled_modules::table
            .filter(
                enabled_modules::guild
                    .eq(guild)
                    .and(enabled_modules::module.eq(self.to_string())),
            )
            .select(enabled_modules::enabled)
            .first(db)
            .optional()?;

        Ok(enable_status.unwrap_or(false))
    }
}
