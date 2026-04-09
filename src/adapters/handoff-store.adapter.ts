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
import { open, readdir } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { generateHandoffId, HANDOFF_ID_PATTERN } from "../domain/id.js";
import type {
  CreateUkiHandoffInput,
  UkiHandoff,
  UkiHandoffStatus,
} from "../domain/uki-types.js";
import { validateUkiHandoff } from "../domain/validators.js";
import type {
  HandoffStorePort,
  UpdateHandoffStatusMeta,
} from "../ports/handoff-store.port.js";
import { compressUki } from "../lib/uki-format.js";
import { ensureDir, readJson, writeJson, removeIfExists } from "../lib/fs.js";
import { UKI_HANDOFF_VERSION } from "../domain/uki-types.js";
import { MaestroError } from "../domain/errors.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";

const HANDOFFS_DIR = "handoffs";
const LOCK_RETRY_DELAY_MS = 10;
const LOCK_RETRY_COUNT = 100;

export class FsHandoffStoreAdapter implements HandoffStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, HANDOFFS_DIR);
  }

  private itemPath(id: string): string {
    assertSafeSegment(id, "handoff ID", HANDOFF_ID_PATTERN, "digits and dashes in YYYY-MM-DD-NNN format");
    return resolveWithin(this.dir(), `${id}.json`, "Handoff path");
  }

  private createLockPath(): string {
    return resolveWithin(this.dir(), ".create.lock", "Handoff create lock");
  }

  private claimLockPath(id: string): string {
    return resolveWithin(this.dir(), `${id}.claim.lock`, "Handoff claim lock");
  }

  private async withLock<T>(lockPath: string, fn: () => Promise<T>): Promise<T> {
    await ensureDir(this.dir());
    let attempt = 0;

    while (true) {
      try {
        const handle = await open(lockPath, "wx");
        try {
          return await fn();
        } finally {
          await handle.close();
          await removeIfExists(lockPath);
        }
      } catch (error) {
        const errno = error as NodeJS.ErrnoException;
        if (errno.code !== "EEXIST" || attempt >= LOCK_RETRY_COUNT) {
          throw error;
        }
        attempt += 1;
        await sleep(LOCK_RETRY_DELAY_MS);
      }
    }
  }

  private async readValidated(
    id: string,
    opts: { tolerant?: boolean } = {},
  ): Promise<UkiHandoff | undefined> {
    try {
      const raw = await readJson<unknown>(this.itemPath(id));
      if (raw === undefined) return undefined;
      return validateUkiHandoff(raw);
    } catch (error) {
      if (opts.tolerant) {
        return undefined;
      }
      throw new MaestroError(`Invalid handoff record: ${id}`, [
        error instanceof Error ? error.message : String(error),
        `Repair or remove ${this.itemPath(id)} before retrying`,
      ]);
    }
  }

  private async listIdsDesc(): Promise<string[]> {
    try {
      const entries = await readdir(this.dir());
      return entries
        .filter((entry) => entry.endsWith(".json") && !entry.startsWith("_"))
        .map((entry) => entry.replace(/\.json$/, ""))
        .filter((id) => HANDOFF_ID_PATTERN.test(id))
        .sort()
        .reverse();
    } catch {
      return [];
    }
  }

  async create(input: CreateUkiHandoffInput): Promise<UkiHandoff> {
    return this.withLock(this.createLockPath(), async () => {
      const existingIds = await this.listIdsDesc();
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
    });
  }

  async claimPending(id?: string, pickedUpBy?: string): Promise<UkiHandoff | undefined> {
    const targetId = id ?? await this.findLatestPendingId();
    if (!targetId) return undefined;

    return this.withLock(this.claimLockPath(targetId), async () => {
      const existing = await this.readValidated(targetId, { tolerant: true });
      if (!existing || existing.status !== "pending") {
        return undefined;
      }

      const updated: UkiHandoff = {
        ...existing,
        status: "picked-up",
        pickedUpAt: existing.pickedUpAt ?? new Date().toISOString(),
        pickedUpBy: pickedUpBy ?? existing.pickedUpBy,
      };
      await writeJson(this.itemPath(targetId), updated);
      return updated;
    });
  }

  async get(id: string): Promise<UkiHandoff | undefined> {
    return this.readValidated(id);
  }

  async getLatestPending(): Promise<UkiHandoff | undefined> {
    const latestPendingId = await this.findLatestPendingId();
    return latestPendingId ? this.readValidated(latestPendingId, { tolerant: true }) : undefined;
  }

  async list(filter?: { status?: UkiHandoffStatus }): Promise<readonly UkiHandoff[]> {
    const ids = await this.listIdsDesc();
    const results = await Promise.all(ids.map((id) => this.readValidated(id, { tolerant: true })));
    return results.filter((handoff): handoff is UkiHandoff =>
      handoff !== undefined && (!filter?.status || handoff.status === filter.status),
    );
  }

  async updateStatus(
    id: string,
    status: UkiHandoffStatus,
    meta?: UpdateHandoffStatusMeta,
  ): Promise<UkiHandoff | undefined> {
    const existing = await this.readValidated(id);
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

  private async findLatestPendingId(): Promise<string | undefined> {
    const ids = await this.listIdsDesc();
    for (const id of ids) {
      const handoff = await this.readValidated(id, { tolerant: true });
      if (handoff?.status === "pending") {
        return id;
      }
    }
    return undefined;
  }
}
