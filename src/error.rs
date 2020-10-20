use thiserror::Error;

#[derive(Error, Debug)]
pub enum CaretakerError {
    #[error("A deliberate error")]
    DeliberateError,
    #[error("No guild ID in received message")]
    NoGuildId,
    #[error("No database connection in userdata")]
    NoDatabaseConnection,
    #[error("No shard metadata collection in userdata")]
    NoShardMetadataCollection,
    #[error("Missing own shard ID {0} metadata in shard metadata collection")]
    MissingOwnShardMetadata(u64),
    #[error("Missing field '{0}' in model")]
    MissingField(&'static str),
    #[error("The index {0} is out of range")]
    IndexOutOfRange(usize),
}
