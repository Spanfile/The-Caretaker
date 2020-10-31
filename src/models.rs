use super::schema::{actions, module_settings, modules};
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

#[derive(Insertable, Debug)]
#[table_name = "module_settings"]
pub struct NewModuleSetting<'a> {
    pub guild: i64,
    pub module: ModuleKind,
    pub setting: &'a str,
    pub value: &'a str,
}
