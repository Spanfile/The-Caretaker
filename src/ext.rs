use crate::error::InternalError;
use serenity::prelude::{TypeMap, TypeMapKey};
use std::{any::type_name, time::Duration};

pub trait DurationExt {
    fn round_to_seconds(self) -> Duration;
}

impl DurationExt for Duration {
    fn round_to_seconds(self) -> Duration {
        Duration::from_secs(self.as_secs())
    }
}

pub trait UserdataExt {
    fn get_userdata<T>(&self) -> Result<&T::Value, InternalError>
    where
        T: TypeMapKey;
    fn get_userdata_mut<T>(&mut self) -> Result<&mut T::Value, InternalError>
    where
        T: TypeMapKey;
}

impl UserdataExt for TypeMap {
    fn get_userdata<T>(&self) -> Result<&T::Value, InternalError>
    where
        T: TypeMapKey,
    {
        self.get::<T>()
            .ok_or_else(|| InternalError::MissingUserdata(type_name::<T>()))
    }

    fn get_userdata_mut<T>(&mut self) -> Result<&mut T::Value, InternalError>
    where
        T: TypeMapKey,
    {
        self.get_mut::<T>()
            .ok_or_else(|| InternalError::MissingUserdata(type_name::<T>()))
    }
}
