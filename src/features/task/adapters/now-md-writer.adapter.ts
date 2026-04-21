import { join } from "node:path";
import type { Task } from "../domain/task-types.js";
import type { NowMdWriterPort } from "../ports/now-md-writer.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, writeText } from "@/shared/lib/fs.js";
import { buildNowMd } from "../usecases/write-now-md.usecase.js";

export class FsNowMdWriterAdapter implements NowMdWriterPort {
  constructor(private readonly baseDir: string) {}

  private tasksDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks");
  }

  private nowMdPath(): string {
    return join(this.tasksDir(), "NOW.md");
  }

  async write(tasks: readonly Task[], now: Date = new Date()): Promise<void> {
    const content = buildNowMd({ tasks, now });
    await ensureDir(this.tasksDir());
    await writeText(this.nowMdPath(), content);
  }
}
