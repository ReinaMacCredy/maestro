/**
 * Filesystem-backed run-state store.
 *
 * Layout: `.maestro/runs/<task-id>/state.json`
 *
 * Writes are atomic via `writeJson` (write-tmp-then-rename).
 * `increment` is a synchronous read-modify-write within the async flow;
 * it does not provide cross-process locking — adequate for single-agent use.
 */
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import { TASK_ID_PATTERN } from "@/features/task/index.js";
import type { RunState } from "../domain/run-state.js";
import type { RunStateDelta, RunStateStorePort } from "../ports/run-state-store.port.js";

const RUNS_DIR = "runs";

export class FsRunStateStoreAdapter implements RunStateStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, RUNS_DIR);
  }

  private taskDir(taskId: string): string {
    assertSafeSegment(taskId, "task ID", TASK_ID_PATTERN, "tsk- followed by 6 hex characters");
    return resolveWithin(this.dir(), taskId, "Run-state task directory");
  }

  private statePath(taskId: string): string {
    return resolveWithin(this.taskDir(taskId), "state.json", "Run-state path");
  }

  async read(taskId: string): Promise<RunState | undefined> {
    const path = this.statePath(taskId);
    const raw = await readJson<unknown>(path);
    if (raw === undefined) return undefined;
    return coerceRunState(raw);
  }

  async write(taskId: string, state: RunState): Promise<void> {
    const taskDir = this.taskDir(taskId);
    await ensureDir(taskDir);
    await writeJson(this.statePath(taskId), state);
  }

  async increment(taskId: string, delta: RunStateDelta): Promise<RunState> {
    const existing = await this.read(taskId);
    const base: RunState = existing ?? {
      schemaVersion: 1,
      taskId,
      retryCount: 0,
      wallClockElapsedSeconds: 0,
      lastUpdatedAt: new Date().toISOString(),
    };

    const next: RunState = {
      ...base,
      retryCount: base.retryCount + (delta.retryCount ?? 0),
      wallClockElapsedSeconds: base.wallClockElapsedSeconds + (delta.wallClockElapsedSeconds ?? 0),
      tokensUsed: delta.tokensUsed !== undefined || base.tokensUsed !== undefined
        ? (base.tokensUsed ?? 0) + (delta.tokensUsed ?? 0)
        : undefined,
      lastUpdatedAt: new Date().toISOString(),
    };

    await this.write(taskId, next);
    return next;
  }
}

function coerceRunState(value: unknown): RunState | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const v = value as Record<string, unknown>;

  if (
    v["schemaVersion"] !== 1
    || typeof v["taskId"] !== "string"
    || typeof v["retryCount"] !== "number"
    || typeof v["wallClockElapsedSeconds"] !== "number"
    || typeof v["lastUpdatedAt"] !== "string"
  ) {
    return undefined;
  }

  return value as RunState;
}
