use super::schema::{actions, guild_settings, module_settings, modules};
use crate::module::{action::ActionKind, ModuleKind};

#[derive(Queryable, Insertable, AsChangeset, Debug)]
pub struct Module {
    pub guild: i64,
    pub module: ModuleKind,
    pub enabled: bool,
}

#[derive(Queryable, Debug)]
pub struct Action {
    pub id: i32,
    pub guild: i64,
    pub module: ModuleKind,
    pub action: ActionKind,
    pub in_channel: Option<i64>,
    pub message: Option<String>,
}

#[derive(Insertable, Debug)]
#[table_name = "actions"]
pub struct NewAction<'a> {
    pub guild: i64,
    pub module: ModuleKind,
    pub action: ActionKind,
    pub in_channel: Option<i64>,
    pub message: Option<&'a str>,
}

#[derive(Queryable, Debug)]
pub struct ModuleSetting {
    pub guild: i64,
    pub module: ModuleKind,
    pub setting: String,
    pub value: String,
}

#[derive(Insertable, AsChangeset, Debug)]
#[table_name = "module_settings"]
pub struct NewModuleSetting<'a> {
    pub guild: i64,
    pub module: ModuleKind,
    pub setting: &'a str,
    pub value: &'a str,
}

#[derive(Queryable, Debug)]
pub struct GuildSettings {
    pub guild: i64,
    pub admin_role: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug)]
#[table_name = "guild_settings"]
// this is required so it is possible to set the Option fields to None in an ON CONFLICT DO UPDATE
#[changeset_options(treat_none_as_null = "true")]
pub struct NewGuildSettings {
    pub guild: i64,
    pub admin_role: Option<i64>,
}
