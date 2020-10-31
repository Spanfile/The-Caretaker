use super::ModuleKind;
use crate::{
    error::{InternalError, SettingsError},
    models,
};
use enum_dispatch::enum_dispatch;
use std::convert::TryFrom;

#[enum_dispatch]
pub trait Settings {
    fn get(&self, setting: &'static str) -> Result<SettingValue, SettingsError>;
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
pub enum SettingValue {
    usize(usize),
    i16(i16),
    i64(i64),
}

#[enum_dispatch(Settings)]
#[derive(Debug)]
pub enum ModuleSettings {
    MassPingSettings,
    CrosspostSettings,
    DynamicSlowmodeSettings,
    UserSlowmodeSettings,
    EmojiSpamSettings,
    MentionSpamSettings,
    SelfbotSettings,
    InviteLinkSettings,
}

impl ModuleSettings {
    pub fn from_db_rows(module: ModuleKind, rows: &[models::ModuleSetting]) -> anyhow::Result<Self> {
        match module {
            ModuleKind::MassPing => Ok(MassPingSettings::from_db_rows(rows)?.into()),
            ModuleKind::Crosspost => Ok(CrosspostSettings::from_db_rows(rows)?.into()),
            ModuleKind::DynamicSlowmode => Ok(DynamicSlowmodeSettings::from_db_rows(rows)?.into()),
            ModuleKind::UserSlowmode => Ok(UserSlowmodeSettings::from_db_rows(rows)?.into()),
            ModuleKind::EmojiSpam => Ok(EmojiSpamSettings::from_db_rows(rows)?.into()),
            ModuleKind::MentionSpam => Ok(MentionSpamSettings::from_db_rows(rows)?.into()),
            ModuleKind::Selfbot => Ok(SelfbotSettings::from_db_rows(rows)?.into()),
            ModuleKind::InviteLink => Ok(InviteLinkSettings::from_db_rows(rows)?.into()),
        }
    }
}

macro_rules! from_impls {
    ($($type:ident),+) => {
        $(
            impl From<$type> for SettingValue {
                fn from(val: $type) -> Self {
                    Self::$type(val)
                }
            }

            impl TryFrom<SettingValue> for $type {
                type Error = SettingsError;

                fn try_from(val: SettingValue) -> Result<Self, Self::Error> {
                    if let SettingValue::$type(val) = val {
                        Ok(val)
                    } else {
                        Err(SettingsError::InvalidType {
                            value: val,
                            ty: "$type",
                        })
                    }
                }
            }
        )+
    };
}

macro_rules! empty_settings {
    ($($settings:ident),+) => {
        $(
            #[derive(Debug, Default)]
            pub struct $settings {}
            impl $settings {
                fn from_db_rows(_: &[models::ModuleSetting]) -> anyhow::Result<Self> {
                    Ok(Self {})
                }
            }
            impl Settings for $settings {
                fn get(&self, setting: &'static str) -> Result<SettingValue, SettingsError> {
                    Err(SettingsError::NoSuchSetting(setting))
                }
            }
        )+
    };
}

macro_rules! settings {
    ($name:ident, $(($setting_name:ident: $setting_type:ty => $default:expr)),+) => {
        #[derive(Debug)]
        pub struct $name {
            $($setting_name: $setting_type,)+
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

        impl Settings for $name {
            fn get(&self, setting: &'static str) -> Result<SettingValue, SettingsError> {
                match setting {
                    $(stringify!($setting_name) => Ok(self.$setting_name.into()),)+
                    _ => Err(SettingsError::NoSuchSetting(setting)),
                }
            }
        }
    };
}

from_impls!(usize, i16, i64);

empty_settings!(
    MassPingSettings,
    DynamicSlowmodeSettings,
    UserSlowmodeSettings,
    EmojiSpamSettings,
    MentionSpamSettings,
    SelfbotSettings,
    InviteLinkSettings
);

settings!(
    CrosspostSettings,
    (minimum_length: usize => 5),
    (threshold: i16 => 80),
    (timeout: i64 => 3600)
);
