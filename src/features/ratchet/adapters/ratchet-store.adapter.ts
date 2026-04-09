import { join } from "node:path";
import type { RatchetSuite, RatchetBaseline } from "../domain/memory-types.js";
import { MAESTRO_DIR, MEMORY_DIR } from "../domain/defaults.js";
import { ensureDir, readJson, writeJson } from "../lib/fs.js";
import type { RatchetStorePort } from "../ports/ratchet-store.port.js";

export class FsRatchetStoreAdapter implements RatchetStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, MEMORY_DIR, "ratchet");
  }

  private suitePath(): string {
    return join(this.dir(), "suite.json");
  }

  private baselinePath(): string {
    return join(this.dir(), "baseline.json");
  }

  async getSuite(): Promise<RatchetSuite> {
    return (await readJson<RatchetSuite>(this.suitePath())) ?? { assertions: [] };
  }

  async writeSuite(suite: RatchetSuite): Promise<void> {
    await ensureDir(this.dir());
    await writeJson(this.suitePath(), suite);
  }

  async getBaseline(): Promise<RatchetBaseline | undefined> {
    return readJson<RatchetBaseline>(this.baselinePath());
  }

  async writeBaseline(baseline: RatchetBaseline): Promise<void> {
    await ensureDir(this.dir());
    await writeJson(this.baselinePath(), baseline);
  }
}
