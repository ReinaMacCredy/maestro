import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { MaestroError } from "@/shared/errors.js";
import { ensureDir, readText, writeJson } from "@/shared/lib/fs.js";
import { assertSafeSegment } from "@/shared/lib/path-safety.js";
import { validateContract } from "../domain/contract/contract-state.js";
import { ANY_TASK_ID_PATTERN } from "@/v2/types/task.js";
import type { Contract } from "../domain/contract/contract-types.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";

export class FsContractVersionStoreAdapter implements ContractVersionStorePort {
  constructor(private readonly baseDir: string) {}

  async write(taskId: string, version: number, contract: Contract): Promise<void> {
    assertSafeSegment(taskId, "task id", ANY_TASK_ID_PATTERN, "'tsk-' followed by 6 hex chars or v2 base36 form");
    const dir = this.taskVersionDir(taskId);
    await ensureDir(dir);
    await writeJson(join(dir, `v${version}.json`), contract);
  }

  async readCurrent(taskId: string): Promise<Contract | undefined> {
    const versions = await this.history(taskId);
    if (versions.length === 0) return undefined;
    return versions[versions.length - 1];
  }

  async readVersion(taskId: string, version: number): Promise<Contract | undefined> {
    assertSafeSegment(taskId, "task id", ANY_TASK_ID_PATTERN, "'tsk-' followed by 6 hex chars or v2 base36 form");
    const path = join(this.taskVersionDir(taskId), `v${version}.json`);
    const raw = await readText(path);
    if (raw === undefined) return undefined;
    return this.parseContract(raw, path);
  }

  async history(taskId: string): Promise<readonly Contract[]> {
    assertSafeSegment(taskId, "task id", ANY_TASK_ID_PATTERN, "'tsk-' followed by 6 hex chars or v2 base36 form");
    const dir = this.taskVersionDir(taskId);
    const filenames = await this.listVersionFilenames(dir);
    const contracts: Contract[] = [];
    for (const filename of filenames) {
      const path = join(dir, filename);
      const raw = await readText(path);
      if (raw === undefined) continue;
      contracts.push(this.parseContract(raw, path));
    }
    return contracts;
  }

  private taskVersionDir(taskId: string): string {
    return join(this.baseDir, MAESTRO_DIR, "contracts", taskId);
  }

  private async listVersionFilenames(dir: string): Promise<readonly string[]> {
    try {
      const entries = await readdir(dir, { withFileTypes: true });
      return entries
        .filter((e) => e.isFile() && /^v\d+\.json$/.test(e.name))
        .map((e) => e.name)
        .sort((a, b) => {
          const numA = parseInt(a.slice(1), 10);
          const numB = parseInt(b.slice(1), 10);
          return numA - numB;
        });
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw error;
    }
  }

  private parseContract(raw: string, path: string): Contract {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new MaestroError(`Versioned contract file is corrupted: ${path}`, [
        "Repair or remove the malformed contract JSON before retrying",
      ]);
    }
    const validated = validateContract(parsed);
    if (!validated) {
      throw new MaestroError(`Versioned contract file contains an invalid record: ${path}`, [
        "Repair the contract JSON before retrying",
      ]);
    }
    return validated;
  }
}
