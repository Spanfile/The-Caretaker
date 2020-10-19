use super::schema::enabled_modules;

#[derive(Queryable, Insertable, Debug)]
#[table_name = "enabled_modules"]
pub struct ModuleEnableStatus {
    pub guild: i64,
    pub module: String,
    pub enabled: bool,
}
