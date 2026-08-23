import { Database } from "bun:sqlite";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

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
      stderr: "pipe",
      stdout: "pipe",
    });
  } catch {
    return { orphanPath: null, path: fallback };
  }
  if (result.exitCode !== 0) return { orphanPath: null, path: fallback };

  const [checkoutOutput, commonDirectoryOutput] = (result.stdout?.toString() ?? "")
    .trim()
    .split("\n");
  if (!checkoutOutput || !commonDirectoryOutput) return { orphanPath: null, path: fallback };
  const checkoutRoot = resolve(cwd, checkoutOutput);
  const commonRoot = dirname(resolve(cwd, commonDirectoryOutput));
  const path = join(commonRoot, ".maestro", "maestro.db");
  const privatePath = join(checkoutRoot, ".maestro", "maestro.db");
  const orphanPath = privatePath !== path && existsSync(privatePath) ? privatePath : null;
  return { orphanPath, path };
}

export class Store {
  readonly database: Database;

  constructor(path: string) {
    mkdirSync(dirname(path), { recursive: true });
    this.database = new Database(path, { create: true, strict: true });
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
