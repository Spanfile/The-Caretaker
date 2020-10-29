use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum InternalError {
    #[error("A deliberate error")]
    DeliberateError,
    #[error("Missing userdata '{0}'")]
    MissingUserdata(&'static str),
    #[error("Missing own shard ID {0} metadata in shard metadata collection")]
    MissingOwnShardMetadata(u64),
    #[error("Missing field '{0}' in model")]
    MissingField(&'static str),
}

#[derive(Error, Debug, Copy, Clone)]
pub enum ArgumentError {
    #[error("The index {0} is out of range")]
    IndexOutOfRange(usize),
    #[error("That command cannot be used in my DMs!")]
    NotSupportedInDM,
}
