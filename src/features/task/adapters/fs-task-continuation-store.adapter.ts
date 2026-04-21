import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, fileExists, readJson, removeIfExists, writeJson } from "@/shared/lib/fs.js";
import { MaestroError } from "@/shared/errors.js";
import { resolveWithin } from "@/shared/lib/path-safety.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";
import {
  validateTaskContinuationSummary,
  type TaskContinuationSummary,
} from "../domain/task-continuation-types.js";

type ContinuationState = "active" | "completed";

export class FsTaskContinuationStoreAdapter implements TaskContinuationStorePort {
  constructor(private readonly baseDir: string) {}

  async getActive(taskId: string): Promise<TaskContinuationSummary | undefined> {
    await this.assertNoSplitState(taskId);
    return this.readSummary(this.summaryPath("active", taskId));
  }

  async getCompleted(taskId: string): Promise<TaskContinuationSummary | undefined> {
    await this.assertNoSplitState(taskId);
    return this.readSummary(this.summaryPath("completed", taskId));
  }

  async listActive(): Promise<readonly TaskContinuationSummary[]> {
    return this.listSummaries("active");
  }

  async listCompleted(): Promise<readonly TaskContinuationSummary[]> {
    return this.listSummaries("completed");
  }

  private async listSummaries(state: ContinuationState): Promise<readonly TaskContinuationSummary[]> {
    const entries = await this.listSummaryFiles(state);
    const summaries = await Promise.all(entries.map((entry) => this.readSummary(join(this.stateDir(state), entry))));
    return summaries
      .filter((summary): summary is TaskContinuationSummary => summary !== undefined)
      .sort((left, right) => right.lastActiveAt.localeCompare(left.lastActiveAt));
  }

  async upsertActive(summary: TaskContinuationSummary): Promise<TaskContinuationSummary> {
    await this.assertNoSplitState(summary.taskId);
    await ensureDir(this.stateDir("active"));
    await writeJson(this.summaryPath("active", summary.taskId), summary);
    return summary;
  }

  async archiveCompleted(summary: TaskContinuationSummary): Promise<TaskContinuationSummary> {
    await this.assertNoSplitState(summary.taskId);
    await ensureDir(this.stateDir("completed"));
    await removeIfExists(this.summaryPath("active", summary.taskId));
    await writeJson(this.summaryPath("completed", summary.taskId), summary);
    return summary;
  }

  async reopen(taskId: string, nextSummary: TaskContinuationSummary): Promise<TaskContinuationSummary | undefined> {
    const archived = await this.getCompleted(taskId);
    if (!archived) return undefined;

    await ensureDir(this.stateDir("active"));
    await removeIfExists(this.summaryPath("completed", taskId));
    await writeJson(this.summaryPath("active", taskId), nextSummary);
    return nextSummary;
  }

  async delete(taskId: string): Promise<void> {
    await Promise.all([
      removeIfExists(this.summaryPath("active", taskId)),
      removeIfExists(this.summaryPath("completed", taskId)),
    ]);
  }

  async deleteCompleted(taskId: string): Promise<void> {
    await removeIfExists(this.summaryPath("completed", taskId));
  }

  private continuationsDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks", "continuations");
  }

  private stateDir(state: ContinuationState): string {
    return join(this.continuationsDir(), state);
  }

  private summaryPath(state: ContinuationState, taskId: string): string {
    return resolveWithin(this.stateDir(state), `${taskId}.json`, "Task continuation summary path");
  }

  private async assertNoSplitState(taskId: string): Promise<void> {
    const [hasActive, hasCompleted] = await Promise.all([
      fileExists(this.summaryPath("active", taskId)),
      fileExists(this.summaryPath("completed", taskId)),
    ]);
    if (hasActive && hasCompleted) {
      throw new MaestroError(`Task continuation for '${taskId}' needs repair before it can be used`, [
        "Both active and completed summaries exist for the same task",
        "Remove the stale duplicate from .maestro/tasks/continuations/",
      ]);
    }
  }

  private async readSummary(path: string): Promise<TaskContinuationSummary | undefined> {
    const raw = await readJson<unknown>(path);
    if (raw === undefined) return undefined;
    const validated = validateTaskContinuationSummary(raw);
    if (!validated) {
      throw new MaestroError(`Task continuation summary is invalid: ${path}`, [
        "Repair the JSON summary before retrying",
      ]);
    }
    return validated;
  }

  private async listSummaryFiles(state: ContinuationState): Promise<readonly string[]> {
    try {
      const entries = await readdir(this.stateDir(state), { withFileTypes: true });
      return entries
        .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
        .map((entry) => entry.name);
    } catch (error) {
      const errno = error as NodeJS.ErrnoException;
      if (errno.code === "ENOENT") return [];
      throw error;
    }
  }
}
