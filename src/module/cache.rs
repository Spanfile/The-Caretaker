use super::{Module, ModuleKind};
use crate::DbConn;
use log::*;
use serenity::{model::id::GuildId, prelude::TypeMapKey};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct ModuleCache {
    guilds: Arc<RwLock<HashMap<GuildId, HashMap<ModuleKind, Module>>>>,
}

impl TypeMapKey for ModuleCache {
    type Value = ModuleCache;
}

impl ModuleCache {
    pub fn populate_from_db(db: &DbConn) -> anyhow::Result<Self> {
        let all_modules = Module::get_all_modules(db)?;
        let module_count = all_modules.len();
        let mut guilds: HashMap<GuildId, HashMap<ModuleKind, Module>> = HashMap::new();

        for module in all_modules {
            match guilds.entry(module.guild) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().insert(module.kind(), module);
                }
                Entry::Vacant(entry) => {
                    let mut new_guild_entry = HashMap::new();
                    new_guild_entry.insert(module.kind(), module);
                    entry.insert(new_guild_entry);
                }
            }
        }

        debug!(
            "Module cache populated. {} modules in total across {} guilds",
            module_count,
            guilds.len()
        );

        Ok(Self {
            guilds: Arc::new(RwLock::new(guilds)),
        })
    }

    pub async fn update(&self, module: Module) -> anyhow::Result<()> {
        let mut modules = self.guilds.write().await;
        match modules.entry(module.guild()) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().insert(module.kind, module);
            }
            Entry::Vacant(entry) => {
                let mut new_guild_entry = HashMap::new();
                new_guild_entry.insert(module.kind(), module);
                entry.insert(new_guild_entry);
            }
        }

        Ok(())
    }

    pub async fn get(&self, guild: GuildId, kind: ModuleKind) -> Module {
        let guilds = self.guilds.read().await;

        if let Some(modules) = guilds.get(&guild) {
            if let Some(module) = modules.get(&kind) {
                return *module;
            }
        }

        Module::default_with_kind_and_guild(kind, guild)
    }
}
