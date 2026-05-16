import { join } from "node:path";
import { ensureDir, writeText } from "@/shared/lib/fs.js";
import type { Task } from "../types/task.js";
import type { NowMdWriterPort } from "./now-md-writer.port.js";

const MAESTRO_DIR = ".maestro";
const TASKS_DIR = "tasks";
const NOW_MD = "NOW.md";

export interface FsNowMdWriterOptions {
  readonly repoRoot: string;
  /**
   * Pure renderer; injected from `service/` so this adapter does not import
   * across the repo -> service layer boundary (forbidden by lint:arch).
   */
  readonly format: (tasks: readonly Task[], now: Date) => string;
}

export class FsNowMdWriter implements NowMdWriterPort {
  readonly #repoRoot: string;
  readonly #format: (tasks: readonly Task[], now: Date) => string;

  constructor(options: FsNowMdWriterOptions) {
    this.#repoRoot = options.repoRoot;
    this.#format = options.format;
  }

  async write(tasks: readonly Task[], now: Date = new Date()): Promise<void> {
    const tasksDir = join(this.#repoRoot, MAESTRO_DIR, TASKS_DIR);
    await ensureDir(tasksDir);
    const content = this.#format(tasks, now);
    await writeText(join(tasksDir, NOW_MD), content);
  }
}
