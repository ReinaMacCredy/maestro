import { join } from "node:path";
import { ensureDir, appendText, readText, removeIfExists } from "@/shared/lib/fs.js";
import { resolveWithin } from "@/shared/lib/path-safety.js";
import type { TaskContinuationHistoryPort } from "../ports/task-continuation-history.port.js";
import {
  validateTaskContinuationEvent,
  type TaskContinuationEvent,
} from "../domain/task-continuation-types.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

export class FsTaskContinuationHistoryStoreAdapter implements TaskContinuationHistoryPort {
  constructor(private readonly baseDir: string) {}

  async append(taskId: string, event: TaskContinuationEvent): Promise<void> {
    await ensureDir(this.historyDir());
    await appendText(this.historyPath(taskId), `${JSON.stringify(event)}\n`);
  }

  async listRecent(taskId: string, limit: number): Promise<readonly TaskContinuationEvent[]> {
    const raw = await readText(this.historyPath(taskId));
    if (raw === undefined) return [];

    const events = raw
      .split("\n")
      .flatMap((line) => {
        const trimmed = line.trim();
        if (trimmed.length === 0) return [];
        try {
          const validated = validateTaskContinuationEvent(JSON.parse(trimmed));
          return validated ? [validated] : [];
        } catch {
          return [];
        }
      });

    if (limit <= 0) return events;
    return events.slice(-limit);
  }

  async delete(taskId: string): Promise<void> {
    await removeIfExists(this.historyPath(taskId));
  }

  private historyDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks", "local-history");
  }

  private historyPath(taskId: string): string {
    return resolveWithin(this.historyDir(), `${taskId}.jsonl`, "Task continuation history path");
  }
}
