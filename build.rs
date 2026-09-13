const COMMANDS: &[&str] = &["load", "close", "select", "execute", "in_transaction"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
