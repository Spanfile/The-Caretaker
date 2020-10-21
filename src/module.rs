pub mod action;

use self::action::ActionKind;
use crate::{error::CaretakerError, models, schema};
use action::Action;
use diesel::{prelude::*, PgConnection};
use log::*;
use serenity::model::id::ChannelId;
use std::{borrow::Cow, collections::HashMap, str::FromStr};
use strum::{Display, EnumIter, EnumString, EnumVariantNames, IntoEnumIterator};

#[derive(Debug, EnumString, EnumVariantNames, EnumIter, Display, Copy, Clone, Eq, PartialEq, Hash)]
#[strum(serialize_all = "kebab-case")]
pub enum Module {
    MassPing,
    Crosspost,
    DynamicSlowmode,
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
            match ActionKind::from_str(&action_model.action)? {
                ActionKind::RemoveMessage => actions.push(Action::remove_message()),
                ActionKind::Notify { .. } => actions.push(Action::notify(
                    action_model.in_channel.map(|c| ChannelId(c as u64)),
                    Cow::Owned(action_model.message.ok_or(CaretakerError::MissingField("message"))?),
                )),
            }
        }

        Ok(actions)
    }

    pub fn add_action_for_guild(&self, guild: i64, action: Action, db: &PgConnection) -> anyhow::Result<i32> {
        use schema::actions;

        let action_str = action.kind.to_string();
        let module_str = self.to_string();

        let action_model = match action.kind {
            ActionKind::RemoveMessage => models::NewAction {
                guild,
                action: &action_str,
                module: &module_str,
                in_channel: None,
                message: None,
            },
            ActionKind::Notify => models::NewAction {
                guild,
                action: &action_str,
                module: &module_str,
                in_channel: action.channel.map(|c| c.0 as i64),
                message: action.message.as_deref(),
            },
        };

        let id: i32 = diesel::insert_into(actions::table)
            .values(&action_model)
            .returning(actions::id)
            .get_result(db)?;
        debug!("{:?} -> ID {}", action_model, id);
        Ok(id)
    }

    pub fn remove_nth_action_for_guild(&self, guild: i64, n: usize, db: &PgConnection) -> anyhow::Result<()> {
        use schema::actions;

        let actions = actions::table
            .filter(actions::guild.eq(guild).and(actions::module.eq(self.to_string())))
            .load::<models::Action>(db)?;
        let delete = actions.get(n).ok_or(CaretakerError::IndexOutOfRange(n))?;

        // return the deleted row's ID but don't store it anywhere, because this way diesel will error if the delete
        // affected no rows
        diesel::delete(actions::table.filter(actions::id.eq(delete.id)))
            .returning(actions::id)
            .get_result::<i32>(db)?;
        debug!("Delete {:?}", delete);
        Ok(())
    }
}
