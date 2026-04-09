/**
 * Filesystem adapter for mission storage
 * Implements the MissionStorePort using atomic file writes
 * Storage layout: .maestro/missions/{id}/
 */
import { join } from "node:path";
import type { Mission, CreateMissionInput, UpdateMissionInput } from "../domain/mission-types.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import { validateMission } from "../domain/mission-validators.js";
import { MaestroError } from "../domain/errors.js";
import { ensureDir, readJson, writeJson, dirExists, listDirs } from "../lib/fs.js";
import { MAESTRO_DIR } from "../domain/defaults.js";

export class FsMissionStoreAdapter implements MissionStorePort {
  constructor(private readonly baseDir: string) {}

  private missionsRoot(): string {
    return join(this.baseDir, MAESTRO_DIR, "missions");
  }

  private missionDir(id: string): string {
    return join(this.missionsRoot(), id);
  }

  private missionPath(id: string): string {
    return join(this.missionDir(id), "mission.json");
  }

  private stagingDir(id: string): string {
    return join(this.missionsRoot(), `.staging-${id}`);
  }

  private stagingMissionPath(id: string): string {
    return join(this.stagingDir(id), "mission.json");
  }

  async listIds(): Promise<readonly string[]> {
    const dirs = await listDirs(this.missionsRoot());
    return dirs
      .map((d) => d.split("/").pop() || "")
      .filter((id) => !id.startsWith(".") && id.length > 0)
      .sort()
      .reverse();
  }

  /** Remove orphaned .staging-* directories left by crashed creates */
  async cleanOrphanedStaging(): Promise<number> {
    const root = this.missionsRoot();
    const { readdir, rm } = await import("node:fs/promises");
    let cleaned = 0;
    try {
      const entries = await readdir(root);
      for (const entry of entries) {
        if (entry.startsWith(".staging-")) {
          await rm(join(root, entry), { recursive: true, force: true });
          cleaned++;
        }
      }
    } catch {
      // missions root may not exist yet
    }
    return cleaned;
  }

  async get(id: string): Promise<Mission | undefined> {
    const data = await readJson<unknown>(this.missionPath(id));
    if (!data) return undefined;
    try {
      return validateMission(data);
    } catch {
      return undefined;
    }
  }

  async exists(id: string): Promise<boolean> {
    return await dirExists(this.missionDir(id));
  }

  async stage(input: CreateMissionInput, id: string, features: readonly string[]): Promise<string> {
    const now = new Date().toISOString();
    const mission: Mission = {
      id,
      status: "draft",
      title: input.title,
      description: input.description,
      ...(input.proposal !== undefined && { proposal: input.proposal }),
      milestones: input.milestones.map((milestone) => ({
        ...milestone,
        featureIds: [],
      })),
      features,
      createdAt: now,
      updatedAt: now,
    };

    const dir = this.stagingDir(id);
    await ensureDir(dir);
    await writeJson(this.stagingMissionPath(id), mission);
    return id;
  }

  async finalize(id: string): Promise<void> {
    const stagingPath = this.stagingDir(id);
    const finalPath = this.missionDir(id);

    // Move from staging to final location with error handling
    const { rename, rm } = await import("node:fs/promises");
    try {
      await rename(stagingPath, finalPath);
    } catch (err: unknown) {
      const code = (err as NodeJS.ErrnoException).code;
      if (code === "ENOTEMPTY" || code === "EEXIST") {
        await rm(stagingPath, { recursive: true, force: true });
        throw new MaestroError(`Mission ${id} already exists`, [
          "Use a different mission ID or delete the existing mission",
          `Mission directory: ${finalPath}`,
        ]);
      }
      throw err;
    }

    // Create subdirectories for features, workers, reports, checkpoints
    await ensureDir(join(finalPath, "features"));
    await ensureDir(join(finalPath, "workers"));
    await ensureDir(join(finalPath, "checkpoints"));
  }

  async update(id: string, input: UpdateMissionInput): Promise<Mission | undefined> {
    const existing = await this.get(id);
    if (!existing) return undefined;

    const now = new Date().toISOString();
    const updated: Mission = {
      ...existing,
      ...(input.title !== undefined && { title: input.title }),
      ...(input.description !== undefined && { description: input.description }),
      ...(input.status !== undefined && { status: input.status }),
      ...(input.completedMilestoneIds !== undefined && { completedMilestoneIds: input.completedMilestoneIds }),
      updatedAt: now,
      ...(input.status === "approved" && { approvedAt: now }),
      ...(input.status === "rejected" && { rejectedAt: now }),
      ...(input.status === "completed" && { completedAt: now }),
    };

    const validated = validateMission(updated);
    await writeJson(this.missionPath(id), validated);
    return validated;
  }

  async list(): Promise<readonly Mission[]> {
    const ids = await this.listIds();
    const settled = await Promise.allSettled(ids.map((id) => this.get(id)));
    const missions = settled
      .filter((r): r is PromiseFulfilledResult<Mission | undefined> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter((m): m is Mission => m !== undefined);

    // Sort by creation date, newest first
    missions.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
    return missions;
  }
}
