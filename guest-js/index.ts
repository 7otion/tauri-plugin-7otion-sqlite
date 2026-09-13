import { invoke } from '@tauri-apps/api/core';

const PLUGIN = 'plugin:7otion-sqlite';

export type BindValue = null | boolean | number | string;

export type Row = Record<string, unknown>;

export interface LoadOptions {
	/** SQLCipher key; needs the plugin's `encryption` feature. */
	key?: string;
	/** Run in order after the plugin's defaults; one naming a default replaces it. */
	pragmas?: Record<string, string | number | boolean>;
}

export interface ExecuteResult {
	rowsAffected: number;
	lastInsertId: number;
}

interface Rows {
	columns: string[];
	rows: unknown[][];
}

interface Loaded {
	handle: number;
	path: string;
}

/** One handle to a database file's connection, which every `load` of that file shares. */
export class Database {
	private constructor(
		private readonly handle: number,
		readonly path: string,
	) {}

	/**
	 * Opens `path`, or attaches to its open connection. Without options it attaches whatever
	 * config the database was opened with; options must match that exactly.
	 */
	static async load(path: string, options?: LoadOptions): Promise<Database> {
		if (options?.key !== undefined) {
			console.warn(
				'[7otion-sqlite] An encryption key passed from JavaScript crosses IPC and stays in ' +
					'webview memory, so it is not safe. Supply it from Rust with app.sqlite().load() instead.',
			);
		}

		const loaded = await invoke<Loaded>(`${PLUGIN}|load`, {
			path,
			config: options ? Database.config(options) : null,
		});

		return new Database(loaded.handle, loaded.path);
	}

	/** On a column name shared by two result columns, the later one wins. */
	async select<T extends Row = Row>(
		sql: string,
		params: BindValue[] = [],
	): Promise<T[]> {
		const { columns, rows } = await invoke<Rows>(`${PLUGIN}|select`, {
			handle: this.handle,
			sql,
			params,
		});

		return rows.map(
			values =>
				Object.fromEntries(
					columns.map((column, index) => [column, values[index]]),
				) as T,
		);
	}

	async execute(
		sql: string,
		params: BindValue[] = [],
	): Promise<ExecuteResult> {
		return invoke<ExecuteResult>(`${PLUGIN}|execute`, {
			handle: this.handle,
			sql,
			params,
		});
	}

	/** The database's own state, not a flag kept in JavaScript. */
	async inTransaction(): Promise<boolean> {
		return invoke<boolean>(`${PLUGIN}|in_transaction`, {
			handle: this.handle,
		});
	}

	/**
	 * Releases this handle, rolling back a transaction it began. The connection closes once no
	 * handle and no Rust `load` still holds it.
	 */
	async close(): Promise<void> {
		await invoke(`${PLUGIN}|close`, { handle: this.handle });
	}

	private static config(options: LoadOptions) {
		return {
			key: options.key ?? null,
			pragmas: Object.entries(options.pragmas ?? {}).map(
				([name, value]) => [name, String(value)],
			),
		};
	}
}
