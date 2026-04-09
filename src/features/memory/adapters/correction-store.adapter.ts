import { join } from "node:path";
import { readdir } from "node:fs/promises";
import type { Correction, CreateCorrectionInput, CorrectionQuery } from "../domain/memory-types.js";
import { MAESTRO_DIR, MEMORY_DIR } from "@/domain/defaults.js";
import { ensureDir, readJson, writeJson, removeIfExists } from "@/lib/fs.js";
import type { CorrectionStorePort } from "../ports/correction-store.port.js";

export class FsCorrectionStoreAdapter implements CorrectionStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, MEMORY_DIR, "corrections");
  }

  private itemPath(id: string): string {
    return join(this.dir(), `${id}.json`);
  }

  async create(input: CreateCorrectionInput): Promise<Correction> {
    await ensureDir(this.dir());
    const id = await this.nextId();
    const now = new Date().toISOString();
    const correction: Correction = {
      id,
      rule: input.rule,
      source: input.source,
      trigger: input.trigger,
      severity: input.severity,
      createdAt: now,
      updatedAt: now,
    };
    await writeJson(this.itemPath(id), correction);
    return correction;
  }

  async get(id: string): Promise<Correction | undefined> {
    return readJson<Correction>(this.itemPath(id));
  }

  async list(): Promise<readonly Correction[]> {
    const ids = await this.listIds();
    const corrections: Correction[] = [];
    for (const id of ids) {
      const c = await this.get(id);
      if (c) corrections.push(c);
    }
    return corrections.sort((a, b) => b.createdAt.localeCompare(a.createdAt) || b.id.localeCompare(a.id));
  }

  async search(query: CorrectionQuery): Promise<readonly Correction[]> {
    const all = await this.list();
    return all.filter((c) => this.matchesQuery(c, query));
  }

  async update(id: string, input: Partial<Correction>): Promise<Correction | undefined> {
    const existing = await this.get(id);
    if (!existing) return undefined;
    const updated: Correction = {
      ...existing,
      ...input,
      id: existing.id,
      createdAt: existing.createdAt,
      updatedAt: new Date().toISOString(),
    };
    await writeJson(this.itemPath(id), updated);
    return updated;
  }

  async remove(id: string): Promise<boolean> {
    return removeIfExists(this.itemPath(id));
  }

  private matchesQuery(correction: Correction, query: CorrectionQuery): boolean {
    if (query.keywords?.length) {
      const triggerKeywords = correction.trigger.keywords.map((k) => k.toLowerCase());
      const match = query.keywords.some((qk) =>
        triggerKeywords.some((tk) => tk.includes(qk.toLowerCase())),
      );
      if (!match) return false;
    }

    if (query.filePaths?.length) {
      const globs = correction.trigger.fileGlobs;
      if (globs.length > 0) {
        const match = query.filePaths.some((fp) =>
          globs.some((g) => {
            const glob = new Bun.Glob(g);
            return glob.match(fp);
          }),
        );
        if (!match) return false;
      }
    }

    if (query.text) {
      const text = query.text.toLowerCase();
      const inRule = correction.rule.toLowerCase().includes(text);
      const inSource = correction.source.toLowerCase().includes(text);
      const inKeywords = correction.trigger.keywords.some((k) =>
        k.toLowerCase().includes(text),
      );
      if (!inRule && !inSource && !inKeywords) return false;
    }

    return true;
  }

  private async listIds(): Promise<string[]> {
    try {
      const entries = await readdir(this.dir());
      return entries
        .filter((e) => e.endsWith(".json") && !e.startsWith("_"))
        .map((e) => e.replace(".json", ""))
        .sort();
    } catch {
      return [];
    }
  }

  private async nextId(): Promise<string> {
    const ids = await this.listIds();
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, "0");
    const d = String(now.getDate()).padStart(2, "0");
    const prefix = `${y}-${m}-${d}`;
    const todayIds = ids.filter((id) => id.startsWith(prefix));
    let maxSeq = 0;
    for (const id of todayIds) {
      const seqStr = id.slice(prefix.length + 1);
      const seq = parseInt(seqStr, 10);
      if (!Number.isNaN(seq) && seq > maxSeq) maxSeq = seq;
    }
    return `${prefix}-${String(maxSeq + 1).padStart(3, "0")}`;
  }
}
