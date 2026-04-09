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
import { MAESTRO_DIR, NO_SESSION_ID } from "../domain/defaults.js";
import { generateHandoffId, HANDOFF_ID_PATTERN } from "../domain/id.js";
import type {
  CreateUkiHandoffInput,
  ExecuteUkiHandoffContent,
  PlanUkiHandoffContent,
  UkiHandoff,
  UkiHandoffContent,
  UkiHandoffStatus,
  UkiMaestroRefs,
} from "../domain/uki-types.js";
import { UKI_HANDOFF_VERSION } from "../domain/uki-types.js";
import { validateUkiHandoff } from "../domain/validators.js";
import type { HandoffStorePort, UpdateHandoffStatusMeta } from "../ports/handoff-store.port.js";
import { compressUki, parseUki } from "../lib/uki-format.js";
import { ensureDir, readJson, removeIfExists, writeJson } from "../lib/fs.js";
import { MaestroError } from "../domain/errors.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";

const HANDOFFS_DIR = "handoffs";
const LOCK_RETRY_DELAY_MS = 10;
const LOCK_RETRY_COUNT = 100;
const LOCK_STALE_MS = 30_000;
const LEGACY_DEFAULT_CONFIDENCE = 0.5;

function normalizePersistedHandoff(value: unknown): unknown {
  if (!value || typeof value !== "object") {
    return value;
  }

  const record = value as Record<string, unknown>;
  const version = record.version === "5.2" || record.version === "5.3" || record.version === "5.4"
    ? record.version
    : UKI_HANDOFF_VERSION;

  if (record.content && typeof record.content === "object") {
    return {
      ...record,
      version,
      content: normalizeContent(record.content as Record<string, unknown>),
    };
  }

  if (typeof record.uki === "string") {
    return {
      ...record,
      version,
      content: parseUki(record.uki),
    };
  }

  return value;
}

function normalizeContent(value: Record<string, unknown>): UkiHandoffContent {
  const mode = value.mode === "plan" ? "plan" : "execute";
  const common = {
    mode,
    currentState: typeof value.currentState === "string" && value.currentState.length > 0
      ? value.currentState
      : "unspecified",
    sessionCore: typeof value.sessionCore === "string" && value.sessionCore.length > 0
      ? value.sessionCore
      : "handoff",
    decisions: normalizeStringArray(value.decisions),
    artifacts: normalizeStringArray(value.artifacts),
    readMore: normalizeStringArray(value.readMore),
    nextAction: typeof value.nextAction === "string" && value.nextAction.length > 0
      ? value.nextAction
      : "review_handoff",
    summary: typeof value.summary === "string" && value.summary.length > 0
      ? value.summary
      : "Handoff-ready-needs_review",
    maestroRefs: normalizeMaestroRefs(value.maestroRefs),
    cs: normalizeConfidence(value.cs),
    signalDelta: normalizeStringArray(value.signalDelta),
    boundaryState: normalizeStringArray(value.boundaryState),
    risks: normalizeStringArray(value.risks),
    blindSpot: typeof value.blindSpot === "string" && value.blindSpot.length > 0
      ? value.blindSpot
      : undefined,
    metaphor: typeof value.metaphor === "string" && value.metaphor.length > 0
      ? value.metaphor
      : undefined,
    causalDrivers: normalizeStringArray(value.causalDrivers),
    divergences: normalizeStringArray(value.divergences),
  };

  if (mode === "plan") {
    return {
      ...common,
      mode,
      planPaths: normalizeStringArray(value.planPaths),
      maestroSync: normalizeStringArray(value.maestroSync),
    } satisfies PlanUkiHandoffContent;
  }

  return {
    ...common,
    mode,
    touchedFiles: normalizeStringArray(value.touchedFiles),
    completedWork: normalizeStringArray(value.completedWork),
    validation: normalizeStringArray(value.validation),
  } satisfies ExecuteUkiHandoffContent;
}

function normalizeStringArray(value: unknown): readonly string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === "string" && item.length > 0);
}

function normalizeConfidence(value: unknown): { readonly work?: number; readonly summary?: number } {
  if (!value || typeof value !== "object") {
    return { summary: LEGACY_DEFAULT_CONFIDENCE };
  }

  const record = value as Record<string, unknown>;
  return {
    ...(typeof record.work === "number" ? { work: record.work } : {}),
    ...(typeof record.summary === "number" ? { summary: record.summary } : {}),
  };
}

function normalizeMaestroRefs(value: unknown): UkiMaestroRefs {
  if (!value || typeof value !== "object") {
    return {};
  }

  const record = value as Record<string, unknown>;
  return {
    ...(typeof record.missionId === "string" ? { missionId: record.missionId } : {}),
    ...(typeof record.featureId === "string" ? { featureId: record.featureId } : {}),
    ...(typeof record.milestoneId === "string" ? { milestoneId: record.milestoneId } : {}),
    ...(typeof record.planPath === "string" ? { planPath: record.planPath } : {}),
    ...(typeof record.specPath === "string" ? { specPath: record.specPath } : {}),
  };
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
        return validateUkiHandoff(normalizePersistedHandoff(raw));
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

    const branch = this.encodeLegacyToken(legacy.git?.branch, "main");
    const sessionCore = this.encodeLegacyToken(legacy.message, "legacy_handoff");
    const currentState = this.encodeLegacyToken(legacy.git?.diffStat, "legacy_git_state");
    const nextAction = this.encodeLegacyToken(legacy.quickstart, "review_legacy_handoff");
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

  private encodeLegacyToken(value: string | undefined, fallback: string): string {
    const normalized = (value ?? "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "");

    if (normalized.length === 0) {
      return fallback;
    }

    return normalized
      .split("_")
      .filter(Boolean)
      .slice(0, 6)
      .join("_");
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
