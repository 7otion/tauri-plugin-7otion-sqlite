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
/** One handle to a database file's connection, which every `load` of that file shares. */
export declare class Database {
    private readonly handle;
    readonly path: string;
    private constructor();
    /**
     * Opens `path`, or attaches to its open connection. Without options it attaches whatever
     * config the database was opened with; options must match that exactly.
     */
    static load(path: string, options?: LoadOptions): Promise<Database>;
    /** On a column name shared by two result columns, the later one wins. */
    select<T extends Row = Row>(sql: string, params?: BindValue[]): Promise<T[]>;
    execute(sql: string, params?: BindValue[]): Promise<ExecuteResult>;
    /** The database's own state, not a flag kept in JavaScript. */
    inTransaction(): Promise<boolean>;
    /**
     * Releases this handle, rolling back a transaction it began. The connection closes once no
     * handle and no Rust `load` still holds it.
     */
    close(): Promise<void>;
    private static config;
}
