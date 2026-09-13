use serde_json::Value as JsonValue;
use tauri::State;

use crate::database::Execution;
use crate::databases::Databases;
use crate::error::Result;

#[tauri::command]
pub(crate) async fn execute(
    databases: State<'_, Databases>,
    db: String,
    sql: String,
    params: Vec<JsonValue>,
) -> Result<Execution> {
    databases.get(&db).await?.execute(&sql, params).await
}
