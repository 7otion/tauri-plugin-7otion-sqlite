use std::fmt;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;

use crate::error::{Error, Result};

const DEFAULT_PRAGMAS: &[(&str, &str)] = &[
    ("journal_mode", "WAL"),
    ("foreign_keys", "ON"),
    ("synchronous", "NORMAL"),
];

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// How a database is opened. Loading an open database with a different config is refused.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    /// SQLCipher key; needs the `encryption` feature.
    pub key: Option<String>,
    /// Run in order after the defaults; one naming a default replaces it.
    pub pragmas: Vec<(String, String)>,
}

impl DatabaseConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn pragma(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.pragmas.push((name.into(), value.into()));
        self
    }

    pub(crate) fn connect_options(&self, path: &Path) -> Result<SqliteConnectOptions> {
        let mut options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .busy_timeout(DEFAULT_BUSY_TIMEOUT);

        for (name, value) in DEFAULT_PRAGMAS {
            options = options.pragma(*name, *value);
        }

        for (name, value) in &self.pragmas {
            Self::validate_pragma(name)?;
            options = options.pragma(name.clone(), value.clone());
        }

        if let Some(key) = &self.key {
            options = Self::with_key(options, key)?;
        }

        Ok(options)
    }

    fn validate_pragma(name: &str) -> Result<()> {
        let mut chars = name.chars();
        let valid = chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');

        if !valid {
            return Err(Error::InvalidPragma(name.to_owned()));
        }
        if name.eq_ignore_ascii_case("key") || name.eq_ignore_ascii_case("rekey") {
            return Err(Error::KeyAsPragma(name.to_owned()));
        }
        Ok(())
    }

    #[cfg(feature = "encryption")]
    fn with_key(options: SqliteConnectOptions, key: &str) -> Result<SqliteConnectOptions> {
        // sqlx runs `key` before every other PRAGMA, as SQLCipher requires.
        Ok(options.pragma("key", format!("'{}'", key.replace('\'', "''"))))
    }

    #[cfg(not(feature = "encryption"))]
    fn with_key(_options: SqliteConnectOptions, _key: &str) -> Result<SqliteConnectOptions> {
        Err(Error::EncryptionDisabled)
    }
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("key", &self.key.as_ref().map(|_| "<redacted>"))
            .field("pragmas", &self.pragmas)
            .finish()
    }
}
