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
import { TASK_ID_PATTERN } from "@/features/task/index.js";
import { VERDICT_ID_PATTERN } from "../domain/verdict-id.js";
import type { Verdict } from "../domain/types.js";
import type { VerdictStorePort } from "../ports/storage.js";

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
    const all = await this.history(taskId);
    if (all.length === 0) return undefined;
    // history() returns chronological order; last is most recent
    return all[all.length - 1];
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
    const taskDir = this.taskDir(taskId);
    let entries;
    try {
      entries = await readdir(taskDir, { withFileTypes: true });
    } catch {
      return [];
    }

    const verdicts: Verdict[] = [];
    for (const entry of entries) {
      if (!entry.isFile() || !entry.name.endsWith(".json")) continue;
      const verdictId = entry.name.slice(0, -".json".length);
      if (!VERDICT_ID_PATTERN.test(verdictId)) continue;
      let raw: unknown;
      try {
        raw = await readJson<unknown>(join(taskDir, entry.name));
      } catch {
        continue;
      }
      if (raw === undefined) continue;
      const verdict = coerceVerdict(raw);
      if (verdict !== undefined) {
        verdicts.push(verdict);
      }
    }

    // Sort chronologically by computedAt
    return verdicts.sort((a, b) => a.computedAt.localeCompare(b.computedAt));
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

  return value as Verdict;
}
