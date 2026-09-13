import { invoke } from '@tauri-apps/api/core';
const PLUGIN = 'plugin:7otion-sqlite';
/** One handle to a database file's connection, which every `load` of that file shares. */
export class Database {
    handle;
    path;
    constructor(handle, path) {
        this.handle = handle;
        this.path = path;
    }
    /**
     * Opens `path`, or attaches to its open connection. Without options it attaches whatever
     * config the database was opened with; options must match that exactly.
     */
    static async load(path, options) {
        if (options?.key !== undefined) {
            console.warn('[7otion-sqlite] An encryption key passed from JavaScript crosses IPC and stays in ' +
                'webview memory, so it is not safe. Supply it from Rust with app.sqlite().load() instead.');
        }
        const loaded = await invoke(`${PLUGIN}|load`, {
            path,
            config: options ? Database.config(options) : null,
        });
        return new Database(loaded.handle, loaded.path);
    }
    /** On a column name shared by two result columns, the later one wins. */
    async select(sql, params = []) {
        const { columns, rows } = await invoke(`${PLUGIN}|select`, {
            handle: this.handle,
            sql,
            params,
        });
        return rows.map(values => Object.fromEntries(columns.map((column, index) => [column, values[index]])));
    }
    async execute(sql, params = []) {
        return invoke(`${PLUGIN}|execute`, {
            handle: this.handle,
            sql,
            params,
        });
    }
    /** The database's own state, not a flag kept in JavaScript. */
    async inTransaction() {
        return invoke(`${PLUGIN}|in_transaction`, {
            handle: this.handle,
        });
    }
    /**
     * Releases this handle, rolling back a transaction it began. The connection closes once no
     * handle and no Rust `load` still holds it.
     */
    async close() {
        await invoke(`${PLUGIN}|close`, { handle: this.handle });
    }
    static config(options) {
        return {
            key: options.key ?? null,
            pragmas: Object.entries(options.pragmas ?? {}).map(([name, value]) => [name, String(value)]),
        };
    }
}
