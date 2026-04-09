/**
 * Filesystem-backed implementation of the v2 HandoffStorePort.
 *
 * Persists records as flat JSON files under `.maestro/handoffs/<id>.json`.
 * Each file contains the full UkiHandoff including the cached UKI v5.2
 * compressed string, so reads do not need to recompress.
 *
 * Ids come from `generateHandoffId()` in `src/domain/id.ts` (shared with
 * the pre-Phase-1 handoff system -- the id shape is stable across the
 * format change).
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { generateHandoffId } from "../domain/id.js";
import type {
  CreateUkiHandoffInput,
  UkiHandoff,
  UkiHandoffStatus,
} from "../domain/uki-types.js";
import type {
  HandoffStorePort,
  UpdateHandoffStatusMeta,
} from "../ports/handoff-store.port.js";
import { compressUki } from "../lib/uki-format.js";
import { ensureDir, readJson, writeJson, removeIfExists } from "../lib/fs.js";
import { UKI_HANDOFF_VERSION } from "../domain/uki-types.js";

const HANDOFFS_DIR = "handoffs";

export class FsHandoffStoreAdapter implements HandoffStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, HANDOFFS_DIR);
  }

  private itemPath(id: string): string {
    return join(this.dir(), `${id}.json`);
  }

  async create(input: CreateUkiHandoffInput): Promise<UkiHandoff> {
    await ensureDir(this.dir());
    const existingIds = await this.listIds();
    const id = generateHandoffId(existingIds, new Date());
    const uki = compressUki(input.slots);
    const handoff: UkiHandoff = {
      id,
      version: UKI_HANDOFF_VERSION,
      timestamp: new Date().toISOString(),
      status: "pending",
      agent: input.agent,
      sessionId: input.sessionId,
      slots: input.slots,
      uki,
    };
    await writeJson(this.itemPath(id), handoff);
    return handoff;
  }

  async get(id: string): Promise<UkiHandoff | undefined> {
    return readJson<UkiHandoff>(this.itemPath(id));
  }

  async getLatestPending(): Promise<UkiHandoff | undefined> {
    const pending = await this.list({ status: "pending" });
    return pending[0];
  }

  async list(filter?: { status?: UkiHandoffStatus }): Promise<readonly UkiHandoff[]> {
    const ids = await this.listIds();
    const results: UkiHandoff[] = [];
    for (const id of ids) {
      const handoff = await this.get(id);
      if (!handoff) continue;
      if (filter?.status && handoff.status !== filter.status) continue;
      results.push(handoff);
    }
    // Newest first -- id is date-sequential, so descending id sort is
    // chronological. Secondary sort by timestamp for determinism when
    // ids collide (shouldn't happen, but be defensive).
    return results.sort((a, b) => {
      if (b.id !== a.id) return b.id.localeCompare(a.id);
      return b.timestamp.localeCompare(a.timestamp);
    });
  }

  async updateStatus(
    id: string,
    status: UkiHandoffStatus,
    meta?: UpdateHandoffStatusMeta,
  ): Promise<UkiHandoff | undefined> {
    const existing = await this.get(id);
    if (!existing) return undefined;
    const nowIso = new Date().toISOString();
    const updated: UkiHandoff = {
      ...existing,
      status,
      pickedUpAt: status === "picked-up"
        ? existing.pickedUpAt ?? nowIso
        : existing.pickedUpAt,
      pickedUpBy: status === "picked-up"
        ? meta?.pickedUpBy ?? existing.pickedUpBy
        : existing.pickedUpBy,
      completedAt: status === "completed"
        ? meta?.completedAt ?? nowIso
        : existing.completedAt,
      report: status === "completed"
        ? meta?.report ?? existing.report
        : existing.report,
    };
    await writeJson(this.itemPath(id), updated);
    return updated;
  }

  async delete(id: string): Promise<boolean> {
    return removeIfExists(this.itemPath(id));
  }

  private async listIds(): Promise<string[]> {
    try {
      const entries = await readdir(this.dir());
      return entries
        .filter((e) => e.endsWith(".json") && !e.startsWith("_"))
        .map((e) => e.replace(/\.json$/, ""))
        .sort();
    } catch {
      return [];
    }
  }
}
