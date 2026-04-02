import { readdir } from "node:fs/promises";
import { join } from "node:path";
import type { RuntimeEventRecord } from "../domain/worker-types.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { validateRuntimeEventRecord } from "../domain/worker-validators.js";
import { appendText, ensureDir, readText } from "../lib/fs.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";
import type { RuntimeEventStorePort } from "../ports/runtime-event-store.port.js";

const FEATURE_ID_PATTERN = /^[A-Za-z0-9._-]+$/;

export class FsRuntimeEventStoreAdapter implements RuntimeEventStorePort {
  constructor(private readonly baseDir: string) {}

  private workersDir(missionId: string): string {
    return join(this.baseDir, MAESTRO_DIR, "missions", missionId, "workers");
  }

  private featureDir(missionId: string, featureId: string): string {
    assertSafeSegment(
      featureId,
      "feature ID",
      FEATURE_ID_PATTERN,
      "letters, numbers, dots, dashes, and underscores",
    );
    return resolveWithin(this.workersDir(missionId), featureId, "Runtime event feature directory");
  }

  private eventPath(missionId: string, featureId: string): string {
    return resolveWithin(this.featureDir(missionId, featureId), "events.jsonl", "Runtime event log path");
  }

  async append(missionId: string, event: RuntimeEventRecord): Promise<RuntimeEventRecord> {
    const validated = validateRuntimeEventRecord(event);
    await ensureDir(this.featureDir(missionId, event.featureId));
    await appendText(this.eventPath(missionId, event.featureId), `${JSON.stringify(validated)}\n`);
    return validated;
  }

  async listByFeature(missionId: string, featureId: string): Promise<readonly RuntimeEventRecord[]> {
    const content = await readText(this.eventPath(missionId, featureId));
    if (!content) return [];

    return content
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
      .map((line) => JSON.parse(line) as unknown)
      .map((value) => {
        try {
          return validateRuntimeEventRecord(value);
        } catch {
          return undefined;
        }
      })
      .filter((event): event is RuntimeEventRecord => event !== undefined)
      .sort((left, right) => left.timestamp.localeCompare(right.timestamp));
  }
}
