/**
 * JSONL-backed principle store.
 * Storage: `.maestro/principles.jsonl` (one JSON object per line).
 *
 * Simpler than the task store: no locking, no id generation.
 * Principles are low-volume, admin-edited records. Concurrency
 * conflicts are unlikely and not worth the complexity of file locking.
 */

import { join } from "node:path";
import type { Principle, CreatePrincipleInput } from "../domain/principle-types.js";
import type { MilestoneProfile } from "../domain/mission-types.js";
import type { PrincipleStorePort } from "../ports/principle-store.port.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readText, writeText } from "@/shared/lib/fs.js";
import { validatePrinciple } from "../domain/principle-validators.js";
import { MaestroError } from "@/shared/errors.js";

export class JsonlPrincipleStoreAdapter implements PrincipleStorePort {
  constructor(private readonly baseDir: string) {}

  private filePath(): string {
    return join(this.baseDir, MAESTRO_DIR, "principles.jsonl");
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
}
