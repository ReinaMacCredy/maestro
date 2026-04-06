import { join } from "node:path";
import { readdir } from "node:fs/promises";
import type { RawLearningEntry, CompiledLearnings } from "../domain/memory-types.js";
import { MAESTRO_DIR, MEMORY_DIR } from "../domain/defaults.js";
import { ensureDir, readJson, writeJson } from "../lib/fs.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";

export class FsLearningStoreAdapter implements LearningStorePort {
  constructor(private readonly baseDir: string) {}

  private rawDir(): string {
    return join(this.baseDir, MAESTRO_DIR, MEMORY_DIR, "learnings", "raw");
  }

  private compiledPath(): string {
    return join(this.baseDir, MAESTRO_DIR, MEMORY_DIR, "learnings", "_compiled.json");
  }

  async appendRaw(entry: RawLearningEntry): Promise<void> {
    await ensureDir(this.rawDir());
    const suffix = `${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const filename = `${entry.sessionDate}-${suffix}.json`;
    await writeJson(join(this.rawDir(), filename), entry);
  }

  async listRaw(): Promise<readonly RawLearningEntry[]> {
    try {
      const files = await readdir(this.rawDir());
      const entries: RawLearningEntry[] = [];
      for (const file of files.filter((f) => f.endsWith(".json")).sort()) {
        const entry = await readJson<RawLearningEntry>(join(this.rawDir(), file));
        if (entry) entries.push(entry);
      }
      return entries;
    } catch {
      return [];
    }
  }

  async rawCount(): Promise<number> {
    try {
      const files = await readdir(this.rawDir());
      return files.filter((f) => f.endsWith(".json")).length;
    } catch {
      return 0;
    }
  }

  async readCompiled(): Promise<CompiledLearnings | undefined> {
    return readJson<CompiledLearnings>(this.compiledPath());
  }

  async writeCompiled(compiled: CompiledLearnings): Promise<void> {
    await ensureDir(join(this.baseDir, MAESTRO_DIR, MEMORY_DIR, "learnings"));
    await writeJson(this.compiledPath(), compiled);
  }
}
