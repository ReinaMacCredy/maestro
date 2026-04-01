import { readdir, stat } from "node:fs/promises";
import { join } from "node:path";
import type { WorkerRuntime } from "../domain/runtime-types.js";
import { FEATURE_ID_PATTERN } from "../domain/mission-validators.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { validateWorkerRuntime } from "../domain/runtime-validators.js";
import { ensureDir, readJson, removeIfExists, writeJson } from "../lib/fs.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";
import type { RuntimeStorePort } from "../ports/runtime-store.port.js";

export class FsRuntimeStoreAdapter implements RuntimeStorePort {
  constructor(private readonly baseDir: string) {}

  private missionDir(missionId: string): string {
    return join(this.baseDir, MAESTRO_DIR, "missions", missionId);
  }

  private workersDir(missionId: string): string {
    return join(this.missionDir(missionId), "workers");
  }

  private runtimePath(missionId: string, featureId: string): string {
    assertSafeSegment(featureId, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
    return resolveWithin(this.workersDir(missionId), join(featureId, "runtime.json"), "Worker runtime path");
  }

  async get(missionId: string, featureId: string): Promise<WorkerRuntime | undefined> {
    const data = await readJson<unknown>(this.runtimePath(missionId, featureId));
    if (!data) return undefined;
    try {
      return validateWorkerRuntime(data);
    } catch {
      return undefined;
    }
  }

  async save(missionId: string, featureId: string, runtime: WorkerRuntime): Promise<WorkerRuntime> {
    const validated = validateWorkerRuntime(runtime);
    const path = this.runtimePath(missionId, featureId);
    await ensureDir(join(path, ".."));
    await writeJson(path, validated);
    return validated;
  }

  async delete(missionId: string, featureId: string): Promise<boolean> {
    return removeIfExists(this.runtimePath(missionId, featureId));
  }

  async list(missionId: string): Promise<readonly WorkerRuntime[]> {
    const dir = this.workersDir(missionId);
    let entries: string[];
    try {
      entries = await readdir(dir);
    } catch {
      return [];
    }

    const runtimes = await Promise.allSettled(
      entries.map(async (featureId) => {
        const featureDir = join(dir, featureId);
        const stats = await stat(featureDir);
        if (!stats.isDirectory()) return undefined;
        return this.get(missionId, featureId);
      }),
    );

    return runtimes
      .filter((result): result is PromiseFulfilledResult<WorkerRuntime | undefined> => result.status === "fulfilled")
      .map((result) => result.value)
      .filter((runtime): runtime is WorkerRuntime => runtime !== undefined)
      .sort((a, b) => a.featureId.localeCompare(b.featureId));
  }
}
