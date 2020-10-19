use super::schema::{actions, enabled_modules};

#[derive(Queryable, Insertable, Debug)]
#[table_name = "enabled_modules"]
pub struct ModuleEnableStatus {
    pub guild: i64,
    pub module: String,
    pub enabled: bool,
}

#[derive(Queryable, Insertable, Debug)]
pub struct Action {
    pub id: i32,
    pub guild: i64,
    pub module: String,
    pub action: String,
    pub in_channel: Option<i64>,
    pub message: Option<String>,
}
