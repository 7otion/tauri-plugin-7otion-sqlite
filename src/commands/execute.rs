use serde_json::Value as JsonValue;
use tauri::State;

use crate::database::Execution;
use crate::databases::{Databases, HandleId};
use crate::error::Result;

#[tauri::command]
pub(crate) async fn execute(
    databases: State<'_, Databases>,
    handle: HandleId,
    sql: String,
    params: Vec<JsonValue>,
) -> Result<Execution> {
    databases
        .handle(handle)
        .await?
        .execute(handle, &sql, params)
        .await
}
