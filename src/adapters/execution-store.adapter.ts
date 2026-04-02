import { readdir } from "node:fs/promises";
import { join } from "node:path";
import type { ExecutionRecord } from "../domain/worker-types.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { validateExecutionRecord } from "../domain/worker-validators.js";
import { ensureDir, readJson, writeJson } from "../lib/fs.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";
import type { ExecutionStorePort } from "../ports/execution-store.port.js";

const EXECUTION_ID_PATTERN = /^[A-Za-z0-9._-]+$/;

export class FsExecutionStoreAdapter implements ExecutionStorePort {
  constructor(private readonly baseDir: string) {}

  private executionsDir(missionId: string): string {
    return join(this.baseDir, MAESTRO_DIR, "missions", missionId, "executions");
  }

  private executionPath(missionId: string, executionId: string): string {
    assertSafeSegment(
      executionId,
      "execution ID",
      EXECUTION_ID_PATTERN,
      "letters, numbers, dots, dashes, and underscores",
    );

    return resolveWithin(
      this.executionsDir(missionId),
      `${executionId}.json`,
      "Execution record path",
    );
  }

  async get(missionId: string, executionId: string): Promise<ExecutionRecord | undefined> {
    const data = await readJson<unknown>(this.executionPath(missionId, executionId));
    if (!data) return undefined;

    try {
      return validateExecutionRecord(data);
    } catch {
      return undefined;
    }
  }

  async save(missionId: string, record: ExecutionRecord): Promise<ExecutionRecord> {
    const validated = validateExecutionRecord(record);
    const path = this.executionPath(missionId, record.id);
    await ensureDir(this.executionsDir(missionId));
    await writeJson(path, validated);
    return validated;
  }

  async list(missionId: string): Promise<readonly ExecutionRecord[]> {
    let entries: string[];
    try {
      entries = await readdir(this.executionsDir(missionId));
    } catch {
      return [];
    }

    const records = await Promise.all(
      entries
        .filter((entry) => entry.endsWith(".json"))
        .map(async (entry) => {
          const id = entry.replace(/\.json$/, "");
          return this.get(missionId, id);
        }),
    );

    return records
      .filter((record): record is ExecutionRecord => record !== undefined)
      .sort((left, right) => left.startedAt.localeCompare(right.startedAt));
  }

  async getByFeature(missionId: string, featureId: string): Promise<readonly ExecutionRecord[]> {
    const all = await this.list(missionId);
    return all.filter((record) => record.featureId === featureId);
  }
}
