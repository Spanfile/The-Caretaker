use crate::{models, module::ExclusionKind};
use serenity::model::id::{RoleId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusion {
    User(UserId),
    Role(RoleId),
}

#[derive(Debug)]
pub struct ModuleExclusion {
    exclusions: Vec<Exclusion>,
}

impl ModuleExclusion {
    pub fn from_db_rows(rows: &[models::ModuleExclusion]) -> ModuleExclusion {
        Self {
            exclusions: rows
                .iter()
                .map(|excl| match excl.kind {
                    ExclusionKind::User => Exclusion::User(UserId(excl.id as u64)),
                    ExclusionKind::Role => Exclusion::Role(RoleId(excl.id as u64)),
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.exclusions.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = Exclusion> + '_ {
        self.exclusions.iter().copied()
    }

    pub fn contains(&self, excl: Exclusion) -> bool {
        self.exclusions.iter().any(|e| *e == excl)
    }
}
