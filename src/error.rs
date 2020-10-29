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
    #[error("Impossible case: {0}. This is a bug!")]
    ImpossibleCase(String),
}

#[derive(Error, Debug, Copy, Clone)]
pub enum ArgumentError {
    #[error("The index {0} is out of range")]
    IndexOutOfRange(usize),
    #[error("That command cannot be used in my DMs")]
    NotSupportedInDM,
    #[error("The channel <#{0}> is not in this guild")]
    ChannelNotInGuild(ChannelId),
}
