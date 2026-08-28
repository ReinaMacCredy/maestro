import { Database, SQLiteError } from "bun:sqlite";
import { existsSync, mkdirSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { CliError } from "./cli.ts";

export interface StoreLocation {
  orphanPath: string | null;
  path: string;
  root: string;
}

export function resolveStoreLocation(cwd: string): StoreLocation {
  const resolvedCwd = resolve(cwd);
  const fallback = join(resolvedCwd, ".maestro", "maestro.db");
  const assertOutsideStore = (root: string | null): void => {
    let current = resolvedCwd;
    while (true) {
      if (basename(current) === ".maestro") {
        throw new CliError(
          "STORE_INSIDE_STORE",
          `${resolvedCwd} is inside a .maestro directory; run maestro from the directory that owns it: ${dirname(current)}`,
        );
      }
      if (current === root) return;
      const parent = dirname(current);
      if (parent === current) return;
      current = parent;
    }
  };
  let result: ReturnType<typeof Bun.spawnSync>;
  try {
    result = Bun.spawnSync(["git", "rev-parse", "--show-toplevel", "--git-common-dir"], {
      cwd,
      env: { ...process.env, LC_ALL: "C" },
      stderr: "pipe",
      stdout: "pipe",
    });
  } catch (error) {
    throw new Error(
      `cannot resolve git repository: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (result.exitCode !== 0) {
    const diagnostic = result.stderr?.toString().trim() ?? "";
    if (
      diagnostic.startsWith("fatal: not a git repository (or any of the parent directories):") ||
      diagnostic.startsWith("fatal: not a git repository (or any parent up to mount point ")
    ) {
      assertOutsideStore(null);
      return { orphanPath: null, path: fallback, root: resolvedCwd };
    }
    throw new Error(`cannot resolve git repository: ${diagnostic || `git exited ${result.exitCode}`}`);
  }

  const [checkoutOutput, commonDirectoryOutput] = (result.stdout?.toString() ?? "")
    .trim()
    .split("\n");
  if (!checkoutOutput || !commonDirectoryOutput) {
    throw new Error("cannot resolve git repository: git returned incomplete paths");
  }
  const checkoutRoot = resolve(cwd, checkoutOutput);
  assertOutsideStore(checkoutRoot);
  const commonDirectory = resolve(cwd, commonDirectoryOutput);
  const commonRoot = basename(commonDirectory) === ".git" ? dirname(commonDirectory) : commonDirectory;
  const path = join(commonRoot, ".maestro", "maestro.db");
  const privatePath = join(checkoutRoot, ".maestro", "maestro.db");
  const orphanPath = privatePath !== path && existsSync(privatePath) ? privatePath : null;
  return { orphanPath, path, root: checkoutRoot };
}

function assertSqliteIdentifier(identifier: string): void {
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(identifier)) {
    throw new Error(`invalid SQLite identifier: ${identifier}`);
  }
}

export class Store {
  readonly database: Database;
  readonly ephemeral: boolean;
  readonly readOnly: boolean;

  constructor(readonly path: string, options: { readonly?: boolean } = {}) {
    this.readOnly = options.readonly ?? false;
    this.ephemeral = this.readOnly && !existsSync(path);
    if (!this.readOnly) mkdirSync(dirname(path), { recursive: true });
    this.database = this.openDatabase(path);
    this.database.exec("PRAGMA busy_timeout = 5000");
    this.database.exec("PRAGMA foreign_keys = ON");
    if (this.readOnly) return;
    try {
      this.database.exec("PRAGMA journal_mode = WAL");
    } catch (error) {
      if (!(error instanceof SQLiteError) || !error.code?.startsWith("SQLITE_BUSY")) throw error;
      Bun.sleepSync(100);
      this.database.exec("PRAGMA journal_mode = WAL");
    }
  }

  private openDatabase(path: string): Database {
    if (this.ephemeral) return new Database(":memory:", { create: true, strict: true });
    if (!this.readOnly) return new Database(path, { create: true, strict: true });
    try {
      const readonly = new Database(path, { create: false, readonly: true, strict: true });
      try {
        readonly.query("SELECT count(*) FROM sqlite_master").get();
        return readonly;
      } catch (error) {
        readonly.close();
        throw error;
      }
    } catch (error) {
      // A WAL database is unreadable without its -shm, and a read-only handle
      // cannot create one, so a cleanly closed store would otherwise report
      // itself unreadable and unload every store-backed verb. Reopen writable
      // and let SQLite refuse the writes instead: the sidecars reappear, the
      // content does not change (d706). A store that fails both ways keeps its
      // original diagnosis.
      try {
        const observer = new Database(path, { create: false, strict: true });
        observer.exec("PRAGMA query_only = 1");
        observer.query("SELECT count(*) FROM sqlite_master").get();
        return observer;
      } catch {
        throw error;
      }
    }
  }

  migrate(sql: string): void {
    if (this.readOnly && !this.ephemeral) return;
    this.database.exec(sql);
  }

  ensureColumn(table: string, column: string, migration: string): void {
    if (this.hasColumn(table, column)) return;
    try {
      this.migrate(migration);
    } catch (error) {
      if (!this.hasColumn(table, column)) throw error;
    }
  }

  hasColumn(table: string, column: string): boolean {
    assertSqliteIdentifier(table);
    assertSqliteIdentifier(column);
    return this.database
      .query<{ name: string }, []>(`PRAGMA table_info(${table})`)
      .all()
      .some((entry) => entry.name === column);
  }

  nextPrefixedId(table: string, prefix: string): string {
    assertSqliteIdentifier(table);
    if (!/^[A-Za-z]$/.test(prefix)) {
      throw new Error(`invalid ID prefix: ${prefix}`);
    }
    const next = this.database
      .query<{ next: number }, []>(
        `SELECT COALESCE(MAX(CAST(SUBSTR(id, 2) AS INTEGER)), 0) + 1 AS next FROM ${table}`,
      )
      .get()?.next ?? 1;
    return `${prefix}${next}`;
  }

  close(): void {
    this.database.close();
  }
}
