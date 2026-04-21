import { join } from "node:path";
import type { ContractStoreQueryPort } from "../ports/contract-store.port.js";
import type { Task } from "../domain/task-types.js";
import type { NowMdWriterPort } from "../ports/now-md-writer.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, writeText } from "@/shared/lib/fs.js";
import { buildNowMd } from "../domain/now-md-format.js";

export class FsNowMdWriterAdapter implements NowMdWriterPort {
  constructor(
    private readonly baseDir: string,
    private readonly contractStore?: ContractStoreQueryPort,
  ) {}

  private tasksDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks");
  }

  private nowMdPath(): string {
    return join(this.tasksDir(), "NOW.md");
  }

  async write(tasks: readonly Task[], now: Date = new Date()): Promise<void> {
    const content = buildNowMd({
      tasks,
      now,
      contracts: await this.loadContracts(),
    });
    await ensureDir(this.tasksDir());
    await writeText(this.nowMdPath(), content);
  }

  private async loadContracts() {
    if (!this.contractStore) {
      return new Map();
    }

    try {
      const contracts = await this.contractStore.all();
      return new Map(contracts.map((contract) => [contract.id, contract] as const));
    } catch {
      // NOW.md is derived output; contract-loading failures should not block task writes.
      return new Map();
    }
  }
}
