pub mod action;
pub mod cache;
pub mod settings;
pub mod dbimport {
    pub use super::{action::Action_kind, Exclusion_kind, Module_kind};
}
pub mod exclusion;

use self::{
    action::{Action, ActionKind},
    exclusion::{Exclusion, ModuleExclusion},
    settings::{ModuleSettings, Settings},
};
use crate::{
    error::{ArgumentError, InternalError},
    models::{self, NewModuleExclusion},
    schema, DbConn,
};
use diesel::prelude::*;
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
    EmojiSpam,
    MentionSpam,
    Selfbot,
    InviteLink,
    ChannelActivity,
    UserActivity,
}

// the database schema holds its own version of this enum, remember to modify it as well if modying this one
#[derive(Debug, Clone, Copy, PartialEq, Eq, DbEnum)]
#[DieselType = "Exclusion_kind"]
pub enum ExclusionKind {
    User,
    Role,
}

#[derive(Debug, Copy, Clone)]
pub struct Module {
    guild: GuildId,
    kind: ModuleKind,
    enabled: bool,
}

impl From<models::Module> for Module {
    fn from(m: models::Module) -> Self {
        Self {
            guild: GuildId(m.guild as u64),
            kind: m.module,
            enabled: m.enabled,
        }
    }
}

impl Module {
    fn default_with_kind_and_guild(kind: ModuleKind, guild: GuildId) -> Self {
        Self {
            guild,
            kind,
            enabled: false,
        }
    }

    pub fn get_all_modules(db: &DbConn) -> anyhow::Result<Vec<Module>> {
        use schema::modules;

        Ok(modules::table
            .load::<models::Module>(db)?
            .into_iter()
            .map(|m| m.into())
            .collect())
    }

    pub fn get_all_modules_for_guild(guild: GuildId, db: &DbConn) -> anyhow::Result<HashMap<ModuleKind, Module>> {
        use schema::modules;

        let mut modules = HashMap::new();
        for kind in ModuleKind::iter() {
            modules.insert(kind, Module::default_with_kind_and_guild(kind, guild));
        }

        for m in modules::table
            .filter(modules::guild.eq(guild.0 as i64))
            .load::<models::Module>(db)?
        {
            modules.insert(m.module, m.into());
        }

        debug!("Modules for {}: {:?}", guild, modules);
        Ok(modules)
    }

    pub fn get_module_for_guild(guild: GuildId, kind: ModuleKind, db: &DbConn) -> anyhow::Result<Module> {
        use schema::modules;

        let module = modules::table
            .filter(modules::guild.eq(guild.0 as i64).and(modules::module.eq(kind)))
            .first::<models::Module>(db)
            .optional()?
            .map_or_else(|| Module::default_with_kind_and_guild(kind, guild), Module::from);

        debug!("Module {} for {}: {:?}", kind, guild, module);
        Ok(module)
    }

    pub fn kind(self) -> ModuleKind {
        self.kind
    }

    pub fn guild(self) -> GuildId {
        self.guild
    }

    pub fn is_enabled(self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool, db: &DbConn) -> anyhow::Result<()> {
        use schema::modules;

        self.enabled = enabled;
        let enabled_setting = models::Module {
            guild: self.guild.0 as i64,
            module: self.kind,
            enabled,
        };

        // return the inserted row's guild ID but don't store it anywhere, because this way diesel will error if the
        // insert affected no rows
        diesel::insert_into(modules::table)
            .values(&enabled_setting)
            .on_conflict((modules::guild, modules::module))
            .do_update()
            .set(&enabled_setting)
            .returning(modules::guild)
            .get_result::<i64>(db)?;

        debug!("{:?}: insert module {:?}", self, enabled_setting);
        Ok(())
    }

    // since an Action may contain borrowed data in the message Cow, it has to always have its lifetime specified. in
    // this function's case, the Rust compiler simply infers the action's lifetime to be that of the database connection
    // reference's, which is incorrect (they don't share any data via reference). by specifying the 'static lifetime for
    // the returning action, it is "locked" to always containing only owned data (or the borrowed data would have to
    // be 'static as well). this function returns only new action objects that contain only owned data, so the
    // lifetime is valid
    pub fn get_actions(self, db: &DbConn) -> anyhow::Result<Vec<Action<'static>>> {
        use schema::actions;

        let actions = actions::table
            .filter(
                actions::guild
                    .eq(self.guild.0 as i64)
                    .and(actions::module.eq(self.kind)),
            )
            .load::<models::Action>(db)?
            .into_iter()
            .map::<Result<Action, InternalError>, _>(|m| match m.action {
                ActionKind::RemoveMessage => Ok(Action::remove_message()),
                ActionKind::Notify => Ok(Action::notify(
                    m.in_channel.map(|c| ChannelId(c as u64)),
                    m.message
                        .map(Cow::Owned)
                        .ok_or(InternalError::MissingField("message"))?,
                )),
            })
            .collect::<Result<_, _>>()?;

