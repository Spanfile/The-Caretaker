use thiserror::Error;

#[derive(Error, Debug)]
pub enum InternalError {
    #[error("A deliberate error")]
    DeliberateError,
    #[error("No guild ID in received message")]
    NoGuildId,
    #[error("Missing userdata '{0}'")]
    MissingUserdata(&'static str),
    #[error("Missing own shard ID {0} metadata in shard metadata collection")]
    MissingOwnShardMetadata(u64),
    #[error("Missing field '{0}' in model")]
    MissingField(&'static str),
}

#[derive(Error, Debug)]
pub enum ArgumentError {
    #[error("The index {0} is out of range")]
    IndexOutOfRange(usize),
}
