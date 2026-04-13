/**
 * JSONL-backed principle store.
 * Storage: `.maestro/principles.jsonl` (one JSON object per line).
 *
 * Simpler than the task store: no locking, no id generation.
 * Principles are low-volume, admin-edited records. Concurrency
 * conflicts are unlikely and not worth the complexity of file locking.
 */

import { join } from "node:path";
import type {
  Principle,
  CreatePrincipleInput,
  PrincipleOutcomeRecord,
  PrincipleOutcome,
} from "../domain/principle-types.js";
import type { MilestoneProfile } from "../domain/mission-types.js";
import type { PrincipleStorePort } from "../ports/principle-store.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readText, writeText, appendText } from "@/shared/lib/fs.js";
import { validatePrinciple } from "../domain/principle-validators.js";
import { MaestroError } from "@/shared/errors.js";

const DEFAULT_OUTCOME_TAIL_LIMIT = 500;
const VALID_OUTCOMES: ReadonlySet<PrincipleOutcome> = new Set(["pending", "helpful", "unhelpful"]);

export class JsonlPrincipleStoreAdapter implements PrincipleStorePort {
  constructor(private readonly baseDir: string) {}

  private filePath(): string {
    return join(this.baseDir, MAESTRO_DIR, "principles.jsonl");
  }

  private outcomesPath(): string {
    return join(this.baseDir, MAESTRO_DIR, "principles", "outcomes.jsonl");
  }

  async list(): Promise<readonly Principle[]> {
    return this.readAll();
  }

  async listByProfile(profile: MilestoneProfile): Promise<readonly Principle[]> {
    const all = await this.readAll();
    return all.filter((p) => p.profiles.includes(profile));
  }

  async get(id: string): Promise<Principle | undefined> {
    const all = await this.readAll();
    return all.find((p) => p.id === id);
  }

  async create(input: CreatePrincipleInput): Promise<Principle> {
    const all = await this.readAll();
    if (all.some((p) => p.id === input.id)) {
      throw new MaestroError(`Principle '${input.id}' already exists`, [
        `Use a different --id or remove the existing principle first`,
        `maestro principle remove ${input.id}`,
      ]);
    }

    const principle: Principle = {
      id: input.id,
      name: input.name,
      source: input.source ?? "custom",
      rule: input.rule,
      profiles: input.profiles as readonly MilestoneProfile[],
      mode: input.mode,
      ...(input.gateField !== undefined ? { gateField: input.gateField } : {}),
      ...(input.gateCheck !== undefined ? { gateCheck: input.gateCheck as Principle["gateCheck"] } : {}),
    };

    // Validate the assembled principle through the Zod schema
    validatePrinciple(principle);

    const allUpdated = [...all, principle];
    await this.writeAll(allUpdated);
    return principle;
  }

  async remove(id: string): Promise<boolean> {
    const all = await this.readAll();
    const filtered = all.filter((p) => p.id !== id);
    if (filtered.length === all.length) return false;
    await this.writeAll(filtered);
    return true;
  }

  private async readAll(): Promise<Principle[]> {
    const raw = await readText(this.filePath());
    if (raw === undefined) return [];

    const results: Principle[] = [];
    const lines = raw.split("\n");
    for (const [index, line] of lines.entries()) {
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;

      let parsed: unknown;
      try {
        parsed = JSON.parse(trimmed);
      } catch (error) {
        throw new MaestroError(`Invalid principle record at line ${index + 1}`, [
          this.filePath(),
          error instanceof Error ? error.message : String(error),
        ]);
      }

      try {
        results.push(validatePrinciple(parsed));
      } catch (error) {
        throw new MaestroError(`Invalid principle schema at line ${index + 1}`, [
          this.filePath(),
          error instanceof Error ? error.message : String(error),
        ]);
      }
    }
    return results;
  }

  private async writeAll(principles: readonly Principle[]): Promise<void> {
    await ensureDir(join(this.baseDir, MAESTRO_DIR));
    const lines = principles.map((p) => JSON.stringify(p));
    const content = lines.length === 0 ? "" : lines.join("\n") + "\n";
    await writeText(this.filePath(), content);
  }

  async recordOutcome(record: PrincipleOutcomeRecord): Promise<void> {
    if (!VALID_OUTCOMES.has(record.outcome)) return;
    try {
      await ensureDir(join(this.baseDir, MAESTRO_DIR, "principles"));
      await appendText(this.outcomesPath(), JSON.stringify(record) + "\n");
    } catch {
      // Best-effort: outcome recording must never block the caller. The
      // next write attempt will retry. Telemetry can be layered on later.
    }
  }

  async listOutcomes(limit: number = DEFAULT_OUTCOME_TAIL_LIMIT): Promise<readonly PrincipleOutcomeRecord[]> {
    const raw = await readText(this.outcomesPath());
    if (raw === undefined) return [];
    const lines = raw.split("\n");
    const records: PrincipleOutcomeRecord[] = [];
    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;
      const parsed = safeParseOutcomeRecord(trimmed);
      if (parsed) records.push(parsed);
    }
    if (limit > 0 && records.length > limit) {
      return records.slice(records.length - limit);
    }
    return records;
  }

  async listPendingOutcomesForHandoff(
    handoffId: string,
  ): Promise<readonly PrincipleOutcomeRecord[]> {
    const all = await this.listOutcomes();
    // Each (principleId, handoffId) pair's effective outcome is the newest
    // record. Filter to pairs where the latest row is still "pending".
    const latestByPair = new Map<string, PrincipleOutcomeRecord>();
    for (const record of all) {
      if (record.handoffId !== handoffId) continue;
      const key = `${record.principleId}::${record.handoffId}`;
      const existing = latestByPair.get(key);
      if (!existing || existing.recordedAt <= record.recordedAt) {
        latestByPair.set(key, record);
      }
    }
    return [...latestByPair.values()].filter((r) => r.outcome === "pending");
  }
}

function safeParseOutcomeRecord(line: string): PrincipleOutcomeRecord | undefined {
  try {
    const parsed = JSON.parse(line) as Record<string, unknown>;
    if (
      typeof parsed.principleId !== "string"
      || typeof parsed.handoffId !== "string"
      || typeof parsed.outcome !== "string"
      || typeof parsed.recordedAt !== "string"
      || !VALID_OUTCOMES.has(parsed.outcome as PrincipleOutcome)
    ) {
      return undefined;
    }
    return {
      principleId: parsed.principleId,
      handoffId: parsed.handoffId,
      featureId: typeof parsed.featureId === "string" ? parsed.featureId : undefined,
      missionId: typeof parsed.missionId === "string" ? parsed.missionId : undefined,
      outcome: parsed.outcome as PrincipleOutcome,
      recordedAt: parsed.recordedAt,
    };
  } catch {
    return undefined;
  }
}
