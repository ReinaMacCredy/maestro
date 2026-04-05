import { open, readdir } from "node:fs/promises";
import { join } from "node:path";
import type { RuntimeEventRecord } from "../domain/worker-types.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { validateRuntimeEventRecord } from "../domain/worker-validators.js";
import { appendText, ensureDir, readText } from "../lib/fs.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";
import {
  DEFAULT_RUNTIME_EVENT_TAIL_MAX_BYTES,
  DEFAULT_RUNTIME_EVENT_TAIL_MAX_LINES,
  type RuntimeEventStorePort,
  type RuntimeEventTailOptions,
} from "../ports/runtime-event-store.port.js";

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
    const maxBytes = Math.max(1, options.maxBytes ?? DEFAULT_RUNTIME_EVENT_TAIL_MAX_BYTES);
    const maxLines = Math.max(1, options.maxLines ?? DEFAULT_RUNTIME_EVENT_TAIL_MAX_LINES);

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

      let windowBytes = Math.min(stat.size, maxBytes);
      let start = Math.max(0, stat.size - windowBytes);
      let content = "";

      while (true) {
        content = await readUtf8Range(file, start, stat.size - start);
        if (content.length === 0) return [];

        if (start === 0) break;

        const firstNewline = content.indexOf("\n");
        if (firstNewline >= 0 && firstNewline < content.length - 1) {
          content = content.slice(firstNewline + 1);
          break;
        }

        if (start === 0 || windowBytes >= stat.size) break;
        windowBytes = Math.min(stat.size, windowBytes * 2);
        start = Math.max(0, stat.size - windowBytes);
      }

      const lines = content
        .split(/\r?\n/)
        .map((line) => line.trim())
        .filter((line) => line.length > 0)
        .slice(-maxLines);

      return parseRuntimeEventLines(lines);
    } finally {
      await file.close();
    }
  }
}

async function readUtf8Range(
  file: Awaited<ReturnType<typeof open>>,
  start: number,
  readLength: number,
): Promise<string> {
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

  if (totalBytesRead === 0) return "";
  return buffer.subarray(0, totalBytesRead).toString("utf8");
}

function parseRuntimeEvents(content: string): readonly RuntimeEventRecord[] {
  return parseRuntimeEventLines(
    content
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.length > 0),
  );
}

function parseRuntimeEventLines(lines: readonly string[]): readonly RuntimeEventRecord[] {
  return lines
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
