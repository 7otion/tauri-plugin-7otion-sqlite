import { invoke } from '@tauri-apps/api/core';
const PLUGIN = 'plugin:7otion-sqlite';
/** A connection to one database file, shared by every `load` of that file. */
export class Database {
    path;
    constructor(path) {
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
        const resolved = await invoke(`${PLUGIN}|load`, {
            path,
            config: options ? Database.config(options) : null,
        });
        return new Database(resolved);
    }
    /** On a column name shared by two result columns, the later one wins. */
    async select(sql, params = []) {
        const { columns, rows } = await invoke(`${PLUGIN}|select`, {
            db: this.path,
            sql,
            params,
        });
        return rows.map(values => Object.fromEntries(columns.map((column, index) => [column, values[index]])));
    }
    async execute(sql, params = []) {
        return invoke(`${PLUGIN}|execute`, {
            db: this.path,
            sql,
            params,
        });
    }
    /** The database's own state, not a flag kept in JavaScript. */
    async inTransaction() {
        return invoke(`${PLUGIN}|in_transaction`, { db: this.path });
    }
    /** Closes the connection for every handle to this file. */
    async close() {
        await invoke(`${PLUGIN}|close`, { db: this.path });
    }
    static config(options) {
        return {
            key: options.key ?? null,
            pragmas: Object.entries(options.pragmas ?? {}).map(([name, value]) => [name, String(value)]),
        };
    }
}
