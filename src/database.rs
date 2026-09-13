use std::ops::{Deref, DerefMut};
use std::path::Path;

use serde::Serialize;
use serde_json::Value as JsonValue;
use sqlx::sqlite::SqliteConnection;
use sqlx::{Column, ConnectOptions, Connection, Row};
use tokio::sync::{Mutex, MutexGuard};

use crate::config::DatabaseConfig;
use crate::error::{Error, Result};
use crate::value;

/// One open database file and its single connection.
pub struct Database {
    id: String,
    config: DatabaseConfig,
    connection: Mutex<Option<SqliteConnection>>,
}

/// A query result as column names plus positional rows, so names are not repeated per row.
#[derive(Debug, Serialize)]
pub(crate) struct Rows {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<JsonValue>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Execution {
    pub(crate) rows_affected: u64,
    pub(crate) last_insert_id: i64,
}

impl Database {
    pub(crate) async fn open(id: String, path: &Path, config: DatabaseConfig) -> Result<Self> {
        let connection = config.connect_options(path)?.connect().await?;

        Ok(Self {
            id,
            config,
            connection: Mutex::new(Some(connection)),
        })
    }

    /// The resolved path, which identifies the database to the plugin.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The connection, held until the guard drops; everything else on this database waits.
    pub async fn lock(&self) -> Result<ConnectionGuard<'_>> {
        let guard = self.connection.lock().await;
        if guard.is_none() {
            return Err(Error::Closed(self.id.clone()));
        }
        Ok(ConnectionGuard(guard))
    }

    pub(crate) fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    pub(crate) async fn select(&self, sql: &str, params: Vec<JsonValue>) -> Result<Rows> {
        tracing::debug!(database = %self.id, sql, params = params.len(), "select");

        let mut query = sqlx::query(sql);
        for param in params {
            query = value::bind(query, param)?;
        }

        let mut connection = self.lock().await?;
        let rows = query.fetch_all(&mut *connection).await?;

        let columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|column| column.name().to_owned())
                    .collect()
            })
            .unwrap_or_default();

        let rows = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|index| value::to_json(row.try_get_raw(index)?))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Rows { columns, rows })
    }

    pub(crate) async fn execute(&self, sql: &str, params: Vec<JsonValue>) -> Result<Execution> {
        tracing::debug!(database = %self.id, sql, params = params.len(), "execute");

        let mut query = sqlx::query(sql);
        for param in params {
            query = value::bind(query, param)?;
        }

        let mut connection = self.lock().await?;
        let result = query.execute(&mut *connection).await?;

        Ok(Execution {
            rows_affected: result.rows_affected(),
            last_insert_id: result.last_insert_rowid(),
        })
    }

    pub(crate) async fn in_transaction(&self) -> Result<bool> {
        let mut connection = self.lock().await?;
        Self::transaction_open(&mut connection).await
    }

    /// Rolls back a transaction no caller can still own, such as one a reloaded page left open.
    pub(crate) async fn roll_back_leftover_transaction(&self) -> Result<bool> {
        let mut connection = self.lock().await?;
        if !Self::transaction_open(&mut connection).await? {
            return Ok(false);
        }

        sqlx::query("ROLLBACK").execute(&mut *connection).await?;
        Ok(true)
    }

    pub(crate) async fn close(&self) -> Result<()> {
        if let Some(connection) = self.connection.lock().await.take() {
            connection.close().await?;
        }
        Ok(())
    }

    async fn transaction_open(connection: &mut SqliteConnection) -> Result<bool> {
        let mut handle = connection.lock_handle().await?;
        // SAFETY: the handle is locked, so the connection's worker is not using it meanwhile.
        let autocommit =
            unsafe { libsqlite3_sys::sqlite3_get_autocommit(handle.as_raw_handle().as_ptr()) };
        Ok(autocommit == 0)
    }
}

/// Exclusive use of a database's connection; the next caller waits until it drops.
pub struct ConnectionGuard<'a>(MutexGuard<'a, Option<SqliteConnection>>);

impl Deref for ConnectionGuard<'_> {
    type Target = SqliteConnection;

    fn deref(&self) -> &SqliteConnection {
        self.0
            .as_ref()
            .expect("a guard is only made for an open connection")
    }
}

impl DerefMut for ConnectionGuard<'_> {
    fn deref_mut(&mut self) -> &mut SqliteConnection {
        self.0
            .as_mut()
            .expect("a guard is only made for an open connection")
    }
}
