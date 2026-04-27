/**
 * Filesystem adapter for assertion storage
 * Implements the AssertionStorePort using a single assertions.json file per mission
 * Storage layout: .maestro/missions/{missionId}/assertions.json
 */
import { join } from "node:path";
import type { Assertion, CreateAssertionInput, UpdateAssertionInput } from "../domain/mission-types.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import { validateAssertion } from "../domain/mission-validators.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";

export class FsAssertionStoreAdapter implements AssertionStorePort {
  constructor(private readonly baseDir: string) {}

  private missionsRoot(): string {
    return join(this.baseDir, MAESTRO_DIR, "missions");
  }

  private missionDir(missionId: string): string {
    return join(this.missionsRoot(), missionId);
  }

  private assertionsPath(missionId: string): string {
    return join(this.missionDir(missionId), "assertions.json");
  }

  private async readAssertions(missionId: string, strict = false): Promise<readonly Assertion[]> {
    const data = await readJson<{ assertions: unknown[] }>(this.assertionsPath(missionId));
    if (!data?.assertions) return [];

    const assertions: Assertion[] = [];
    for (const a of data.assertions) {
      try {
        assertions.push(validateAssertion(a));
      } catch (err) {
        if (strict) {
          throw new Error(`Assertion store contains an invalid record: ${err instanceof Error ? err.message : String(err)}`);
        }
        // Skip invalid assertions in lenient mode
      }
    }
    return assertions;
  }

  private async writeAssertions(missionId: string, assertions: readonly Assertion[]): Promise<void> {
    await ensureDir(this.missionDir(missionId));
    await writeJson(this.assertionsPath(missionId), { assertions });
  }

  async get(missionId: string, assertionId: string): Promise<Assertion | undefined> {
    const assertions = await this.readAssertions(missionId);
    return assertions.find((a) => a.id === assertionId);
  }

  async exists(missionId: string, assertionId: string): Promise<boolean> {
    const assertion = await this.get(missionId, assertionId);
    return assertion !== undefined;
  }

  async create(missionId: string, input: CreateAssertionInput, id: string): Promise<Assertion> {
    const now = new Date().toISOString();
    const assertion: Assertion = {
      id,
      missionId,
      milestoneId: input.milestoneId,
      featureId: input.featureId,
      result: "pending",
      description: input.description,
      surface: input.surface ?? "cli",
      createdAt: now,
      updatedAt: now,
    };

    const validated = validateAssertion(assertion);
    const assertions = await this.readAssertions(missionId, true);
    await this.writeAssertions(missionId, [...assertions, validated]);
    return validated;
  }

  async update(
    missionId: string,
    assertionId: string,
    input: UpdateAssertionInput,
  ): Promise<Assertion | undefined> {
    const assertions = await this.readAssertions(missionId, true);
    const index = assertions.findIndex((a) => a.id === assertionId);
    if (index === -1) return undefined;

    const now = new Date().toISOString();
    const existing = assertions[index]!;
    const updated: Assertion = {
      id: existing.id,
      missionId: existing.missionId,
      milestoneId: existing.milestoneId,
      featureId: existing.featureId,
      description: existing.description,
      surface: existing.surface,
      createdAt: existing.createdAt,
      result: input.result,
      updatedAt: now,
      evidence: input.evidence,
      waivedReason: input.waivedReason,
    };

    const validated = validateAssertion(updated);
    const newAssertions = [...assertions];
    newAssertions[index] = validated;
    await this.writeAssertions(missionId, newAssertions);
    return validated;
  }

  async list(missionId: string): Promise<readonly Assertion[]> {
    const assertions = await this.readAssertions(missionId);
    // Sort by creation date
    return [...assertions].sort((a, b) => a.createdAt.localeCompare(b.createdAt));
  }

  async listByMilestone(missionId: string, milestoneId: string): Promise<readonly Assertion[]> {
    const assertions = await this.readAssertions(missionId);
    return assertions.filter((a) => a.milestoneId === milestoneId);
  }

  async getMany(missionId: string, assertionIds: readonly string[]): Promise<readonly Assertion[]> {
    const assertions = await this.readAssertions(missionId);
    const idSet = new Set(assertionIds);
    return assertions.filter((a) => idSet.has(a.id));
  }
}
