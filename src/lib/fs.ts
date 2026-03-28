import { mkdir, readdir, rename, rm, stat } from "node:fs/promises";
import { join } from "node:path";

export async function ensureDir(dir: string): Promise<void> {
  await mkdir(dir, { recursive: true });
}

export async function readText(path: string): Promise<string | undefined> {
  try {
    return await Bun.file(path).text();
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw err;
  }
}

export async function readJson<T>(path: string): Promise<T | undefined> {
  try {
    return await Bun.file(path).json() as T;
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw err;
  }
}

export async function writeJson(path: string, data: unknown): Promise<void> {
  await writeAtomic(path, JSON.stringify(data, null, 2) + "\n");
}

export async function writeText(path: string, content: string): Promise<void> {
  await writeAtomic(path, content);
}

async function writeAtomic(path: string, content: string): Promise<void> {
  const tmp = `${path}.tmp.${Date.now()}`;
  await Bun.write(tmp, content);
  await rename(tmp, path);
}

export async function removeIfExists(
  path: string,
  opts?: { recursive?: boolean },
): Promise<boolean> {
  try {
    await rm(path, opts);
    return true;
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw err;
  }
}

export async function dirExists(dir: string): Promise<boolean> {
  try {
    return (await stat(dir)).isDirectory();
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw err;
  }
}

export async function listDirs(dir: string): Promise<string[]> {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter((e) => e.isDirectory()).map((e) => join(dir, e.name));
  } catch {
    return [];
  }
}
