use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value as JsonValue, json};

use crate::database::Database;
use crate::databases::HandleId;
use crate::{DatabaseConfig, Databases, Error};

/// Statements issued from Rust, which no handle owns.
const FROM_RUST: HandleId = 0;

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

async fn exec(database: &Database, handle: HandleId, sql: &str) {
    database.execute(handle, sql, vec![]).await.unwrap();
}

#[test]
fn values_round_trip() {
    let directory = TempDirectory::new("values");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();
        exec(&database, FROM_RUST, "CREATE TABLE t (a, b, c, d, e)").await;
        database
            .execute(
                FROM_RUST,
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
        exec(&database, FROM_RUST, "CREATE TABLE t (a)").await;

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
        exec(
            &database,
            FROM_RUST,
            "CREATE TABLE t (id INTEGER PRIMARY KEY, a)",
        )
        .await;

        let inserted = database
            .execute(
                FROM_RUST,
                "INSERT INTO t (a) VALUES (?), (?)",
                vec![json!(1), json!(2)],
            )
            .await
            .unwrap();
        let updated = database
            .execute(FROM_RUST, "UPDATE t SET a = 0", vec![])
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
        exec(&database, FROM_RUST, "CREATE TABLE t (a)").await;
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
fn a_transaction_spans_calls_and_rolls_back() {
    let directory = TempDirectory::new("transaction");
    run(async {
        let database = directory.databases().load("a.sqlite", None).await.unwrap();
        exec(&database, FROM_RUST, "CREATE TABLE t (a)").await;

        assert!(!database.in_transaction().await.unwrap());
        exec(&database, FROM_RUST, "BEGIN").await;
        assert!(database.in_transaction().await.unwrap());
        exec(&database, FROM_RUST, "INSERT INTO t VALUES (1)").await;
        exec(&database, FROM_RUST, "ROLLBACK").await;
        assert!(!database.in_transaction().await.unwrap());

        assert_eq!(
            scalar(&database, "SELECT count(*) FROM t", vec![]).await,
            json!(0)
        );
        database.close().await.unwrap();
    });
}

#[test]
fn handles_share_the_connection_until_the_last_is_released() {
    let directory = TempDirectory::new("handles");
    run(async {
        let databases = directory.databases();
        let (first, database) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();
        let (second, same) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();

        assert!(Arc::ptr_eq(&database, &same));
        assert_ne!(first, second);

        databases.release(first).await.unwrap();
        assert!(databases.handle(first).await.is_err());
        assert!(databases.handle(second).await.is_ok());
        assert!(database.lock().await.is_ok());

        databases.release(second).await.unwrap();
        databases.release(second).await.unwrap();
        assert!(matches!(
            databases.handle(second).await,
            Err(Error::UnknownHandle(_))
        ));
        assert!(matches!(database.lock().await, Err(Error::Closed(_))));
    });
}

#[test]
fn a_database_loaded_from_rust_outlives_its_handles() {
    let directory = TempDirectory::new("rust-held");
    run(async {
        let databases = directory.databases();
        let database = databases.load("a.sqlite", None).await.unwrap();
        let (handle, _) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();

        databases.release(handle).await.unwrap();

        assert!(database.lock().await.is_ok());
        databases.close(database.id()).await.unwrap();
    });
}

#[test]
fn closing_from_rust_ends_every_handle_and_only_that_file() {
    let directory = TempDirectory::new("close");
    run(async {
        let databases = directory.databases();
        let a = databases.load("a.sqlite", None).await.unwrap();
        let (handle, _) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();
        let b = databases.load("b.sqlite", None).await.unwrap();

        databases.close(a.id()).await.unwrap();

        assert!(matches!(a.lock().await, Err(Error::Closed(_))));
        assert!(matches!(
            databases.handle(handle).await,
            Err(Error::UnknownHandle(_))
        ));
        assert!(databases.close(a.id()).await.is_ok());
        assert_eq!(scalar(&b, "SELECT 1", vec![]).await, json!(1));
        databases.close(b.id()).await.unwrap();
    });
}

#[test]
fn releasing_a_webview_releases_only_its_handles() {
    let directory = TempDirectory::new("webview");
    run(async {
        let databases = directory.databases();
        let (main, _) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();
        let (other, database) = databases
            .load_handle("a.sqlite", None, "other")
            .await
            .unwrap();

        databases.release_webview("main").await.unwrap();

        assert!(databases.handle(main).await.is_err());
        assert!(databases.handle(other).await.is_ok());
        databases.close(database.id()).await.unwrap();
    });
}

#[test]
fn a_released_handle_rolls_back_only_the_transaction_it_began() {
    let directory = TempDirectory::new("owner");
    run(async {
        let databases = directory.databases();
        let database = databases.load("a.sqlite", None).await.unwrap();
        let (owner, _) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();
        let (bystander, _) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();
        exec(&database, owner, "CREATE TABLE t (a)").await;
        exec(&database, owner, "BEGIN").await;
        exec(&database, bystander, "INSERT INTO t VALUES (1)").await;

        databases.release(bystander).await.unwrap();
        assert!(database.in_transaction().await.unwrap());

        databases.release(owner).await.unwrap();
        assert!(!database.in_transaction().await.unwrap());
        assert_eq!(
            scalar(&database, "SELECT count(*) FROM t", vec![]).await,
            json!(0)
        );
        databases.close(database.id()).await.unwrap();
    });
}

#[test]
fn loading_leaves_an_open_transaction_alone() {
    let directory = TempDirectory::new("load-in-transaction");
    run(async {
        let databases = directory.databases();
        let (owner, database) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();
        exec(&database, owner, "BEGIN").await;

        let (_, _) = databases
            .load_handle("a.sqlite", None, "main")
            .await
            .unwrap();

        assert!(database.in_transaction().await.unwrap());
        databases.close(database.id()).await.unwrap();
    });
}

#[test]
fn a_relative_path_needs_the_data_directory() {
    run(async {
        let result = Databases::new(None).load("a.sqlite", None).await;

        assert!(matches!(result, Err(Error::NoDataDirectory(_))));
    });
}
