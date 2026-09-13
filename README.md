# tauri-plugin-7otion-sqlite

SQLite for Tauri v2 on one connection per database file, so a transaction's
`BEGIN`, statements and `COMMIT` all reach the connection that began them —
unlike `tauri-plugin-sql`, whose connection pool can split them
([plugins-workspace#886](https://github.com/tauri-apps/plugins-workspace/issues/886)).

## Install

```toml
# src-tauri/Cargo.toml
tauri-plugin-7otion-sqlite = { git = "https://github.com/7otion/tauri-plugin-7otion-sqlite" }
```

```json
"tauri-plugin-7otion-sqlite-api": "github:7otion/tauri-plugin-7otion-sqlite"
```

```rust
tauri::Builder::default().plugin(tauri_plugin_7otion_sqlite::init())
```

Add `"7otion-sqlite:default"` to your capability's permissions.

## JavaScript

```ts
import { Database } from 'tauri-plugin-7otion-sqlite-api';

const db = await Database.load('app.sqlite', { pragmas: { busy_timeout: 10000 } });

await db.execute('INSERT INTO users (name) VALUES (?)', ['Ann']);
const users = await db.select<{ id: number; name: string }>('SELECT * FROM users');
```

A relative path resolves against the app data directory. Every `load` of the
same file shares one connection: not open, it opens with the given options;
open, it attaches when no options or the same options are given, and throws on
different ones — `close()` it and load it again. `close()` closes the connection
for every handle to that file.

Load each database once and share the handle: `load` rolls back a transaction
left open, taking it for one a reloaded page abandoned.

Connections start with `journal_mode = WAL`, `foreign_keys = ON`,
`synchronous = NORMAL` and a 5 second `busy_timeout`; `pragmas` replace them by
name and run in order.

Values bind as `null`, booleans (`0`/`1`), numbers (`INTEGER` when whole, `REAL`
otherwise) and strings; arrays and objects are refused. `BLOB`s come back as byte
arrays. Integers beyond 2^53 lose precision in JavaScript.

## Rust

```rust
use tauri_plugin_7otion_sqlite::SqliteExt;

let db = app.sqlite().load("app.sqlite", None).await?;
let mut connection = db.lock().await?; // &mut sqlx::SqliteConnection
sqlx::query("DELETE FROM sessions").execute(&mut *connection).await?;
```

JavaScript and Rust share each connection, so Rust statements run inside any
transaction JavaScript has open. Statements are logged with `tracing` at debug
level; keys never are.

## Encryption

```toml
tauri-plugin-7otion-sqlite = { git = "…", features = ["encryption"] }
```

Builds SQLCipher instead of SQLite, for the whole app, on CommonCrypto on Apple
platforms and the system OpenSSL elsewhere. On Windows set `OPENSSL_DIR`, or use
`encryption-vendored-openssl`, which compiles OpenSSL and needs a full Perl (such
as Strawberry Perl) and NASM.

```rust
app.sqlite().load("app.sqlite", Some(DatabaseConfig::new().key(key))).await?;
```

A key passed from JavaScript (`{ key }`) works, but it crosses IPC and stays in
webview memory, so both sides log a warning.

## License

MIT
