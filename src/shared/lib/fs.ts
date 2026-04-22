import { appendFile, mkdir, readdir, rename, rm, stat } from "node:fs/promises";
import { randomUUID } from "node:crypto";
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

/**
 * Read text from a file, or from stdin when `path === "-"`. Returns undefined
 * only for a missing file path; stdin never resolves to undefined (it resolves
 * to an empty string on EOF with no input).
 */
export async function readTextOrStdin(path: string): Promise<string | undefined> {
  if (path === "-") {
    return new Response(Bun.stdin).text();
  }
  return readText(path);
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

export async function appendText(path: string, content: string): Promise<void> {
  await appendFile(path, content, "utf8");
}

async function writeAtomic(path: string, content: string): Promise<void> {
  // randomUUID gives per-write uniqueness even when two callers race within the
  // same millisecond; a colliding tmp path could otherwise be overwritten and
  // the loser's rename would read partial content.
  const tmp = `${path}.tmp.${randomUUID()}`;
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

export async function fileExists(path: string): Promise<boolean> {
  return Bun.file(path).exists();
}

interface RenameForInPlaceReplaceOptions {
  readonly removeImpl?: typeof rm;
  readonly renameImpl?: typeof rename;
}

/**
 * Rename `target` to `target`.old before a caller writes the new file.
 * Windows cannot overwrite a running executable in place, but renaming
 * a running exe on the same volume is allowed, so the new binary can
 * be written to `target` and the .old copy cleaned up on next startup.
 */
export async function renameForInPlaceReplace(
  target: string,
  options: RenameForInPlaceReplaceOptions = {},
): Promise<void> {
  const removeImpl = options.removeImpl ?? rm;
  const renameImpl = options.renameImpl ?? rename;
  const oldPath = `${target}.old`;
  try {
    await removeImpl(oldPath, { force: true });
  } catch (err: unknown) {
    // On Windows, a prior `.old` can still be locked by the previous maestro
    // process (antivirus, slow handle release). `force: true` only swallows
    // ENOENT, not EBUSY/EPERM/EACCES. Tolerate those so the rename below can
    // still proceed; if it genuinely cannot, that path will fail loudly.
    const code = (err as NodeJS.ErrnoException).code;
    if (code !== "EBUSY" && code !== "EPERM" && code !== "EACCES") throw err;
  }
  try {
    await renameImpl(target, oldPath);
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return;
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
