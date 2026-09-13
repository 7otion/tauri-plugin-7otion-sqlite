use tauri::State;

use crate::databases::{Databases, HandleId};
use crate::error::Result;

#[tauri::command]
pub(crate) async fn in_transaction(
    databases: State<'_, Databases>,
    handle: HandleId,
) -> Result<bool> {
    databases.handle(handle).await?.in_transaction().await
}
