pub mod action;
pub mod dbimport {
    pub use super::{action::Action_kind, Module_kind};
}

use self::action::{Action, ActionKind};
use crate::{
    error::{ArgumentError, InternalError},
    models, schema,
};
use diesel::{prelude::*, PgConnection};
use diesel_derive_enum::DbEnum;
use log::*;
use serenity::model::id::{ChannelId, GuildId};
use std::{borrow::Cow, collections::HashMap};
use strum::{Display, EnumIter, EnumString, EnumVariantNames, IntoEnumIterator};

// the database schema holds its own version of this enum, remember to modify it as well if modying this one
#[derive(Debug, EnumString, EnumVariantNames, EnumIter, Display, Copy, Clone, Eq, PartialEq, Hash, DbEnum)]
#[strum(serialize_all = "kebab-case")]
#[DieselType = "Module_kind"]
pub enum ModuleKind {
    MassPing,
    Crosspost,
    DynamicSlowmode,
    EmojiSpam,
    MentionSpam,
    Selfbot,
}

#[derive(Debug, Copy, Clone)]
pub struct Module {
    guild: GuildId,
    kind: ModuleKind,
    enabled: bool,
}

impl From<models::ModuleSetting> for Module {
    fn from(m: models::ModuleSetting) -> Self {
        Self {
            guild: GuildId(m.guild as u64),
            kind: m.module,
            enabled: m.enabled,
        }
    }
}

impl Module {
    fn default_for_kind_with_guild(kind: ModuleKind, guild: GuildId) -> Self {
        match kind {
            ModuleKind::MassPing
            | ModuleKind::Crosspost
            | ModuleKind::DynamicSlowmode
            | ModuleKind::EmojiSpam
            | ModuleKind::MentionSpam
            | ModuleKind::Selfbot => Self {
                guild,
                kind,
                enabled: false,
            },
        }
    }

    pub fn get_all_modules_for_guild(guild: GuildId, db: &PgConnection) -> anyhow::Result<HashMap<ModuleKind, Module>> {
        use schema::module_settings;

        let mut modules = HashMap::new();
        for kind in ModuleKind::iter() {
            modules.insert(kind, Module::default_for_kind_with_guild(kind, guild));
        }

        for m in module_settings::table
            .filter(module_settings::guild.eq(guild.0 as i64))
            .load::<models::ModuleSetting>(db)?
        {
            modules.insert(m.module, m.into());
        }

        debug!("{:#?}", modules);
        Ok(modules)
    }

    pub fn get_module_for_guild(guild: GuildId, kind: ModuleKind, db: &PgConnection) -> anyhow::Result<Module> {
        use schema::module_settings;

        let module = module_settings::table
            .filter(
                module_settings::guild
                    .eq(guild.0 as i64)
                    .and(module_settings::module.eq(kind)),
            )
            .first::<models::ModuleSetting>(db)
            .optional()?
            .map_or_else(|| Module::default_for_kind_with_guild(kind, guild), Module::from);

        debug!("{:#?}", module);
        Ok(module)
    }

    pub fn kind(self) -> ModuleKind {
        self.kind
    }

    pub fn guild(self) -> GuildId {
        self.guild
    }

    pub fn enabled(self) -> bool {
        self.enabled
    }

    pub fn set_enabled(self, enabled: bool, db: &PgConnection) -> anyhow::Result<()> {
        use schema::module_settings;

        let enabled_setting = models::ModuleSetting {
            guild: self.guild.0 as i64,
            module: self.kind,
            enabled,
        };

        // return the inserted row's guild ID but don't store it anywhere, because this way diesel will error if the
        // insert affected no rows
        diesel::insert_into(module_settings::table)
            .values(&enabled_setting)
            .on_conflict((module_settings::guild, module_settings::module))
            .do_update()
            .set(&enabled_setting)
            .returning(module_settings::guild)
            .get_result::<i64>(db)?;

        debug!("Insert {:#?}", enabled_setting);
        Ok(())
    }

    pub fn get_actions(self, db: &PgConnection) -> anyhow::Result<Vec<Action>> {
        use schema::actions;

        let mut actions = Vec::new();
        for model in actions::table
            .filter(
                actions::guild
                    .eq(self.guild.0 as i64)
                    .and(actions::module.eq(self.kind)),
            )
            .load::<models::Action>(db)?
        {
            match model.action {
                ActionKind::RemoveMessage => actions.push(Action::remove_message()),
                ActionKind::Notify => actions.push(Action::notify(
                    model.in_channel.map(|c| ChannelId(c as u64)),
                    model
                        .message
                        .map(Cow::Owned)
                        .ok_or(InternalError::MissingField("message"))?,
                )),
            }
        }

        debug!("{:#?}", actions);
        Ok(actions)
    }

    pub fn add_action(self, action: Action, db: &PgConnection) -> anyhow::Result<i32> {
        use schema::actions;

        let action_model = match action.kind {
            ActionKind::RemoveMessage => models::NewAction {
                guild: self.guild.0 as i64,
                action: action.kind,
                module: self.kind,
                in_channel: None,
                message: None,
            },
            ActionKind::Notify => models::NewAction {
                guild: self.guild.0 as i64,
                action: action.kind,
                module: self.kind,
                in_channel: action.channel.map(|c| c.0 as i64),
                message: action.message.as_deref(),
            },
        };

        let id = diesel::insert_into(actions::table)
            .values(&action_model)
            .returning(actions::id)
            .get_result::<i32>(db)?;

        debug!("Insert {:?} -> ID {}", action_model, id);
        Ok(id)
    }

    pub fn remove_nth_action(self, n: usize, db: &PgConnection) -> anyhow::Result<()> {
        use schema::actions;

        let actions = actions::table
            .filter(
                actions::guild
                    .eq(self.guild.0 as i64)
                    .and(actions::module.eq(self.kind)),
            )
            .load::<models::Action>(db)?;
        let delete = actions.get(n).ok_or(ArgumentError::IndexOutOfRange(n))?;

        // return the deleted row's ID but don't store it anywhere, because this way diesel will error if the delete
        // affected no rows
        diesel::delete(actions::table.filter(actions::id.eq(delete.id)))
            .returning(actions::id)
            .get_result::<i32>(db)?;

        debug!("Delete {:?}", delete);
        Ok(())
    }

    pub fn action_count(self, db: &PgConnection) -> anyhow::Result<i64> {
        use diesel::dsl::*;
        use schema::actions;

        let count = actions::table
            .filter(
                actions::guild
                    .eq(self.guild.0 as i64)
                    .and(actions::module.eq(self.kind)),
            )
            .select(count_star())
            .first(db)?;

        debug!("# actions: {}", count);
        Ok(count)
    }
}
