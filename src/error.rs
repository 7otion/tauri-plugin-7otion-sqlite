use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(
        "database handle {0} is not open: it was closed, its page reloaded, or its database was closed"
    )]
    UnknownHandle(u64),

    #[error("database {0} was closed")]
    Closed(String),

    #[error(
        "database {0} is already open with a different configuration: release every handle to it, \
         and close it from Rust if the app loaded it there, then load it again with the new one"
    )]
    ConfigMismatch(String),

    #[error("a key needs the plugin's `encryption` feature, which this build does not enable")]
    EncryptionDisabled,

    #[error("invalid PRAGMA name {0:?}")]
    InvalidPragma(String),

    #[error("PRAGMA {0} is set through the config's key, not its pragmas")]
    KeyAsPragma(String),

    #[error("{0} cannot be bound: only null, booleans, numbers and strings can")]
    Unbindable(&'static str),

    #[error("relative path {0} needs the app data directory, which is unavailable")]
    NoDataDirectory(String),

    #[error("{0} names no file")]
    NoFileName(String),
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
