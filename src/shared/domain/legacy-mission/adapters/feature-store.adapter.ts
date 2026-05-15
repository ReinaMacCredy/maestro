/**
 * Filesystem adapter for feature storage
 * Implements the FeatureStorePort using one JSON file per feature
 * Storage layout: .maestro/missions/{missionId}/features/{featureId}.json
 */
import type { Feature, CreateFeatureInput, UpdateFeatureInput } from "../types.js";
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import { FEATURE_ID_PATTERN, validateFeature } from "../validators.js";
import { migrateLegacyWorkerType } from "@/features/mission/feature/feature-migration.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";

export class FsFeatureStoreAdapter implements FeatureStorePort {
  constructor(private readonly baseDir: string) {}

  private missionsRoot(): string {
    return join(this.baseDir, MAESTRO_DIR, "missions");
  }

  private missionDir(missionId: string): string {
    return join(this.missionsRoot(), missionId);
  }

  private featuresDir(missionId: string): string {
    return join(this.missionDir(missionId), "features");
  }

  private featurePath(missionId: string, featureId: string): string {
    assertSafeSegment(featureId, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
    return resolveWithin(this.featuresDir(missionId), `${featureId}.json`, "Feature path");
  }

  async get(missionId: string, featureId: string): Promise<Feature | undefined> {
    const path = this.featurePath(missionId, featureId);
    const data = await readJson<unknown>(path);
    if (!data) return undefined;
    const { normalized, migrated } = migrateLegacyWorkerType(data);
    try {
      const validated = validateFeature(normalized);
      if (migrated) {
        await writeJson(path, validated);
      }
      return validated;
    } catch {
      return undefined;
    }
  }

  async exists(missionId: string, featureId: string): Promise<boolean> {
    const path = this.featurePath(missionId, featureId);
    try {
      const stat = await (await import("node:fs/promises")).stat(path);
      return stat.isFile();
    } catch {
      return false;
    }
  }

  async create(missionId: string, input: CreateFeatureInput, id: string): Promise<Feature> {
    assertSafeSegment(id, "feature ID", FEATURE_ID_PATTERN, "letters, numbers, dashes, and underscores");
    const now = new Date().toISOString();
    const feature: Feature = {
      id,
      missionId,
      milestoneId: input.milestoneId,
      status: "pending",
      title: input.title,
      description: input.description,
      agentType: input.agentType,
      verificationSteps: input.verificationSteps,
      dependsOn: input.dependsOn ?? [],
      fulfills: input.fulfills ?? [],
      preconditions: input.preconditions,
      expectedBehavior: input.expectedBehavior,
      createdAt: now,
      updatedAt: now,
    };

    const validated = validateFeature(feature);
    await ensureDir(this.featuresDir(missionId));
    await writeJson(this.featurePath(missionId, id), validated);
    return validated;
  }

  async update(
    missionId: string,
    featureId: string,
    input: UpdateFeatureInput,
  ): Promise<Feature | undefined> {
    const existing = await this.get(missionId, featureId);
    if (!existing) return undefined;

    const now = new Date().toISOString();
    const updated: Feature = {
      ...existing,
      ...(input.status !== undefined && { status: input.status }),
      ...(input.report !== undefined && { report: input.report }),
      updatedAt: now,
    };

    const validated = validateFeature(updated);
    await writeJson(this.featurePath(missionId, featureId), validated);
    return validated;
  }

  async list(
    missionId: string,
    filter?: { milestoneId?: string; status?: string },
  ): Promise<readonly Feature[]> {
    const dir = this.featuresDir(missionId);
    let featureIds: string[];

    try {
      const entries = await readdir(dir);
      featureIds = entries
        .filter((e) => e.endsWith(".json"))
        .map((e) => e.replace(".json", ""));
    } catch {
      return [];
    }

    const settled = await Promise.allSettled(
      featureIds.map((id) => this.get(missionId, id)),
    );
    let features = settled
      .filter((r): r is PromiseFulfilledResult<Feature | undefined> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter((f): f is Feature => f !== undefined);

    // Apply filters
    if (filter?.milestoneId) {
      features = features.filter((f) => f.milestoneId === filter.milestoneId);
    }
    if (filter?.status) {
      features = features.filter((f) => f.status === filter.status);
    }

      // Preserve plan order by listing oldest-created features first.
      // Fall back to feature ID when timestamps are identical.
      features.sort((a, b) => {
        const byCreatedAt = a.createdAt.localeCompare(b.createdAt);
        return byCreatedAt !== 0 ? byCreatedAt : a.id.localeCompare(b.id);
      });
      return features;
    }

  async getMany(missionId: string, featureIds: readonly string[]): Promise<readonly Feature[]> {
    const settled = await Promise.allSettled(
      featureIds.map((id) => this.get(missionId, id)),
    );
    return settled
      .filter((r): r is PromiseFulfilledResult<Feature | undefined> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter((f): f is Feature => f !== undefined);
  }
}
