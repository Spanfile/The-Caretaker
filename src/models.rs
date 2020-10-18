use super::schema::enabled_modules;

#[derive(Queryable)]
pub struct ModuleEnableStatus {
    pub guild: i64,
    pub module: String,
    pub enabled: bool,
}

#[derive(Insertable)]
#[table_name = "enabled_modules"]
pub struct NewModuleEnabledStatus<'a> {
    pub guild: i64,
    pub module: &'a str,
    pub enabled: i64,
}
