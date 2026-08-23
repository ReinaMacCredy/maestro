import { Database } from "bun:sqlite";
import { existsSync, mkdirSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

export interface StoreLocation {
  orphanPath: string | null;
  path: string;
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
      return { orphanPath: null, path: fallback };
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
  return { orphanPath, path };
}

export class Store {
  readonly database: Database;

  constructor(readonly path: string) {
    mkdirSync(dirname(path), { recursive: true });
    this.database = new Database(path, { create: true, strict: true });
    this.database.exec("PRAGMA busy_timeout = 5000");
    this.database.exec("PRAGMA foreign_keys = ON");
    this.database.exec("PRAGMA journal_mode = WAL");
  }

  migrate(sql: string): void {
    this.database.exec(sql);
  }

  close(): void {
    this.database.close();
  }
}
