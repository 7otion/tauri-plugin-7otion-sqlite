use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value as JsonValue, json};

use crate::database::Database;
use crate::{DatabaseConfig, Databases, Error};

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tauri-plugin-7otion-sqlite-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn databases(&self) -> Databases {
        Databases::new(Some(self.0.clone()))
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run<F: Future>(future: F) -> F::Output {
    tauri::async_runtime::block_on(future)
}

async fn scalar(database: &Database, sql: &str, params: Vec<JsonValue>) -> JsonValue {
    let mut rows = database.select(sql, params).await.unwrap().rows;
    rows.remove(0).remove(0)
}

#[test]
fn values_round_trip() {
    let directory = TempDirectory::new("values");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();
        database
            .execute("CREATE TABLE t (a, b, c, d, e)", vec![])
            .await
            .unwrap();
        database
            .execute(
                "INSERT INTO t VALUES (?, ?, ?, ?, ?)",
                vec![
                    json!(null),
                    json!(true),
                    json!(42),
                    json!(1.5),
                    json!("text"),
                ],
            )
            .await
            .unwrap();

        let result = database.select("SELECT * FROM t", vec![]).await.unwrap();

        assert_eq!(result.columns, ["a", "b", "c", "d", "e"]);
        assert_eq!(
            result.rows,
            [vec![
                json!(null),
                json!(1),
                json!(42),
                json!(1.5),
                json!("text")
            ]]
        );
        database.close().await.unwrap();
    });
}

#[test]
fn whole_numbers_bind_as_integers() {
    let directory = TempDirectory::new("numbers");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();

        assert_eq!(
            scalar(&database, "SELECT typeof(?)", vec![json!(5)]).await,
            json!("integer")
        );
        assert_eq!(
            scalar(&database, "SELECT typeof(?)", vec![json!(5.5)]).await,
            json!("real")
        );
        database.close().await.unwrap();
    });
}

#[test]
fn blobs_read_as_bytes_and_infinity_as_null() {
    let directory = TempDirectory::new("blobs");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();

        assert_eq!(
            scalar(&database, "SELECT x'0102ff'", vec![]).await,
            json!([1, 2, 255])
        );
        assert_eq!(scalar(&database, "SELECT 9e999", vec![]).await, json!(null));
        database.close().await.unwrap();
    });
}

#[test]
fn arrays_and_objects_are_refused() {
    let directory = TempDirectory::new("unbindable");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();

        let array = database.select("SELECT ?", vec![json!([1])]).await;
        let object = database.select("SELECT ?", vec![json!({ "a": 1 })]).await;

        assert!(matches!(array, Err(Error::Unbindable("an array"))));
        assert!(matches!(object, Err(Error::Unbindable("an object"))));
        database.close().await.unwrap();
    });
}

#[test]
fn an_empty_result_has_no_columns() {
    let directory = TempDirectory::new("empty");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();
        database
            .execute("CREATE TABLE t (a)", vec![])
            .await
            .unwrap();

        let result = database.select("SELECT * FROM t", vec![]).await.unwrap();

        assert!(result.columns.is_empty());
        assert!(result.rows.is_empty());
        database.close().await.unwrap();
    });
}

#[test]
fn execute_reports_rows_affected_and_the_last_insert_id() {
    let directory = TempDirectory::new("execute");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();
        database
            .execute("CREATE TABLE t (id INTEGER PRIMARY KEY, a)", vec![])
            .await
            .unwrap();

        let inserted = database
            .execute(
                "INSERT INTO t (a) VALUES (?), (?)",
                vec![json!(1), json!(2)],
            )
            .await
            .unwrap();
        let updated = database
            .execute("UPDATE t SET a = 0", vec![])
            .await
            .unwrap();

        assert_eq!(inserted.rows_affected, 2);
        assert_eq!(inserted.last_insert_id, 2);
        assert_eq!(updated.rows_affected, 2);
        database.close().await.unwrap();
    });
}

#[test]
fn defaults_apply_without_a_config() {
    let directory = TempDirectory::new("defaults");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();

        assert_eq!(
            scalar(&database, "PRAGMA journal_mode", vec![]).await,
            json!("wal")
        );
        assert_eq!(
            scalar(&database, "PRAGMA foreign_keys", vec![]).await,
            json!(1)
        );
        assert_eq!(
            scalar(&database, "PRAGMA synchronous", vec![]).await,
            json!(1)
        );
        assert_eq!(
            scalar(&database, "PRAGMA busy_timeout", vec![]).await,
            json!(5000)
        );
        database.close().await.unwrap();
    });
}

#[test]
fn pragmas_override_the_defaults() {
    let directory = TempDirectory::new("pragmas");
    run(async {
        let config = DatabaseConfig::new()
            .pragma("busy_timeout", "1234")
            .pragma("synchronous", "OFF");
        let database = directory
            .databases()
            .load("a.sqlite", Some(config))
            .await
            .unwrap();

        assert_eq!(
            scalar(&database, "PRAGMA busy_timeout", vec![]).await,
            json!(1234)
        );
        assert_eq!(
            scalar(&database, "PRAGMA synchronous", vec![]).await,
            json!(0)
        );
        database.close().await.unwrap();
    });
}

#[test]
fn malformed_pragma_names_and_key_pragmas_are_refused() {
    let directory = TempDirectory::new("bad-pragmas");
    run(async {
        let databases = directory.databases();

        let malformed = databases
            .load(
                "a.sqlite",
                Some(DatabaseConfig::new().pragma("x; DROP", "1")),
            )
            .await;
        let key = databases
            .load(
                "b.sqlite",
                Some(DatabaseConfig::new().pragma("KEY", "'secret'")),
            )
            .await;

        assert!(matches!(malformed, Err(Error::InvalidPragma(_))));
        assert!(matches!(key, Err(Error::KeyAsPragma(_))));
    });
}

