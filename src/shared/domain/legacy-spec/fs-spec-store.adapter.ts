/**
 * Filesystem-backed spec store.
 *
 * Layout: `.maestro/specs/<mission-id>.json` — one file per Mission Spec.
 *
 * Writes are atomic via `writeJson` (write-tmp-then-rename). Reads are
 * tolerant: malformed JSON or unrecognised shapes are silently skipped so a
 * single bad file does not poison `list()`.
 *
 * v1 specs on disk are forward-migrated to v2 at read time. The file is NOT
 * rewritten eagerly — it gets upgraded naturally on the next `spec edit`.
 *
 * Specs are committed (not gitignored) so the team shares them.
 */
import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import type { Spec, RuntimeSignal, RuntimeSignalThreshold, RolloutPlan, CanaryPlan } from "./types.js";
import type { LegacySpecStorePort } from "./spec-store.port.js";

const SPECS_DIR = "specs";
const CURRENT_SCHEMA_VERSION = 2;

/**
 * Segment pattern for mission IDs.
 * Mission IDs look like "2026-05-04-001" — date + 3-digit sequence.
 */
const MISSION_ID_PATTERN = /^[\w][\w.-]*$/;

export class FsSpecStoreAdapter implements LegacySpecStorePort {
  constructor(private readonly baseDir: string) {}

  private dir(): string {
    return join(this.baseDir, MAESTRO_DIR, SPECS_DIR);
  }

  private specPath(missionId: string): string {
    assertSafeSegment(missionId, "mission ID", MISSION_ID_PATTERN, "word characters, hyphens, or dots");
    return resolveWithin(this.dir(), `${missionId}.json`, "Spec path");
  }

  async write(spec: Spec): Promise<void> {
    await ensureDir(this.dir());
    await writeJson(this.specPath(spec.mission_id), spec);
  }

  async read(missionId: string): Promise<Spec | undefined> {
    const path = this.specPath(missionId);
    let raw: unknown;
    try {
      raw = await readJson<unknown>(path);
    } catch {
      return undefined;
    }
    return coerceSpec(raw);
  }

  async list(): Promise<Spec[]> {
    let entries;
    try {
      entries = await readdir(this.dir(), { withFileTypes: true });
    } catch {
      return [];
    }
    // One file per spec; reads are I/O-independent.
    const candidates = entries
      .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
      .map((entry) => ({
        missionId: entry.name.slice(0, -".json".length),
        path: join(this.dir(), entry.name),
      }));
    const settled = await Promise.all(
      candidates.map(async ({ missionId, path }): Promise<Spec | undefined> => {
        const raw = await readJson<unknown>(path).catch(() => undefined);
        const spec = coerceSpec(raw);
        if (!spec || spec.mission_id !== missionId) return undefined;
        return spec;
      }),
    );
    const specs = settled.filter((s): s is Spec => s !== undefined);
    return specs.sort((a, b) => a.mission_id.localeCompare(b.mission_id));
  }
}

// ---------------------------------------------------------------------------
// Coercion helpers
// ---------------------------------------------------------------------------

/**
 * Attempt to produce a validated v2 Spec from an unknown on-disk value.
 *
 * - v2 shape: validate strictly, return undefined on any field failure.
 * - v1 shape: forward-migrate at read time (runtime_signals → [], rollout_plan → undefined).
 * - Anything else: return undefined.
 *
 * Files are NOT rewritten; upgrading happens on the next `spec edit`.
 */
export function coerceSpec(value: unknown): Spec | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const v = value as Record<string, unknown>;

  const version = v["schema_version"];

  if (version === CURRENT_SCHEMA_VERSION) {
    return validateV2(v);
  }

  if (version === 1) {
    return migrateV1(v);
  }

  return undefined;
}

function validateV2(v: Record<string, unknown>): Spec | undefined {
  if (
    typeof v["mission_id"] !== "string"
    || !Array.isArray(v["acceptance_criteria"])
    || !Array.isArray(v["non_goals"])
    || !Array.isArray(v["runtime_signals"])
    || typeof v["created_at"] !== "string"
    || typeof v["updated_at"] !== "string"
  ) {
    return undefined;
  }

  const signals = validateRuntimeSignals(v["runtime_signals"]);
  if (signals === undefined) return undefined;

  const rolloutPlan = v["rollout_plan"] !== undefined
    ? validateRolloutPlan(v["rollout_plan"])
    : undefined;
  if (v["rollout_plan"] !== undefined && rolloutPlan === undefined) return undefined;

  return {
    schema_version: 2,
    mission_id: v["mission_id"] as string,
    acceptance_criteria: v["acceptance_criteria"] as Spec["acceptance_criteria"],
    non_goals: v["non_goals"] as Spec["non_goals"],
    runtime_signals: signals,
    rollout_plan: rolloutPlan,
    created_at: v["created_at"] as string,
    updated_at: v["updated_at"] as string,
  };
}

