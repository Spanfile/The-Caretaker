use serenity::model::id::ChannelId;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum InternalError {
    #[error("A deliberate error")]
    DeliberateError,
    #[error("Missing userdata '{0}'")]
    MissingUserdata(&'static str),
    #[error("Missing own shard ID {0} metadata in shard metadata collection")]
    MissingOwnShardMetadata(u64),
    #[error("Missing field '{0}' in model")]
    MissingField(&'static str),
    #[error("Invalid field '{0}' in model")]
    InvalidField(&'static str),
    #[error("Impossible case: {0}. This is a bug!")]
    ImpossibleCase(String),
    #[error("Conversion failed: {0}")]
    ConversionFailed(&'static str),
    #[error("Missing guild ID")]
    MissingGuildID,
}

#[derive(Error, Debug)]
pub enum ArgumentError {
    #[error("The index {0} is out of range")]
    UsizeOutOfRange(usize),
    #[error("The index {0} is out of range")]
    I64OutOfRange(i64),
    #[error("That command cannot be used in my DMs")]
    NotSupportedInDM,
    #[error("The channel <#{0}> is not in this guild")]
    ChannelNotInGuild(ChannelId),
    #[error("No such setting: {0}")]
    NoSuchSetting(String),
    #[error("Invalid notify message format: {0}")]
    InvalidNotifyFormat(String),
    #[error("You do not have permission to run that command")]
    NoPermission,
    #[error("Exclusion already exists")]
    ExclusionAlreadyExists,
    #[error("No such exclusion")]
    NoSuchExclusion,
    #[error("The module already has the maximum amount of exclusions ({0} out of {1})")]
    ExclusionLimit(usize, usize),
    #[error("The module already has the maximum amount of actions ({0} out of {1})")]
    ActionLimit(usize, usize),
}
