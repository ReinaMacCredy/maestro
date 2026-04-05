import { open, readdir } from "node:fs/promises";
import { join } from "node:path";
import type { RuntimeEventRecord } from "../domain/worker-types.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { validateRuntimeEventRecord } from "../domain/worker-validators.js";
import { appendText, ensureDir, readText } from "../lib/fs.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";
import type { RuntimeEventStorePort, RuntimeEventTailOptions } from "../ports/runtime-event-store.port.js";

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

    return parseRuntimeEvents(content);
  }

  async tailByFeature(
    missionId: string,
    featureId: string,
    options: RuntimeEventTailOptions = {},
  ): Promise<readonly RuntimeEventRecord[]> {
    const eventPath = this.eventPath(missionId, featureId);
    const maxBytes = Math.max(1, options.maxBytes ?? 512 * 1024);
    const maxLines = Math.max(1, options.maxLines ?? 256);

    let file;
    try {
      file = await open(eventPath, "r");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") {
        return [];
      }
      throw error;
    }

      try {
        const stat = await file.stat();
        if (stat.size === 0) return [];

        const start = Math.max(0, stat.size - maxBytes);
        const readLength = stat.size - start;
        const buffer = Buffer.alloc(readLength);
        let totalBytesRead = 0;

        while (totalBytesRead < readLength) {
          const { bytesRead } = await file.read(
            buffer,
            totalBytesRead,
            readLength - totalBytesRead,
            start + totalBytesRead,
          );
          if (bytesRead <= 0) {
            break;
          }
          totalBytesRead += bytesRead;
        }

        if (totalBytesRead === 0) return [];

        let content = buffer.subarray(0, totalBytesRead).toString("utf8");
        if (start > 0) {
          const firstNewline = content.indexOf("\n");
          if (firstNewline < 0) return [];
        content = content.slice(firstNewline + 1);
      }

      const lines = content
        .split(/\r?\n/)
        .map((line) => line.trim())
        .filter((line) => line.length > 0)
        .slice(-maxLines);

      return parseRuntimeEvents(lines.join("\n"));
    } finally {
      await file.close();
    }
  }
}

function parseRuntimeEvents(content: string): readonly RuntimeEventRecord[] {
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
