use tauri::State;

use crate::databases::Databases;
use crate::error::Result;

#[tauri::command]
pub(crate) async fn in_transaction(databases: State<'_, Databases>, db: String) -> Result<bool> {
    databases.get(&db).await?.in_transaction().await
}
