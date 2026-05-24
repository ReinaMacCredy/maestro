/**
 * Filesystem-backed verdict store.
 *
 * Layout: `.maestro/verdicts/<task-id>/<verdict-id>.json` — one file per verdict.
 *
 * Writes are atomic via `writeJson` (write-tmp-then-rename). Reads are
 * tolerant: malformed JSON, unknown extensions, and nested directories are
 * silently skipped so a single bad file does not poison `history()` or
 * `readLatest()`.
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import { ANY_TASK_ID_PATTERN as TASK_ID_PATTERN } from "@/types/task.js";
import { VERDICT_ID_PATTERN } from "../domain/verdict-id.js";
import type { Verdict } from "../domain/types.js";
import type { ReadLatestWithCorruptionResult, VerdictStorePort } from "../ports/storage.js";

interface TaskVerdictScan {
  readonly verdicts: Verdict[];
  readonly corruptCount: number;
}

const VERDICTS_DIR = "verdicts";

export class FsVerdictStoreAdapter implements VerdictStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, VERDICTS_DIR);
  }

  private taskDir(taskId: string): string {
    assertSafeSegment(taskId, "task ID", TASK_ID_PATTERN, "tsk- followed by 6 hex characters");
    return resolveWithin(this.dir(), taskId, "Verdict task directory");
  }

  private verdictPath(taskId: string, verdictId: string): string {
    assertSafeSegment(verdictId, "verdict ID", VERDICT_ID_PATTERN, "vrd- followed by a 13-digit timestamp and 6 hex characters");
    return resolveWithin(this.taskDir(taskId), `${verdictId}.json`, "Verdict path");
  }

  async write(taskId: string, verdict: Verdict): Promise<void> {
    const taskDir = this.taskDir(taskId);
    const path = this.verdictPath(taskId, verdict.id);
    await ensureDir(taskDir);
    await writeJson(path, verdict);
  }

  async readLatest(taskId: string): Promise<Verdict | undefined> {
    const verdicts = await this.history(taskId);
    return verdicts.length > 0 ? verdicts[verdicts.length - 1] : undefined;
  }

  async readLatestWithCorruption(taskId: string): Promise<ReadLatestWithCorruptionResult> {
    assertSafeSegment(taskId, "task ID", TASK_ID_PATTERN, "tsk- followed by 6 hex characters");
    const scan = await this.scanTaskVerdicts(this.taskDir(taskId));
    const sorted = [...scan.verdicts].sort((a, b) => a.computedAt.localeCompare(b.computedAt));
    return {
      verdict: sorted.length > 0 ? sorted[sorted.length - 1] : undefined,
      corruptCount: scan.corruptCount,
    };
  }

  async readVersion(taskId: string, verdictId: string): Promise<Verdict | undefined> {
    assertSafeSegment(taskId, "task ID", TASK_ID_PATTERN, "tsk- followed by 6 hex characters");
    assertSafeSegment(verdictId, "verdict ID", VERDICT_ID_PATTERN, "vrd- followed by a 13-digit timestamp and 6 hex characters");
    const path = this.verdictPath(taskId, verdictId);
    const raw = await readJson<unknown>(path);
    if (raw === undefined) return undefined;
    return coerceVerdict(raw);
  }

  async history(taskId: string): Promise<readonly Verdict[]> {
    assertSafeSegment(taskId, "task ID", TASK_ID_PATTERN, "tsk- followed by 6 hex characters");
    const scan = await this.scanTaskVerdicts(this.taskDir(taskId));
    return scan.verdicts.sort((a, b) => a.computedAt.localeCompare(b.computedAt));
  }

  async findByTreeSha(treeSha: string): Promise<readonly Verdict[]> {
    const baseDir = this.dir();
    let taskDirs;
    try {
      taskDirs = await readdir(baseDir, { withFileTypes: true });
    } catch {
      return [];
    }

    // Each task dir is independent; read them in parallel.
    const perTask = await Promise.all(
      taskDirs
        .filter((entry) => entry.isDirectory() && TASK_ID_PATTERN.test(entry.name))
        .map((entry) => this.scanTaskVerdicts(join(baseDir, entry.name))),
    );

    const matches: Verdict[] = [];
    for (const { verdicts } of perTask) {
      for (const verdict of verdicts) {
        if (verdict.subject?.tree_sha === treeSha) matches.push(verdict);
      }
    }

    return matches.sort((a, b) => a.computedAt.localeCompare(b.computedAt));
  }

  // Scans a task directory and returns parsed verdicts alongside the count
  // of files that looked like verdict records (matched VERDICT_ID_PATTERN)
  // but could not be parsed. Callers that need corruption visibility
  // (status/doctor) read the `corruptCount`; callers that only need data
  // (`history`, `findByTreeSha`) discard it.
  private async scanTaskVerdicts(taskDir: string): Promise<TaskVerdictScan> {
    let entries;
    try {
      entries = await readdir(taskDir, { withFileTypes: true });
    } catch {
      return { verdicts: [], corruptCount: 0 };
    }

    // One file per verdict; reads are I/O-independent.
    const candidates = entries.filter((entry) => {
      if (!entry.isFile() || !entry.name.endsWith(".json")) return false;
      const verdictId = entry.name.slice(0, -".json".length);
      return VERDICT_ID_PATTERN.test(verdictId);
    });
    const settled = await Promise.all(
      candidates.map(async (entry): Promise<Verdict | undefined> => {
        try {
          const raw = await readJson<unknown>(join(taskDir, entry.name));
          if (raw === undefined) return undefined;
          return coerceVerdict(raw);
        } catch {
          return undefined;
        }
      }),
    );
    const verdicts: Verdict[] = [];
    let corruptCount = 0;
    for (const v of settled) {
      if (v === undefined) corruptCount += 1;
      else verdicts.push(v);
    }
    return { verdicts, corruptCount };
  }
}

function coerceVerdict(value: unknown): Verdict | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const v = value as Record<string, unknown>;

  if (
    v["schemaVersion"] !== 1
    || typeof v["id"] !== "string"
    || typeof v["taskId"] !== "string"
    || typeof v["contractVersion"] !== "number"
    || typeof v["computedAt"] !== "string"
    || typeof v["decision"] !== "string"
    || typeof v["effectiveRiskClass"] !== "string"
    || !Array.isArray(v["reasons"])
    || !Array.isArray(v["evidenceConsulted"])
    || !Array.isArray(v["policiesConsulted"])
    || typeof v["trustVerifier"] !== "object"
    || v["trustVerifier"] === null
  ) {
    return undefined;
  }

  // subject is optional; if present it must be an object with a string tree_sha
  // so findByTreeSha's `=== treeSha` comparison can never silently match a
  // malformed record.
  const subject = v["subject"];
  if (subject !== undefined) {
    if (
      typeof subject !== "object"
      || subject === null
      || typeof (subject as Record<string, unknown>).tree_sha !== "string"
    ) {
      return undefined;
    }
  }

  return value as Verdict;
}
