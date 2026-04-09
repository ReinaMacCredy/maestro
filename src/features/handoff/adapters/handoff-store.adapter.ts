/**
 * Filesystem-backed handoff store.
 *
 * Persists records as flat JSON files under `.maestro/handoffs/<id>.json`.
 * New writes store the canonical structured handoff payload in `content`
 * plus the cached UKI transfer string.
 */
import { join } from "node:path";
import { open, readdir, stat } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import { MAESTRO_DIR } from "@/domain/defaults.js";
import { NO_SESSION_ID } from "../domain/constants.js";
import { generateHandoffId, HANDOFF_ID_PATTERN } from "@/domain/id.js";
import type {
  CreateUkiHandoffInput,
  ExecuteUkiHandoffContent,
  UkiHandoff,
  UkiHandoffStatus,
} from "../domain/uki-types.js";
import { SUPPORTED_UKI_HANDOFF_VERSIONS, UKI_HANDOFF_VERSION } from "../domain/uki-types.js";
import { validateUkiHandoff, validateUkiHandoffContent } from "../domain/validators.js";
import type { HandoffStorePort, UpdateHandoffStatusMeta } from "../ports/handoff-store.port.js";
import { compressUki, parseUki } from "../lib/uki-format.js";
import { ensureDir, readJson, removeIfExists, writeJson } from "@/lib/fs.js";
import { MaestroError } from "@/domain/errors.js";
import { assertSafeSegment, resolveWithin } from "@/lib/path-safety.js";
import { normalizeUkiToken } from "../lib/uki-token.js";

const HANDOFFS_DIR = "handoffs";
const LOCK_RETRY_DELAY_MS = 10;
const LOCK_RETRY_COUNT = 100;
const LOCK_STALE_MS = 30_000;
const LEGACY_DEFAULT_CONFIDENCE = 0.5;

function normalizePersistedHandoff(value: unknown): UkiHandoff {
  if (!value || typeof value !== "object") {
    return validateUkiHandoff(value);
  }

  const record = value as Record<string, unknown>;
  const version = normalizePersistedVersion(record.version);

  if (record.content && typeof record.content === "object") {
    const content = validateUkiHandoffContent(record.content);
    const validated = validateUkiHandoff({
      ...record,
      version,
      content,
      uki: typeof record.uki === "string" && record.uki.length > 0
        ? record.uki
        : compressUki(content),
    });
    return {
      ...validated,
      version,
      uki: compressUki(validated.content),
    };
  }

  if (typeof record.uki === "string") {
    const content = parseUki(record.uki);
    const validated = validateUkiHandoff({
      ...record,
      version,
      content,
      uki: record.uki,
    });
    return {
      ...validated,
      version,
      uki: compressUki(validated.content),
    };
  }

  return validateUkiHandoff({
    ...record,
    version,
  });
}

function normalizePersistedVersion(value: unknown): UkiHandoff["version"] {
  if (
    typeof value === "string"
    && (SUPPORTED_UKI_HANDOFF_VERSIONS as readonly string[]).includes(value)
  ) {
    return value as UkiHandoff["version"];
  }
  return UKI_HANDOFF_VERSION;
}

