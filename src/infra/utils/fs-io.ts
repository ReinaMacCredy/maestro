/**
 * File I/O utilities for maestroCLI.
 * Extracted from paths.ts -- atomic writes, JSON helpers, directory ops.
 */

import * as path from 'path';
import * as fs from 'fs';

/** Maximum path length (conservative cross-platform limit). */
export const MAX_PATH_LENGTH = 240;

export function ensureDir(dirPath: string): void {
  if (dirPath.length > MAX_PATH_LENGTH) {
    throw new Error(
      `Path exceeds maximum length (${dirPath.length} > ${MAX_PATH_LENGTH}): ` +
      `shorten feature/task names or move project to a shorter base path.`
    );
  }
  fs.mkdirSync(dirPath, { recursive: true });
}

export function fileExists(filePath: string): boolean {
  return fs.existsSync(filePath);
}

export function readJson<T>(filePath: string): T | null {
  try {
    const content = fs.readFileSync(filePath, 'utf-8');
    return JSON.parse(content) as T;
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw e;
  }
}

export function writeAtomic(filePath: string, content: string): void {
  ensureDir(path.dirname(filePath));
  const tempPath = `${filePath}.tmp.${process.pid}.${Date.now()}`;

  try {
    fs.writeFileSync(tempPath, content);
    fs.renameSync(tempPath, filePath);
  } catch (error) {
    try {
      fs.unlinkSync(tempPath);
    } catch {
      // Temp file cleanup failed -- may be left behind on disk-full
    }
    const errno = (error as NodeJS.ErrnoException).code;
    if (errno === 'ENOSPC') {
      throw new Error(`Disk full: cannot write ${filePath}. Free disk space and retry.`);
    }
    throw error;
  }
}

export function writeJsonAtomic<T>(filePath: string, data: T): void {
  writeAtomic(filePath, JSON.stringify(data, null, 2));
}

export function readText(filePath: string): string | null {
  try {
    return fs.readFileSync(filePath, 'utf-8');
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw e;
  }
}

export function writeText(filePath: string, content: string): void {
  writeAtomic(filePath, content);
}

export function deepMerge<T extends Record<string, unknown>>(
  target: T,
  patch: Partial<T>
): T {
  const result = { ...target };

  for (const key of Object.keys(patch) as Array<keyof T>) {
    const patchValue = patch[key];

    if (patchValue === undefined) {
      continue;
    }

    if (
      patchValue !== null &&
      typeof patchValue === 'object' &&
      !Array.isArray(patchValue) &&
      result[key] !== null &&
      typeof result[key] === 'object' &&
      !Array.isArray(result[key])
    ) {
      result[key] = deepMerge(
        result[key] as Record<string, unknown>,
        patchValue as Record<string, unknown>
      ) as T[keyof T];
    } else {
      result[key] = patchValue as T[keyof T];
    }
  }

  return result;
}
