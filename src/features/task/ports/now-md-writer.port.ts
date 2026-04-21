import type { Task } from "../domain/task-types.js";

export interface NowMdWriterPort {
  write(tasks: readonly Task[], now?: Date): Promise<void>;
}
