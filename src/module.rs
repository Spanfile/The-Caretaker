use std::collections::HashMap;

use crate::{models, schema};
use diesel::{prelude::*, PgConnection};
use log::*;
use std::str::FromStr;
use strum::{Display, EnumIter, EnumString, EnumVariantNames, IntoEnumIterator};

#[derive(Debug, EnumString, EnumVariantNames, EnumIter, Display, Copy, Clone, Eq, PartialEq, Hash)]
#[strum(serialize_all = "kebab-case")]
pub enum Module {
    MassPing,
    Crosspost,
}

impl Module {
    pub fn get_all_modules_for_guild(guild: i64, db: &PgConnection) -> anyhow::Result<HashMap<Module, bool>> {
        use schema::enabled_modules;

        let mut modules = HashMap::new();
        for m in Module::iter() {
            modules.insert(m, false);
        }

        let enabled_modules = enabled_modules::table
            .filter(enabled_modules::guild.eq(guild).and(enabled_modules::enabled.eq(true)))
            .select(enabled_modules::module)
            .load::<String>(db)?;

        for enabled_module in &enabled_modules {
            modules.insert(Module::from_str(enabled_module)?, true);
        }

        Ok(modules)
    }

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
