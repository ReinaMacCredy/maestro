import type { Task } from "../types/task.js";

export interface NowMdWriterPort {
  /** Render the NOW.md dashboard for the given tasks and write it atomically. */
  write(tasks: readonly Task[], now?: Date): Promise<void>;
}