export class FsHandoffStoreAdapter implements HandoffStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, HANDOFFS_DIR);
  }

  private itemPath(id: string): string {
    assertSafeSegment(id, "handoff ID", HANDOFF_ID_PATTERN, "digits and dashes in YYYY-MM-DD-NNN format");
    return resolveWithin(this.dir(), `${id}.json`, "Handoff path");
  }

  private legacyDir(id: string): string {
    assertSafeSegment(id, "handoff ID", HANDOFF_ID_PATTERN, "digits and dashes in YYYY-MM-DD-NNN format");
    return resolveWithin(this.dir(), id, "Legacy handoff directory");
  }

  private legacyEnvelopePath(id: string): string {
    return resolveWithin(this.legacyDir(id), "envelope.json", "Legacy handoff envelope");
  }

  private createLockPath(): string {
    return resolveWithin(this.dir(), ".create.lock", "Handoff create lock");
  }

  private latestClaimLockPath(): string {
    return resolveWithin(this.dir(), ".claim-latest.lock", "Handoff latest-claim lock");
  }

  private claimLockPath(id: string): string {
    return resolveWithin(this.dir(), `${id}.claim.lock`, "Handoff claim lock");
  }

  private async removeStaleLock(lockPath: string): Promise<boolean> {
    try {
      const lockStat = await stat(lockPath);
      if (Date.now() - lockStat.mtimeMs < LOCK_STALE_MS) {
        return false;
      }
      await removeIfExists(lockPath);
      return true;
    } catch (error) {
      const errno = error as NodeJS.ErrnoException;
      if (errno.code === "ENOENT") {
        return false;
      }
      throw error;
    }
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
        if (errno.code !== "EEXIST") {
          throw error;
        }
        if (await this.removeStaleLock(lockPath)) {
          continue;
        }
        if (attempt >= LOCK_RETRY_COUNT) {
          throw new MaestroError(`Lock is still active: ${lockPath}`, [
            "Retry the handoff command once the other process exits",
            `If this lock is stale, remove it manually: rm ${lockPath}`,
          ]);
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
        if (raw !== undefined) {
          return normalizePersistedHandoff(raw);
        }

      const legacyEnvelope = await readJson<unknown>(this.legacyEnvelopePath(id));
      if (legacyEnvelope !== undefined) {
        return this.convertLegacyEnvelope(id, legacyEnvelope);
      }
      return undefined;
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
      return [...new Set(entries
        .filter((entry) => !entry.startsWith("_"))
        .flatMap((entry) => entry.endsWith(".json")
          ? [entry.replace(/\.json$/, "")]
          : [entry],
        )
        .filter((id) => HANDOFF_ID_PATTERN.test(id))
        .sort()
        .reverse())];
    } catch {
      return [];
    }
  }

  async create(input: CreateUkiHandoffInput): Promise<UkiHandoff> {
    return this.withLock(this.createLockPath(), async () => {
      const existingIds = await this.listIdsDesc();
      const id = generateHandoffId(existingIds, new Date());
      const uki = compressUki(input.content);
      const handoff: UkiHandoff = {
        id,
        version: UKI_HANDOFF_VERSION,
        timestamp: new Date().toISOString(),
        status: "pending",
        agent: input.agent,
        sessionId: input.sessionId,
        content: input.content,
        uki,
      };
      await writeJson(this.itemPath(id), handoff);
      return handoff;
    });
  }

  async claimPending(id?: string, pickedUpBy?: string): Promise<UkiHandoff | undefined> {
    if (id) {
      return this.withLock(this.claimLockPath(id), async () =>
        this.claimPendingUnlocked(id, pickedUpBy),
      );
    }

    return this.withLock(this.latestClaimLockPath(), async () => {
      const targetId = await this.findLatestPendingId();
      if (!targetId) return undefined;
      return this.claimPendingUnlocked(targetId, pickedUpBy);
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

  private async claimPendingUnlocked(
    id: string,
    pickedUpBy?: string,
  ): Promise<UkiHandoff | undefined> {
    const existing = await this.readValidated(id, { tolerant: true });
    if (!existing || existing.status !== "pending") {
      return undefined;
    }

    const updated: UkiHandoff = {
      ...existing,
      status: "picked-up",
      pickedUpAt: existing.pickedUpAt ?? new Date().toISOString(),
      pickedUpBy: pickedUpBy ?? existing.pickedUpBy,
    };
    await writeJson(this.itemPath(id), updated);
    return updated;
  }

  private convertLegacyEnvelope(id: string, value: unknown): UkiHandoff {
    const envelope = value as {
      readonly handoff?: {
        readonly id?: string;
        readonly timestamp?: string;
        readonly message?: string;
        readonly sitrep?: string;
        readonly quickstart?: string;
        readonly session?: {
          readonly agent?: string;
          readonly sessionId?: string;
        };
        readonly git?: {
          readonly branch?: string;
          readonly diffStat?: string;
        };
      };
      readonly status?: UkiHandoffStatus;
      readonly pickedUpAt?: string;
      readonly pickedUpBy?: string;
      readonly completedAt?: string;
      readonly report?: string;
    };

    const legacy = envelope.handoff;
    if (
      !legacy
      || typeof legacy.timestamp !== "string"
      || typeof legacy.message !== "string"
      || typeof legacy.sitrep !== "string"
      || typeof legacy.quickstart !== "string"
      || typeof envelope.status !== "string"
    ) {
      throw new Error(`Legacy handoff ${id} is missing required fields`);
    }

      const branch = normalizeUkiToken(legacy.git?.branch, { fallback: "main", maxSegments: 6 });
      const sessionCore = normalizeUkiToken(legacy.message, { fallback: "legacy_handoff", maxSegments: 6 });
      const currentState = normalizeUkiToken(legacy.git?.diffStat, { fallback: "legacy_git_state", maxSegments: 6 });
      const nextAction = normalizeUkiToken(legacy.quickstart, { fallback: "review_legacy_handoff", maxSegments: 6 });
      const summary = this.encodeLegacySummary(legacy.message);
    const content: ExecuteUkiHandoffContent = {
      mode: "execute",
      currentState,
      sessionCore,
      decisions: [],
      artifacts: [`branch_${branch}`],
      readMore: [`branch_${branch}`],
      nextAction,
      summary,
      maestroRefs: {},
      cs: { summary: LEGACY_DEFAULT_CONFIDENCE },
      signalDelta: [],
      boundaryState: [],
      risks: [],
      causalDrivers: [],
      divergences: [],
      touchedFiles: [],
      completedWork: [],
      validation: [],
    };

    return {
      id: legacy.id ?? id,
      version: UKI_HANDOFF_VERSION,
      timestamp: legacy.timestamp,
      status: envelope.status,
      agent: legacy.session?.agent ?? "unknown",
      sessionId: legacy.session?.sessionId ?? NO_SESSION_ID,
      content,
      uki: compressUki(content),
      pickedUpAt: envelope.pickedUpAt,
      pickedUpBy: envelope.pickedUpBy,
      completedAt: envelope.completedAt,
      report: envelope.report,
    };
  }

    private encodeLegacySummary(value: string): string {
    const normalized = value
      .replace(/\s+/g, "_")
      .replace(/[^A-Za-z0-9_-]/g, "")
      .replace(/^_+|_+$/g, "");

    if (normalized.length === 0) {
      return "Legacy_handoff";
    }

    return normalized.slice(0, 80);
  }
}
