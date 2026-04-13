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
} from "../domain/principle-types.js";
import type { MilestoneProfile } from "../domain/mission-types.js";
import type { PrincipleStorePort } from "../ports/principle-store.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readText, writeText, appendText } from "@/shared/lib/fs.js";
import {
  validatePrinciple,
  PrincipleOutcomeRecordSchema,
  safeParsePrincipleOutcomeRecord,
} from "../domain/principle-validators.js";
import { MaestroError } from "@/shared/errors.js";

const DEFAULT_OUTCOME_TAIL_LIMIT = 500;

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
    // Schema validation catches drift (bad outcome values, missing ids) at
    // the write boundary so the JSONL never gains unreadable rows.
    const validated = PrincipleOutcomeRecordSchema.safeParse(record);
    if (!validated.success) return;
    try {
      await ensureDir(join(this.baseDir, MAESTRO_DIR, "principles"));
      await appendText(this.outcomesPath(), JSON.stringify(validated.data) + "\n");
    } catch {
      // Best-effort: outcome recording must never block the caller. The
      // next write attempt will retry. Telemetry can be layered on later.
    }
  }

  async listOutcomes(limit: number = DEFAULT_OUTCOME_TAIL_LIMIT): Promise<readonly PrincipleOutcomeRecord[]> {
    const raw = await readText(this.outcomesPath());
    if (raw === undefined) return [];
    const records: PrincipleOutcomeRecord[] = [];
    for (const line of raw.split("\n")) {
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;
      let json: unknown;
      try {
        json = JSON.parse(trimmed);
      } catch {
        continue;
      }
      const parsed = safeParsePrincipleOutcomeRecord(json);
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
