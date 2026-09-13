use tauri::State;

use crate::databases::Databases;
use crate::error::Result;

#[tauri::command]
pub(crate) async fn close(databases: State<'_, Databases>, db: String) -> Result<()> {
    databases.close(&db).await
}
