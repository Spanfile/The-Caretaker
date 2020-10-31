use super::ModuleKind;
use crate::{error::InternalError, models};
use std::convert::TryFrom;

#[derive(Debug)]
pub enum ModuleSettings {
    MassPingSettings(MassPingSettings),
    CrosspostSettings(CrosspostSettings),
    DynamicSlowmodeSettings(DynamicSlowmodeSettings),
    UserSlowmodeSettings(UserSlowmodeSettings),
    EmojiSpamSettings(EmojiSpamSettings),
    MentionSpamSettings(MentionSpamSettings),
    SelfbotSettings(SelfbotSettings),
    InviteLinkSettings(InviteLinkSettings),
}

impl ModuleSettings {
    pub fn from_db_rows(module: ModuleKind, rows: &[models::ModuleSetting]) -> anyhow::Result<Self> {
        match module {
            ModuleKind::MassPing => Ok(Self::MassPingSettings(MassPingSettings::from_db_rows(rows)?)),
            ModuleKind::Crosspost => Ok(Self::CrosspostSettings(CrosspostSettings::from_db_rows(rows)?)),
            ModuleKind::DynamicSlowmode => Ok(Self::DynamicSlowmodeSettings(DynamicSlowmodeSettings::from_db_rows(
                rows,
            )?)),
            ModuleKind::UserSlowmode => Ok(Self::UserSlowmodeSettings(UserSlowmodeSettings::from_db_rows(rows)?)),
            ModuleKind::EmojiSpam => Ok(Self::EmojiSpamSettings(EmojiSpamSettings::from_db_rows(rows)?)),
            ModuleKind::MentionSpam => Ok(Self::MentionSpamSettings(MentionSpamSettings::from_db_rows(rows)?)),
            ModuleKind::Selfbot => Ok(Self::SelfbotSettings(SelfbotSettings::from_db_rows(rows)?)),
            ModuleKind::InviteLink => Ok(Self::InviteLinkSettings(InviteLinkSettings::from_db_rows(rows)?)),
        }
    }
}

macro_rules! create_empty_settings {
    ($($settings:ident),+) => {
        $(
            #[derive(Debug, Default)]
            pub struct $settings {}
            impl $settings {
                fn from_db_rows(_: &[models::ModuleSetting]) -> anyhow::Result<Self> {
                    Ok(Self {})
                }
            }
        )+
    };
}

macro_rules! create_settings {
    ($name:ident, $(($setting_name:ident: $setting_type:ty => $default:expr)),+) => {
        #[derive(Debug)]
        pub struct $name {
            $(pub $setting_name: $setting_type,)+
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($setting_name: $default),+ }
            }
        }

        impl $name {
            fn from_db_rows(rows: &[models::ModuleSetting]) -> anyhow::Result<Self> {
                let mut new_self = Self::default();
                for row in rows {
                    match row.setting.as_ref() {
                        $(stringify!($setting_name) => new_self.$setting_name = row.value.parse::<$setting_type>()?,)+
                        _ => return Err(InternalError::InvalidField("setting").into()),
                    }
                }
                Ok(new_self)
            }
        }
    };
}

macro_rules! setting_from_impls {
    ($($name:ident),+) => {
        $(
            impl TryFrom<ModuleSettings> for $name {
                type Error = InternalError;

                fn try_from(settings: ModuleSettings) -> Result<Self, Self::Error> {
                    match settings {
                        ModuleSettings::$name(s) => Ok(s),
                        _ => Err(InternalError::ImpossibleCase(format!("attempt to ")))
                    }
                }
            }
        )+
    };
}

create_empty_settings!(
    MassPingSettings,
    DynamicSlowmodeSettings,
    UserSlowmodeSettings,
    EmojiSpamSettings,
    MentionSpamSettings,
    SelfbotSettings,
    InviteLinkSettings
);

create_settings!(
    CrosspostSettings,
    (minimum_length: usize => 5),
    (threshold: i16 => 80),
    (timeout: i64 => 3600)
);

setting_from_impls!(
    MassPingSettings,
    CrosspostSettings,
    DynamicSlowmodeSettings,
    UserSlowmodeSettings,
    EmojiSpamSettings,
    MentionSpamSettings,
    SelfbotSettings,
    InviteLinkSettings
);
