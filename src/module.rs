pub mod action;

use crate::{error::CaretakerError, models, schema};
use action::Action;
use diesel::{prelude::*, PgConnection};
use log::*;
use serenity::model::id::ChannelId;
use std::{collections::HashMap, str::FromStr};
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

    pub fn get_actions_for_guild(&self, guild: i64, db: &PgConnection) -> anyhow::Result<Vec<Action>> {
        use schema::actions;

        let mut actions = Vec::new();
        for action_model in actions::table
            .filter(actions::guild.eq(guild).and(actions::module.eq(self.to_string())))
            .load::<models::Action>(db)?
        {
            // strum's from_str impl returns the proper variant, but with all fields set to their default values (where
            // could it get values for 'em anyways?)
            match Action::from_str(&action_model.action)? {
                Action::RemoveMessage => actions.push(Action::RemoveMessage),
                Action::NotifyUser { .. } => actions.push(Action::NotifyUser {
                    message: action_model.message.ok_or(CaretakerError::MissingField("message"))?,
                }),
                Action::NotifyIn { .. } => actions.push(Action::NotifyIn {
                    channel: ChannelId(
                        action_model
                            .in_channel
                            .ok_or(CaretakerError::MissingField("in_channel"))? as u64,
                    ),
                    message: action_model.message.ok_or(CaretakerError::MissingField("message"))?,
                }),
            }
        }

        Ok(actions)
    }
}
