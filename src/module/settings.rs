use super::ModuleKind;
use crate::{
    error::{ArgumentError, InternalError},
    models,
};
use enum_dispatch::enum_dispatch;

pub trait FromDbRows: Sized {
    fn from_db_rows(rows: &[models::ModuleSetting]) -> anyhow::Result<Self>;
}

#[enum_dispatch]
pub trait Settings {
    fn get_all(&self) -> Vec<(&'static str, String)>;
    fn description_for(&self, setting: &str) -> Result<&'static str, ArgumentError>;
    fn default_for(&self, setting: &str) -> Result<&'static str, ArgumentError>;
    fn set(&mut self, setting: &str, value: &str) -> anyhow::Result<()>;
    fn reset(&mut self, setting: &str) -> Result<(), ArgumentError>;
}

#[enum_dispatch(Settings)]
#[derive(Debug)]
pub enum ModuleSettings {
    MassPingSettings,
    CrosspostSettings,
    EmojiSpamSettings,
    MentionSpamSettings,
    SelfbotSettings,
    InviteLinkSettings,
    ChannelActivitySettings,
    UserActivitySettings,
}

impl ModuleSettings {
    pub fn from_db_rows(module: ModuleKind, rows: &[models::ModuleSetting]) -> anyhow::Result<Self> {
        match module {
            ModuleKind::MassPing => Ok(Self::MassPingSettings(MassPingSettings::from_db_rows(rows)?)),
            ModuleKind::Crosspost => Ok(Self::CrosspostSettings(CrosspostSettings::from_db_rows(rows)?)),
            ModuleKind::EmojiSpam => Ok(Self::EmojiSpamSettings(EmojiSpamSettings::from_db_rows(rows)?)),
            ModuleKind::MentionSpam => Ok(Self::MentionSpamSettings(MentionSpamSettings::from_db_rows(rows)?)),
            ModuleKind::Selfbot => Ok(Self::SelfbotSettings(SelfbotSettings::from_db_rows(rows)?)),
            ModuleKind::InviteLink => Ok(Self::InviteLinkSettings(InviteLinkSettings::from_db_rows(rows)?)),
            ModuleKind::ChannelActivity => Ok(Self::ChannelActivitySettings(ChannelActivitySettings::from_db_rows(
                rows,
            )?)),
            ModuleKind::UserActivity => Ok(Self::UserActivitySettings(UserActivitySettings::from_db_rows(rows)?)),
        }
    }
}

macro_rules! create_empty_settings {
    ($($settings:ident),+) => {
        $(#[derive(Debug, Default)]
        pub struct $settings {}

        impl FromDbRows for $settings {
            fn from_db_rows(_: &[models::ModuleSetting]) -> anyhow::Result<Self> {
                Ok(Self {})
            }
        }

        impl Settings for $settings {
            fn get_all(&self) -> Vec<(&'static str, String)> {
                vec![]
            }

            fn description_for(&self, setting: &str) -> Result<&'static str, ArgumentError> {
                Err(ArgumentError::NoSuchSetting(String::from(setting)))
            }

            fn default_for(&self, setting: &str) -> Result<&'static str, ArgumentError> {
                Err(ArgumentError::NoSuchSetting(String::from(setting)))
            }

            fn set(&mut self, setting: &str, _: &str) -> anyhow::Result<()> {
                Err(ArgumentError::NoSuchSetting(String::from(setting)).into())
            }

            fn reset(&mut self, setting: &str) -> Result<(), ArgumentError> {
                Err(ArgumentError::NoSuchSetting(String::from(setting)))
            }
        })+
    };
}

macro_rules! create_settings {
    ($name:ident, $(($setting_name:ident: $setting_type:ty => $default:expr, $description:literal)),+) => {
        #[derive(Debug)]
        pub struct $name {
            $(pub $setting_name: $setting_type,)+
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($setting_name: $default),+ }
            }
        }

        impl FromDbRows for $name {
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

        impl Settings for $name {
            fn get_all(&self) -> Vec<(&'static str, String)> {
                vec![$((stringify!($setting_name), self.$setting_name.to_string())),+]
            }

            fn description_for(&self, setting: &str) -> Result<&'static str, ArgumentError> {
                match setting {
                    $(stringify!($setting_name) => Ok($description),)+
                    _ => Err(ArgumentError::NoSuchSetting(String::from(setting)))
                }
            }

            fn default_for(&self, setting: &str) -> Result<&'static str, ArgumentError> {
                match setting {
                    $(stringify!($setting_name) => Ok(stringify!($default)),)+
                    _ => Err(ArgumentError::NoSuchSetting(String::from(setting)))
                }
            }

            fn set(&mut self, setting: &str, value: &str) -> anyhow::Result<()> {
                match setting {
                    $(stringify!($setting_name) => Ok(self.$setting_name = value.parse()?),)+
                    _ => Err(ArgumentError::NoSuchSetting(String::from(setting)).into()),
                }
            }

            fn reset(&mut self, setting: &str) -> Result<(), ArgumentError> {
                match setting {
                    $(stringify!($setting_name) => {
                        self.$setting_name = $default;
                        Ok(())
                    })+
                    _ => Err(ArgumentError::NoSuchSetting(String::from(setting))),
                }
            }
        }
    };
}

// there have to be identical empty settings for each type instead of them all sharing one empty settings type because
// enum_dispatch requires each variant in the settings enum to contain an unique type
create_empty_settings!(
    MassPingSettings,
    EmojiSpamSettings,
    MentionSpamSettings,
    SelfbotSettings,
    InviteLinkSettings,
    ChannelActivitySettings,
    UserActivitySettings
);

create_settings!(
    CrosspostSettings,
    (minimum_length: usize => 5, "Ignore messages below this length"),
    (threshold: i16 => 80, "The similarity threshold. Must be an integer between -128 and 128 where 128 means entirely similar, i.e. equal"),
    (timeout: u32 => 3600, "Ignore older messages than this timeout. The value is in seconds")
);
