import { Database, SQLiteError } from "bun:sqlite";
import { existsSync, mkdirSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

export interface StoreLocation {
  orphanPath: string | null;
  path: string;
  root: string;
}

export function resolveStoreLocation(cwd: string): StoreLocation {
  const fallback = join(cwd, ".maestro", "maestro.db");
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
      return { orphanPath: null, path: fallback, root: resolve(cwd) };
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
  const commonDirectory = resolve(cwd, commonDirectoryOutput);
  const commonRoot = basename(commonDirectory) === ".git" ? dirname(commonDirectory) : commonDirectory;
  const path = join(commonRoot, ".maestro", "maestro.db");
  const privatePath = join(checkoutRoot, ".maestro", "maestro.db");
  const orphanPath = privatePath !== path && existsSync(privatePath) ? privatePath : null;
  return { orphanPath, path, root: checkoutRoot };
}

export class Store {
  readonly database: Database;
  readonly ephemeral: boolean;
  readonly readOnly: boolean;

  constructor(readonly path: string, options: { readonly?: boolean } = {}) {
    this.readOnly = options.readonly ?? false;
    this.ephemeral = this.readOnly && !existsSync(path);
    if (!this.readOnly) mkdirSync(dirname(path), { recursive: true });
    this.database = new Database(this.ephemeral ? ":memory:" : path, {
      create: !this.readOnly || this.ephemeral,
      readonly: this.readOnly && !this.ephemeral,
      strict: true,
    });
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

  migrate(sql: string): void {
    if (this.readOnly && !this.ephemeral) return;
    this.database.exec(sql);
  }

  nextPrefixedId(table: string, prefix: string): string {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(table)) {
      throw new Error(`invalid SQLite identifier: ${table}`);
    }
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
