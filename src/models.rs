use super::schema::{actions, enabled_modules};

#[derive(Queryable, Insertable, Debug)]
#[table_name = "enabled_modules"]
pub struct ModuleEnableStatus {
    pub guild: i64,
    pub module: String,
    pub enabled: bool,
}

#[derive(Queryable, Debug)]
pub struct Action {
    pub id: i32,
    pub guild: i64,
    pub module: String,
    pub action: String,
    pub in_channel: Option<i64>,
    pub message: Option<String>,
}

#[derive(Insertable, Debug)]
#[table_name = "actions"]
pub struct NewAction<'a> {
    pub guild: i64,
    pub module: &'a str,
    pub action: &'a str,
    pub in_channel: Option<i64>,
    pub message: Option<&'a str>,
}
