import { chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve } from "node:path";

export const sourceRecordFile = "source.json";

export interface SourceRecord {
  path: string;
}

export type SourceRecordRead =
  | { status: "invalid" }
  | { status: "missing" }
  | { record: SourceRecord; status: "valid" };

export function sourceRecordPath(home: string): string {
  return join(home, ".maestro", sourceRecordFile);
}

export async function readSourceRecord(home: string): Promise<SourceRecordRead> {
  let text: string;
  try {
    text = await readFile(sourceRecordPath(home), "utf8");
  } catch (error) {
    if ((error as { code?: unknown }).code === "ENOENT") return { status: "missing" };
    throw error;
  }
  try {
    const value = JSON.parse(text) as Partial<SourceRecord>;
    return typeof value.path === "string" && isAbsolute(value.path)
      ? { status: "valid", record: { path: resolve(value.path) } }
      : { status: "invalid" };
  } catch {
    return { status: "invalid" };
  }
}

export async function writeSourceRecord(home: string, sourceRoot: string): Promise<void> {
  const path = sourceRecordPath(home);
  await mkdir(dirname(path), { recursive: true });
  await chmod(dirname(path), 0o700);
  await writeFile(path, `${JSON.stringify({ path: resolve(sourceRoot) })}\n`);
  await chmod(path, 0o600);
}
