use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config::DatabaseConfig;
use crate::database::Database;
use crate::error::{Error, Result};

/// Every database the app has open, one connection per file.
pub struct Databases {
    data_directory: Option<PathBuf>,
    open: Mutex<HashMap<String, Arc<Database>>>,
}

impl Databases {
    pub(crate) fn new(data_directory: Option<PathBuf>) -> Self {
        Self {
            data_directory,
            open: Mutex::new(HashMap::new()),
        }
    }

    /// Opens `path`, or attaches to its open connection. With no config it attaches whatever
    /// the database was opened with; a config must match that exactly.
    pub async fn load(
        &self,
        path: impl AsRef<Path>,
        config: Option<DatabaseConfig>,
    ) -> Result<Arc<Database>> {
        let path = self.resolve(path.as_ref())?;
        let id = Self::identity(&path)?;

        let mut open = self.open.lock().await;

        if let Some(database) = open.get(&id) {
            return match config {
                Some(config) if &config != database.config() => Err(Error::ConfigMismatch(id)),
                _ => Ok(database.clone()),
            };
        }

        let database =
            Arc::new(Database::open(id.clone(), &path, config.unwrap_or_default()).await?);
        open.insert(id, database.clone());
        tracing::info!(database = %database.id(), "opened");

        Ok(database)
    }

    /// An open database, by the id `load` gave it.
    pub async fn get(&self, id: &str) -> Result<Arc<Database>> {
        self.open
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| Error::NotLoaded(id.to_owned()))
    }

    /// Closes the connection for every holder; closing one that is not open does nothing.
    pub async fn close(&self, id: &str) -> Result<()> {
        let database = self.open.lock().await.remove(id);

        if let Some(database) = database {
            database.close().await?;
            tracing::info!(database = %id, "closed");
        }
        Ok(())
    }

    fn resolve(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        let data_directory = self
            .data_directory
            .as_ref()
            .ok_or_else(|| Error::NoDataDirectory(path.display().to_string()))?;
        let resolved = data_directory.join(path);

        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(resolved)
    }

    /// The canonical path, so two spellings of one file share a connection.
    fn identity(path: &Path) -> Result<String> {
        let canonical = match path.canonicalize() {
            Ok(canonical) => canonical,
            Err(_) => {
                let name = path
                    .file_name()
                    .ok_or_else(|| Error::NoFileName(path.display().to_string()))?;
                let parent = match path.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => parent,
                    _ => Path::new("."),
                };
                parent.canonicalize()?.join(name)
            }
        };

        Ok(canonical.to_string_lossy().into_owned())
    }
}
