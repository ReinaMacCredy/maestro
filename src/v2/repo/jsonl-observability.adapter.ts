import { appendFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import type {
  ObservabilityEvent,
  ObservabilityPort,
} from "./observability.port.js";

const DEFAULT_RUNS_DIR = ".maestro/runs";

export interface JsonlObservabilityAdapterOptions {
  readonly repoRoot: string;
  readonly subdir?: string;
}

export class JsonlObservabilityAdapter implements ObservabilityPort {
  readonly #runsDir: string;

  constructor(options: JsonlObservabilityAdapterOptions) {
    this.#runsDir = join(options.repoRoot, options.subdir ?? DEFAULT_RUNS_DIR);
  }

  pathFor(taskId: string): string {
    return join(this.#runsDir, taskId, "observability.jsonl");
  }

  async emit(event: ObservabilityEvent): Promise<void> {
    const dir = join(this.#runsDir, event.task_id);
    await mkdir(dir, { recursive: true });
    await appendFile(this.pathFor(event.task_id), `${JSON.stringify(event)}\n`, "utf8");
  }
}
