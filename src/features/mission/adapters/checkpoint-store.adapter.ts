/**
 * Filesystem adapter for checkpoint storage
 * Implements the CheckpointStorePort using timestamp-based filenames
 * Storage layout: .maestro/missions/{missionId}/checkpoints/{timestamp}.json
 */
import { join } from "node:path";
import type { Checkpoint } from "../domain/mission-types.js";
import type { CheckpointStorePort } from "../ports/checkpoint-store.port.js";
import { validateCheckpoint } from "../domain/mission-validators.js";
import { ensureDir, readJson, writeJson, listDirs } from "@/shared/lib/fs.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { readdir } from "node:fs/promises";

export class FsCheckpointStoreAdapter implements CheckpointStorePort {
  constructor(private readonly baseDir: string) {}

  private missionsRoot(): string {
    return join(this.baseDir, MAESTRO_DIR, "missions");
  }

  private missionDir(missionId: string): string {
    return join(this.missionsRoot(), missionId);
  }

  private checkpointsDir(missionId: string): string {
    return join(this.missionDir(missionId), "checkpoints");
  }

  private checkpointPath(missionId: string, checkpointId: string): string {
    return join(this.checkpointsDir(missionId), `${checkpointId}.json`);
  }

  /** Generate a timestamp-based checkpoint ID */
  private generateCheckpointId(): string {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, "0");
    const d = String(now.getDate()).padStart(2, "0");
    const h = String(now.getHours()).padStart(2, "0");
    const min = String(now.getMinutes()).padStart(2, "0");
    const s = String(now.getSeconds()).padStart(2, "0");
    const ms = String(now.getMilliseconds()).padStart(3, "0");
    return `${y}${m}${d}-${h}${min}${s}-${ms}`;
  }

  async get(missionId: string, checkpointId: string): Promise<Checkpoint | undefined> {
    const data = await readJson<unknown>(this.checkpointPath(missionId, checkpointId));
    if (!data) return undefined;
    try {
      return validateCheckpoint(data);
    } catch {
      return undefined;
    }
  }

  async save(
    missionId: string,
    data: Omit<Checkpoint, "id">,
  ): Promise<Checkpoint> {
    const id = this.generateCheckpointId();
    const checkpoint: Checkpoint = {
      ...data,
      id,
    };

    const validated = validateCheckpoint(checkpoint);
    await ensureDir(this.checkpointsDir(missionId));
    await writeJson(this.checkpointPath(missionId, id), validated);
    return validated;
  }

  async list(missionId: string): Promise<readonly Checkpoint[]> {
    const dir = this.checkpointsDir(missionId);
    let checkpointIds: string[];

    try {
      const entries = await readdir(dir);
      checkpointIds = entries
        .filter((e) => e.endsWith(".json"))
        .map((e) => e.replace(".json", ""))
        .sort()
        .reverse(); // Newest first
    } catch {
      return [];
    }

    const settled = await Promise.allSettled(
      checkpointIds.map((id) => this.get(missionId, id)),
    );
    return settled
      .filter((r): r is PromiseFulfilledResult<Checkpoint | undefined> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter((c): c is Checkpoint => c !== undefined);
  }

  async getLatest(missionId: string): Promise<Checkpoint | undefined> {
    const checkpoints = await this.list(missionId);
    return checkpoints[0];
  }

  async load(missionId: string): Promise<Checkpoint | undefined> {
    return this.getLatest(missionId);
  }
}