#[cfg(feature = "encryption")]
#[test]
fn a_key_encrypts_the_file() {
    let directory = TempDirectory::new("encryption");
    run(async {
        let databases = directory.databases();
        let keyed = Some(DatabaseConfig::new().key("it's secret"));

        let database = databases.load("a.sqlite", keyed.clone()).await.unwrap();
        database
            .execute("CREATE TABLE t (a)", vec![])
            .await
            .unwrap();
        databases.close(database.id()).await.unwrap();

        let unkeyed = databases.load("a.sqlite", None).await.unwrap();
        assert!(unkeyed.select("SELECT * FROM t", vec![]).await.is_err());
        databases.close(unkeyed.id()).await.unwrap();

        let rekeyed = databases.load("a.sqlite", keyed).await.unwrap();
        assert!(rekeyed.select("SELECT * FROM t", vec![]).await.is_ok());
        databases.close(rekeyed.id()).await.unwrap();
    });
}

#[cfg(not(feature = "encryption"))]
#[test]
fn a_key_needs_the_encryption_feature() {
    let directory = TempDirectory::new("no-encryption");
    run(async {
        let result = directory
            .databases()
            .load("a.sqlite", Some(DatabaseConfig::new().key("secret")))
            .await;

        assert!(matches!(result, Err(Error::EncryptionDisabled)));
    });
}

#[test]
fn two_spellings_of_one_file_share_a_connection() {
    let directory = TempDirectory::new("identity");
    run(async {
        let databases = directory.databases();

        let relative = databases.load("a.sqlite", None).await.unwrap();
        let absolute = databases
            .load(directory.0.join("a.sqlite"), None)
            .await
            .unwrap();

        assert!(Arc::ptr_eq(&relative, &absolute));
        databases.close(relative.id()).await.unwrap();
    });
}

#[test]
fn loading_an_open_database_compares_configs() {
    let directory = TempDirectory::new("configs");
    run(async {
        let databases = directory.databases();
        let config = DatabaseConfig::new().pragma("busy_timeout", "1000");

        let opened = databases
            .load("a.sqlite", Some(config.clone()))
            .await
            .unwrap();
        let without_config = databases.load("a.sqlite", None).await.unwrap();
        let same_config = databases.load("a.sqlite", Some(config)).await.unwrap();
        let different = databases
            .load(
                "a.sqlite",
                Some(DatabaseConfig::new().pragma("busy_timeout", "2000")),
            )
            .await;

        assert!(Arc::ptr_eq(&opened, &without_config));
        assert!(Arc::ptr_eq(&opened, &same_config));
        assert!(matches!(different, Err(Error::ConfigMismatch(_))));

        databases.close(opened.id()).await.unwrap();
        let reopened = databases
            .load(
                "a.sqlite",
                Some(DatabaseConfig::new().pragma("busy_timeout", "2000")),
            )
            .await
            .unwrap();

        assert_eq!(
            scalar(&reopened, "PRAGMA busy_timeout", vec![]).await,
            json!(2000)
        );
        databases.close(reopened.id()).await.unwrap();
    });
}

#[test]
fn closing_affects_every_holder_and_only_that_file() {
    let directory = TempDirectory::new("close");
    run(async {
        let databases = directory.databases();
        let a = databases.load("a.sqlite", None).await.unwrap();
        let b = databases.load("b.sqlite", None).await.unwrap();
        let id = a.id().to_owned();

        databases.close(&id).await.unwrap();

        assert!(matches!(a.lock().await, Err(Error::Closed(_))));
        assert!(matches!(databases.get(&id).await, Err(Error::NotLoaded(_))));
        assert!(databases.close(&id).await.is_ok());
        assert_eq!(scalar(&b, "SELECT 1", vec![]).await, json!(1));
        databases.close(b.id()).await.unwrap();
    });
}

#[test]
fn a_transaction_spans_calls_and_rolls_back() {
    let directory = TempDirectory::new("transaction");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();
        database
            .execute("CREATE TABLE t (a)", vec![])
            .await
            .unwrap();

        assert!(!database.in_transaction().await.unwrap());
        database.execute("BEGIN", vec![]).await.unwrap();
        assert!(database.in_transaction().await.unwrap());
        database
            .execute("INSERT INTO t VALUES (1)", vec![])
            .await
            .unwrap();
        database.execute("ROLLBACK", vec![]).await.unwrap();
        assert!(!database.in_transaction().await.unwrap());

        assert_eq!(
            scalar(&database, "SELECT count(*) FROM t", vec![]).await,
            json!(0)
        );
        database.close().await.unwrap();
    });
}

#[test]
fn a_leftover_transaction_is_rolled_back() {
    let directory = TempDirectory::new("leftover");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();
        database
            .execute("CREATE TABLE t (a)", vec![])
            .await
            .unwrap();
        database.execute("BEGIN", vec![]).await.unwrap();
        database
            .execute("INSERT INTO t VALUES (1)", vec![])
            .await
            .unwrap();

        assert!(database.roll_back_leftover_transaction().await.unwrap());
        assert!(!database.roll_back_leftover_transaction().await.unwrap());
        assert_eq!(
            scalar(&database, "SELECT count(*) FROM t", vec![]).await,
            json!(0)
        );
        database.close().await.unwrap();
    });
}

#[test]
fn a_relative_path_needs_the_data_directory() {
    run(async {
        let result = Databases::new(None).load("a.sqlite", None).await;

        assert!(matches!(result, Err(Error::NoDataDirectory(_))));
    });
}
