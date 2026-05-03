/**
 * Filesystem-backed evidence store.
 *
 * Layout: `.maestro/evidence/<task-id>/<evidence-id>.json` -- one file per row.
 *
 * Writes are atomic via `writeJson` (write-tmp-then-rename). Reads are
 * tolerant: malformed JSON, unknown extensions, nested directories, and
 * rows with a non-current `schema_version` are silently skipped so a
 * single bad file does not poison `list()` or `read()`.
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import { TASK_ID_PATTERN } from "@/features/task";
import type { EvidenceRow } from "../domain/types.js";
import { EVIDENCE_ID_PATTERN } from "../domain/evidence-id.js";
import type {
  EvidenceListFilter,
  EvidenceStorePort,
} from "../ports/storage.js";

const EVIDENCE_DIR = "evidence";
const CURRENT_SCHEMA_VERSION = 2;
const ACCEPTED_SCHEMA_VERSIONS: ReadonlySet<number> = new Set([1, 2]);

export class FsEvidenceStoreAdapter implements EvidenceStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, EVIDENCE_DIR);
  }

  private taskDir(taskId: string): string {
    assertSafeSegment(taskId, "task ID", TASK_ID_PATTERN, "tsk- followed by 6 hex characters");
    return resolveWithin(this.dir(), taskId, "Evidence task directory");
  }

  private rowPath(taskId: string, evidenceId: string): string {
    assertSafeSegment(evidenceId, "evidence ID", EVIDENCE_ID_PATTERN, "evd- followed by a 13-digit timestamp and 6 hex characters");
    return resolveWithin(this.taskDir(taskId), `${evidenceId}.json`, "Evidence row path");
  }

  async append(row: EvidenceRow): Promise<void> {
    const taskDir = this.taskDir(row.task_id);
    const rowPath = this.rowPath(row.task_id, row.id);
    await ensureDir(taskDir);
    await writeJson(rowPath, row);
  }

  async read(id: string): Promise<EvidenceRow | undefined> {
    assertSafeSegment(id, "evidence ID", EVIDENCE_ID_PATTERN, "evd- followed by a 13-digit timestamp and 6 hex characters");
    for (const taskId of await listTaskDirs(this.dir())) {
      const candidate = await tryReadRow(this.rowPath(taskId, id));
      if (candidate && candidate.id === id && candidate.task_id === taskId) {
        return candidate;
      }
    }
    return undefined;
  }

  async list(filter: EvidenceListFilter = {}): Promise<readonly EvidenceRow[]> {
    const rows: EvidenceRow[] = [];
    const taskIds = filter.task_id
      ? [filter.task_id]
      : await listTaskDirs(this.dir());

    for (const taskId of taskIds) {
      if (!TASK_ID_PATTERN.test(taskId)) continue;
      for (const row of await readTaskDir(this.taskDir(taskId), taskId)) {
        if (filter.session_id !== undefined && row.session_id !== filter.session_id) continue;
        if (filter.kind !== undefined && row.kind !== filter.kind) continue;
        rows.push(row);
      }
    }
    return rows.sort((a, b) => a.created_at.localeCompare(b.created_at));
  }
}

async function listTaskDirs(dir: string): Promise<readonly string[]> {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries
      .filter((entry) => entry.isDirectory() && TASK_ID_PATTERN.test(entry.name))
      .map((entry) => entry.name);
  } catch {
    return [];
  }
}

async function readTaskDir(taskDir: string, taskId: string): Promise<readonly EvidenceRow[]> {
  let entries;
  try {
    entries = await readdir(taskDir, { withFileTypes: true });
  } catch {
    return [];
  }
  const rows: EvidenceRow[] = [];
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.endsWith(".json")) continue;
    const evidenceId = entry.name.slice(0, -".json".length);
    if (!EVIDENCE_ID_PATTERN.test(evidenceId)) continue;
    const row = await tryReadRow(join(taskDir, entry.name));
    if (row && row.id === evidenceId && row.task_id === taskId) {
      rows.push(row);
    }
  }
  return rows;
}

async function tryReadRow(path: string): Promise<EvidenceRow | undefined> {
  let raw: unknown;
  try {
    raw = await readJson<unknown>(path);
  } catch {
    return undefined;
  }
  if (!isEvidenceRow(raw)) return undefined;
  return raw;
}

function isEvidenceRow(value: unknown): value is EvidenceRow {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v["schema_version"] === "number"
    && ACCEPTED_SCHEMA_VERSIONS.has(v["schema_version"] as number)
    && typeof v["id"] === "string"
    && typeof v["task_id"] === "string"
    && typeof v["kind"] === "string"
    && typeof v["witness_level"] === "string"
    && typeof v["created_at"] === "string"
    && typeof v["payload"] === "object"
    && v["payload"] !== null
  );
}
