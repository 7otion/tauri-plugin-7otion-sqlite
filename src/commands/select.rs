use serde_json::Value as JsonValue;
use tauri::State;

use crate::database::Rows;
use crate::databases::Databases;
use crate::error::Result;

#[tauri::command]
pub(crate) async fn select(
    databases: State<'_, Databases>,
    db: String,
    sql: String,
    params: Vec<JsonValue>,
) -> Result<Rows> {
    databases.get(&db).await?.select(&sql, params).await
}
