use tauri::State;

use crate::databases::{Databases, HandleId};
use crate::error::Result;

#[tauri::command]
pub(crate) async fn close(databases: State<'_, Databases>, handle: HandleId) -> Result<()> {
    databases.release(handle).await
}
