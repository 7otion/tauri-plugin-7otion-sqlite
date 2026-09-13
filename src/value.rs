use serde_json::{Number, Value as JsonValue};
use sqlx::query::Query;
use sqlx::sqlite::{Sqlite, SqliteArguments, SqliteValueRef};
use sqlx::{TypeInfo, Value, ValueRef};

use crate::error::{Error, Result};

pub(crate) type SqliteQuery<'q> = Query<'q, Sqlite, SqliteArguments<'q>>;

/// A whole number within i64 binds as INTEGER, any other number as REAL.
pub(crate) fn bind(query: SqliteQuery<'_>, value: JsonValue) -> Result<SqliteQuery<'_>> {
    Ok(match value {
        JsonValue::Null => query.bind(None::<String>),
        JsonValue::Bool(value) => query.bind(i64::from(value)),
        JsonValue::Number(number) => match number.as_i64() {
            Some(integer) => query.bind(integer),
            None => query.bind(
                number
                    .as_f64()
                    .ok_or(Error::Unbindable("a number beyond f64"))?,
            ),
        },
        JsonValue::String(text) => query.bind(text),
        JsonValue::Array(_) => return Err(Error::Unbindable("an array")),
        JsonValue::Object(_) => return Err(Error::Unbindable("an object")),
    })
}

/// Decoded by the value's own storage class, not the column's declared type.
pub(crate) fn to_json(value: SqliteValueRef<'_>) -> Result<JsonValue> {
    if value.is_null() {
        return Ok(JsonValue::Null);
    }

    let storage = value.type_info().name().to_owned();
    let owned = ValueRef::to_owned(&value);

    Ok(match storage.as_str() {
        "INTEGER" => JsonValue::from(owned.try_decode::<i64>()?),
        // JSON has no infinity; SQLite never stores NaN.
        "REAL" => {
            Number::from_f64(owned.try_decode::<f64>()?).map_or(JsonValue::Null, JsonValue::Number)
        }
        "BLOB" => JsonValue::from(owned.try_decode::<Vec<u8>>()?),
        _ => JsonValue::String(owned.try_decode::<String>()?),
    })
}
