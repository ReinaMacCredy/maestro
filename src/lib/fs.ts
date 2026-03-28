import { mkdir, readdir } from "node:fs/promises";
import { join } from "node:path";

/** Ensure a directory exists, creating it recursively if needed. */
export async function ensureDir(dir: string): Promise<void> {
  await mkdir(dir, { recursive: true });
}

/** Read and parse a JSON file. Returns undefined if file doesn't exist. */
export async function readJson<T>(path: string): Promise<T | undefined> {
  const file = Bun.file(path);
  if (!(await file.exists())) return undefined;
  return file.json() as Promise<T>;
}

/** Write JSON to a file atomically (write to tmp, rename). */
export async function writeJson(path: string, data: unknown): Promise<void> {
  const tmp = `${path}.tmp.${Date.now()}`;
  await Bun.write(tmp, JSON.stringify(data, null, 2) + "\n");
  const { rename } = await import("node:fs/promises");
  await rename(tmp, path);
}

/** Write text to a file atomically. */
export async function writeText(path: string, content: string): Promise<void> {
  const tmp = `${path}.tmp.${Date.now()}`;
  await Bun.write(tmp, content);
  const { rename } = await import("node:fs/promises");
  await rename(tmp, path);
}

/** List subdirectory names in a directory. Returns empty array if dir doesn't exist. */
export async function listDirs(dir: string): Promise<string[]> {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter((e) => e.isDirectory()).map((e) => join(dir, e.name));
  } catch {
    return [];
  }
}