        debug!("{:?} actions: {:?}", self, actions);
        Ok(actions)
    }

    pub fn add_action(self, action: &Action, db: &DbConn) -> anyhow::Result<i32> {
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

        debug!("{:?}: insert action {:?} -> ID {}", self, action_model, id);
        Ok(id)
    }

    pub fn remove_nth_action(self, n: usize, db: &DbConn) -> anyhow::Result<()> {
        use schema::actions;

        let actions = actions::table
            .filter(
                actions::guild
                    .eq(self.guild.0 as i64)
                    .and(actions::module.eq(self.kind)),
            )
            .load::<models::Action>(db)?;
        let delete = actions.get(n).ok_or(ArgumentError::UsizeOutOfRange(n))?;

        // return the deleted row's ID but don't store it anywhere, because this way diesel will error if the delete
        // affected no rows
        diesel::delete(actions::table.filter(actions::id.eq(delete.id)))
            .returning(actions::id)
            .get_result::<i32>(db)?;

        debug!("{:?}: delete action {:?}", self, delete);
        Ok(())
    }

    pub fn action_count(self, db: &DbConn) -> anyhow::Result<usize> {
        use diesel::dsl::*;
        use schema::actions;

        let count = actions::table
            .filter(
                actions::guild
                    .eq(self.guild.0 as i64)
                    .and(actions::module.eq(self.kind)),
            )
            .select(count_star())
            .first::<i64>(db)?;

        debug!("{:?}: # actions: {}", self, count);
        Ok(count as usize)
    }

    pub fn get_settings(self, db: &DbConn) -> anyhow::Result<ModuleSettings> {
        use schema::module_settings;

        let rows = module_settings::table
            .filter(
                module_settings::guild
                    .eq(self.guild.0 as i64)
                    .and(module_settings::module.eq(self.kind)),
            )
            .load::<models::ModuleSetting>(db)?;
        let settings = ModuleSettings::from_db_rows(self.kind, &rows)?;

        debug!("{:?} settings: {:?}", self, settings);
        Ok(settings)
    }

    pub fn set_settings(self, settings: &ModuleSettings, db: &DbConn) -> anyhow::Result<()> {
        use diesel::pg::upsert::excluded;
        use schema::module_settings;

        let settings = settings.get_all();
        let rows = settings
            .iter()
            .map(|(setting, value)| models::NewModuleSetting {
                guild: self.guild.0 as i64,
                module: self.kind,
                setting,
                value,
            })
            .collect::<Vec<_>>();

        let new_values = diesel::insert_into(module_settings::table)
            .values(&rows)
            .on_conflict((
                module_settings::guild,
                module_settings::module,
                module_settings::setting,
            ))
            .do_update()
            .set(module_settings::value.eq(excluded(module_settings::value)))
            .returning((
                module_settings::guild,
                module_settings::module,
                module_settings::setting,
                module_settings::value,
            ))
            .get_results::<models::ModuleSetting>(db)?;

        debug!("{:?}: insert settings {:?} -> {:?}", self, rows, new_values);
        Ok(())
    }

    pub fn get_exclusions(self, db: &DbConn) -> anyhow::Result<ModuleExclusion> {
        use schema::module_exclusions;

        let rows = module_exclusions::table
            .filter(
                module_exclusions::guild
                    .eq(self.guild.0 as i64)
                    .and(module_exclusions::module.eq(self.kind)),
            )
            .load::<models::ModuleExclusion>(db)?;
        let exclusions = ModuleExclusion::from_db_rows(&rows);

        debug!("{:?} exclusions: {:?}", self, exclusions);
        Ok(exclusions)
    }

    pub fn add_exclusion(self, excl: Exclusion, db: &DbConn) -> anyhow::Result<()> {
        use schema::module_exclusions;

        let (kind, id) = match excl {
            Exclusion::User(id) => (ExclusionKind::User, id.0 as i64),
            Exclusion::Role(id) => (ExclusionKind::Role, id.0 as i64),
        };
        let exclusion_model = NewModuleExclusion {
            guild: self.guild.0 as i64,
            module: self.kind,
            kind,
            id,
        };

        // return the exclusion's guild ID but don't store it anywhere, because this way diesel will error if the insert
        // affected no rows
        diesel::insert_into(module_exclusions::table)
            .values(&exclusion_model)
            .returning(module_exclusions::guild)
            .get_result::<i64>(db)?;

        debug!("{:?}: insert exclusion {:?}", self, exclusion_model);
        Ok(())
    }

    pub fn remove_exclusion(self, excl: Exclusion, db: &DbConn) -> anyhow::Result<()> {
        use schema::module_exclusions;

        let (kind, id) = match excl {
            Exclusion::User(id) => (ExclusionKind::User, id.0 as i64),
            Exclusion::Role(id) => (ExclusionKind::Role, id.0 as i64),
        };

        // return the deleted row's ID but don't store it anywhere, because this way diesel will error if the delete
        // affected no rows
        diesel::delete(
            module_exclusions::table.filter(
                module_exclusions::guild
                    .eq(self.guild.0 as i64)
                    .and(module_exclusions::module.eq(self.kind))
                    .and(module_exclusions::kind.eq(kind))
                    .and(module_exclusions::id.eq(id)),
            ),
        )
        .returning(module_exclusions::guild)
        .get_result::<i64>(db)?;

        debug!("{:?}: delete exclusion {:?}", self, excl);
        Ok(())
    }
}
