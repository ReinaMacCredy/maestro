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
import { mapWithConcurrency } from "@/shared/lib/concurrency.js";

// Bounds open file handles when listing a task with many evidence rows.
// macOS default ulimit -n is 256; 32 leaves plenty of headroom and still
// keeps wall-clock close to unbounded Promise.all on warm cache.
const PER_TASK_READ_CONCURRENCY = 32;
const PER_LIST_TASK_CONCURRENCY = 8;
import { ANY_TASK_ID_PATTERN as TASK_ID_PATTERN } from "@/v2/types/task.js";
import type { EvidenceRow } from "../domain/types.js";
import { EVIDENCE_ID_PATTERN } from "../domain/evidence-id.js";
import type {
  EvidenceListFilter,
  EvidenceStorePort,
} from "../ports/storage.js";

const EVIDENCE_DIR = "evidence";
const ACCEPTED_SCHEMA_VERSIONS: ReadonlySet<number> = new Set([1, 2, 3]);

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
    const taskIds = filter.task_id
      ? [filter.task_id]
      : await listTaskDirs(this.dir());

    // Each task's evidence directory is independent — read them in parallel,
    // but cap concurrency so a workspace with hundreds of tasks does not
    // exhaust the per-process file-handle budget (EMFILE).
    const validTaskIds = taskIds.filter((taskId) => TASK_ID_PATTERN.test(taskId));
    const perTaskRows = await mapWithConcurrency(
      validTaskIds,
      PER_LIST_TASK_CONCURRENCY,
      (taskId) => readTaskDir(this.taskDir(taskId), taskId),
    );

    const rows: EvidenceRow[] = [];
    for (const taskRows of perTaskRows) {
      for (const row of taskRows) {
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
  // Each row is its own JSON file, so reads are I/O-independent. Parallel
  // reads cut wall-clock for tasks with many evidence rows by roughly the
  // file count, but the parallelism is capped so a task with thousands of
  // rows does not blow past the process file-descriptor budget (EMFILE).
  const candidates = entries.flatMap((entry) => {
    if (!entry.isFile() || !entry.name.endsWith(".json")) return [];
    const evidenceId = entry.name.slice(0, -".json".length);
    if (!EVIDENCE_ID_PATTERN.test(evidenceId)) return [];
    return [{ evidenceId, path: join(taskDir, entry.name) }];
  });
  const rows = await mapWithConcurrency(
    candidates,
    PER_TASK_READ_CONCURRENCY,
    async ({ evidenceId, path }): Promise<EvidenceRow | undefined> => {
      const row = await tryReadRow(path);
      if (row && row.id === evidenceId && row.task_id === taskId) {
        return row;
      }
      return undefined;
    },
  );
  return rows.filter((row): row is EvidenceRow => row !== undefined);
}

async function tryReadRow(path: string): Promise<EvidenceRow | undefined> {
  let raw: unknown;
  try {
    raw = await readJson<unknown>(path);
  } catch {
    return undefined;
  }
  return coerceEvidenceRow(raw);
}

/**
 * Validates and coerces a raw JSON value to an EvidenceRow.
 *
 * - v1 rows missing `witness_level` are synthesized to "agent-claimed-locally".
 * - v2 and v3 rows must carry `witness_level`; rows without it are rejected.
 */
function coerceEvidenceRow(value: unknown): EvidenceRow | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const v = value as Record<string, unknown>;

  if (
    typeof v["schema_version"] !== "number"
    || !ACCEPTED_SCHEMA_VERSIONS.has(v["schema_version"] as number)
    || typeof v["id"] !== "string"
    || typeof v["task_id"] !== "string"
    || typeof v["kind"] !== "string"
    || typeof v["created_at"] !== "string"
    || typeof v["payload"] !== "object"
    || v["payload"] === null
  ) {
    return undefined;
  }

  const version = v["schema_version"] as number;

  if (typeof v["witness_level"] === "string") {
    return value as EvidenceRow;
  }

  // v1 rows may pre-date witness_level — synthesize a safe default
  if (version === 1) {
    return { ...(value as object), witness_level: "agent-claimed-locally" } as EvidenceRow;
  }

  // v2 and v3 rows must carry witness_level
  return undefined;
}
