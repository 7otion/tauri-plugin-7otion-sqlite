//! SQLite for Tauri on one connection per database file, so a transaction's statements all
//! reach the connection that began it.

mod commands;
mod config;
mod database;
mod databases;
mod error;
mod value;

#[cfg(test)]
mod tests;

pub use config::DatabaseConfig;
pub use database::{ConnectionGuard, Database};
pub use databases::Databases;
pub use error::{Error, Result};

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("7otion-sqlite")
        .invoke_handler(tauri::generate_handler![
            commands::load::load,
            commands::close::close,
            commands::select::select,
            commands::execute::execute,
            commands::in_transaction::in_transaction,
        ])
        .setup(|app, _api| {
            app.manage(Databases::new(app.path().app_data_dir().ok()));
            Ok(())
        })
        .build()
}

/// `app.sqlite()`, from anything that can reach the app's managed state.
pub trait SqliteExt<R: Runtime> {
    fn sqlite(&self) -> &Databases;
}

impl<R: Runtime, T: Manager<R>> SqliteExt<R> for T {
    fn sqlite(&self) -> &Databases {
        self.state::<Databases>().inner()
    }
}