function migrateV1(v: Record<string, unknown>): Spec | undefined {
  if (
    typeof v["mission_id"] !== "string"
    || !Array.isArray(v["acceptance_criteria"])
    || !Array.isArray(v["non_goals"])
    || typeof v["created_at"] !== "string"
    || typeof v["updated_at"] !== "string"
  ) {
    return undefined;
  }

  return {
    schema_version: 2,
    mission_id: v["mission_id"] as string,
    acceptance_criteria: v["acceptance_criteria"] as Spec["acceptance_criteria"],
    non_goals: v["non_goals"] as Spec["non_goals"],
    runtime_signals: [],
    rollout_plan: undefined,
    created_at: v["created_at"] as string,
    updated_at: v["updated_at"] as string,
  };
}

function validateRuntimeSignals(arr: unknown[]): readonly RuntimeSignal[] | undefined {
  const result: RuntimeSignal[] = [];
  for (const item of arr) {
    const signal = validateRuntimeSignal(item);
    if (signal === undefined) return undefined;
    result.push(signal);
  }
  return result;
}

function validateRuntimeSignal(item: unknown): RuntimeSignal | undefined {
  if (typeof item !== "object" || item === null) return undefined;
  const s = item as Record<string, unknown>;

  if (
    typeof s["name"] !== "string"
    || typeof s["provider"] !== "string"
    || typeof s["query"] !== "string"
    || typeof s["severity"] !== "string"
  ) {
    return undefined;
  }

  if (!isRuntimeSignalSeverity(s["severity"])) return undefined;

  const threshold = validateThreshold(s["threshold"]);
  if (threshold === undefined) return undefined;

  return {
    name: s["name"] as string,
    ...(typeof s["description"] === "string" ? { description: s["description"] } : {}),
    provider: s["provider"] as string,
    query: s["query"] as string,
    threshold,
    severity: s["severity"],
  };
}

function validateThreshold(value: unknown): RuntimeSignalThreshold | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const t = value as Record<string, unknown>;
  if (
    typeof t["operator"] !== "string"
    || typeof t["value"] !== "number"
    || !isOperator(t["operator"])
  ) {
    return undefined;
  }
  return { operator: t["operator"], value: t["value"] as number };
}

function isOperator(op: string): op is ">" | "<" | ">=" | "<=" | "==" {
  return op === ">" || op === "<" || op === ">=" || op === "<=" || op === "==";
}

function isRuntimeSignalSeverity(s: string): s is "info" | "warn" | "critical" {
  return s === "info" || s === "warn" || s === "critical";
}

function validateRolloutPlan(value: unknown): RolloutPlan | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const r = value as Record<string, unknown>;

  const featureFlag = typeof r["feature_flag"] === "string" ? r["feature_flag"] : undefined;
  const rollbackCommand = typeof r["rollback_command"] === "string" ? r["rollback_command"] : undefined;

  let canary: CanaryPlan | undefined;
  if (r["canary"] !== undefined) {
    canary = validateCanaryPlan(r["canary"]);
    if (canary === undefined) return undefined;
  }

  return {
    ...(featureFlag !== undefined ? { feature_flag: featureFlag } : {}),
    ...(canary !== undefined ? { canary } : {}),
    ...(rollbackCommand !== undefined ? { rollback_command: rollbackCommand } : {}),
  };
}

function validateCanaryPlan(value: unknown): CanaryPlan | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const c = value as Record<string, unknown>;
  if (!Array.isArray(c["stages"])) return undefined;

  const stages = c["stages"].map((stage: unknown) => {
    if (typeof stage !== "object" || stage === null) return undefined;
    const s = stage as Record<string, unknown>;
    if (typeof s["percent"] !== "number" || typeof s["hold_minutes"] !== "number") return undefined;
    return { percent: s["percent"] as number, hold_minutes: s["hold_minutes"] as number };
  });

  if (stages.some((s) => s === undefined)) return undefined;

  return { stages: stages as CanaryPlan["stages"] };
}
