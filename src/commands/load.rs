use tauri::State;

use crate::config::DatabaseConfig;
use crate::databases::Databases;
use crate::error::Result;

#[tauri::command]
pub(crate) async fn load(
    databases: State<'_, Databases>,
    path: String,
    config: Option<DatabaseConfig>,
) -> Result<String> {
    if config.as_ref().is_some_and(|config| config.key.is_some()) {
        tracing::warn!(
            path,
            "an encryption key was passed from JavaScript: it crosses IPC and stays in webview memory; \
             supply it from Rust with app.sqlite().load() instead"
        );
    }

    let database = databases.load(&path, config).await?;

    // A page loads each database once, so a transaction open here belongs to a page that is gone.
    if database.roll_back_leftover_transaction().await? {
        tracing::warn!(database = %database.id(), "rolled back a transaction left open by a previous page");
    }

    Ok(database.id().to_owned())
}
