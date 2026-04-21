import { basename, join } from "node:path";
import { open } from "node:fs/promises";
import type { HandoffAgent, HandoffLaunchRecord, HandoffRefs, HandoffWorktree, LaunchStorePort } from "../domain/launch-types.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { generateHandoffId, HANDOFF_ID_PATTERN } from "@/shared/domain/id.js";
import { assertSafeSegment } from "@/shared/lib/path-safety.js";
import { ensureDir, listDirs, readJson, removeIfExists, writeJson, writeText } from "@/shared/lib/fs.js";
import { MaestroError } from "@/shared/errors.js";

const LAUNCHES_DIR = "launches";
const PICKUP_LOCK_WAIT_MS = 2_000;
const PICKUP_LOCK_RETRY_MS = 20;

export class FsLaunchStoreAdapter implements LaunchStorePort {
  constructor(private readonly projectDir: string) { }

  async create(input: {
    readonly task: string;
    readonly name: string;
    readonly agent: HandoffAgent;
    readonly model: string;
    readonly wait: boolean;
    readonly sourceDir: string;
    readonly targetDir: string;
    readonly refs: HandoffRefs;
    readonly createdByAgent?: string;
    readonly createdBySessionId?: string;
    readonly worktree?: HandoffWorktree;
    readonly prompt: string;
  }): Promise<HandoffLaunchRecord> {
    const existingIds = await this.listIds();
    const id = generateHandoffId(existingIds, new Date());
    const createdAt = new Date().toISOString();
    const launchDirRelative = join(MAESTRO_DIR, LAUNCHES_DIR, id);
    const promptPath = join(launchDirRelative, "prompt.md");
    const outputPath = join(launchDirRelative, "output.log");
    const record: HandoffLaunchRecord = {
      id,
      createdAt,
      task: input.task,
      name: input.name,
      agent: input.agent,
      model: input.model,
      status: "launching",
      wait: input.wait,
      sourceDir: input.sourceDir,
      targetDir: input.targetDir,
      promptPath,
      outputPath,
      command: [],
      refs: input.refs,
      ...(input.createdByAgent ? { createdByAgent: input.createdByAgent } : {}),
      ...(input.createdBySessionId ? { createdBySessionId: input.createdBySessionId } : {}),
      ...(input.worktree ? { worktree: input.worktree } : {}),
    };

    const launchDir = this.resolveLaunchDir(id);
    await ensureDir(launchDir);
    await Promise.all([
      writeText(join(this.projectDir, promptPath), input.prompt),
      writeText(join(this.projectDir, outputPath), ""),
      writeJson(join(launchDir, "launch.json"), record),
    ]);
    return record;
  }

  async update(record: HandoffLaunchRecord): Promise<HandoffLaunchRecord> {
    assertSafeSegment(record.id, "launch ID", HANDOFF_ID_PATTERN, "digits and dashes in YYYY-MM-DD-NNN format");
    await writeJson(join(this.resolveLaunchDir(record.id), "launch.json"), record);
    return record;
  }

  async consume(input: {
    readonly id: string;
    readonly agent: string;
    readonly sessionId?: string;
    readonly pickedUpAt: string;
  }): Promise<HandoffLaunchRecord> {
    assertSafeSegment(input.id, "launch ID", HANDOFF_ID_PATTERN, "digits and dashes in YYYY-MM-DD-NNN format");
    const lockPath = join(this.resolveLaunchDir(input.id), ".pickup.lock");
    const deadline = Date.now() + PICKUP_LOCK_WAIT_MS;

    while (true) {
      try {
        const handle = await open(lockPath, "wx");
        try {
          const current = await this.get(input.id);
          if (!current) {
            throw new MaestroError(`Handoff launch not found: ${input.id}`);
          }
          if (current.consumedAt) {
            throw new MaestroError(
              `Handoff ${input.id} was already consumed by ${current.pickedUpByAgent ?? "another agent"} at ${current.consumedAt}`,
            );
          }
          const updated: HandoffLaunchRecord = {
            ...current,
            pickedUpByAgent: input.agent,
            ...(input.sessionId ? { pickedUpBySessionId: input.sessionId } : {}),
            pickedUpAt: input.pickedUpAt,
            consumedAt: input.pickedUpAt,
          };
          await this.update(updated);
          return updated;
        } finally {
          await handle.close();
          await removeIfExists(lockPath);
        }
      } catch (error) {
        const errno = error as NodeJS.ErrnoException;
        if (errno.code !== "EEXIST") {
          throw error;
        }
        if (Date.now() >= deadline) {
          throw new MaestroError(`Handoff pickup is already in progress for ${input.id}`, [
            "Retry once the other pickup attempt finishes",
          ]);
        }
        await Bun.sleep(PICKUP_LOCK_RETRY_MS);
      }
    }
  }

  async get(id: string): Promise<HandoffLaunchRecord | undefined> {
    assertSafeSegment(id, "launch ID", HANDOFF_ID_PATTERN, "digits and dashes in YYYY-MM-DD-NNN format");
    return readJson<HandoffLaunchRecord>(join(this.resolveLaunchDir(id), "launch.json"));
  }

  async list(): Promise<readonly HandoffLaunchRecord[]> {
    const ids = await this.listIds();
    const records = await Promise.all(ids.map((id) => this.get(id)));
    return records.filter((record): record is HandoffLaunchRecord => record !== undefined);
  }

  resolveArtifactPath(relativePath: string): string {
    return join(this.projectDir, relativePath);
  }

  private async listIds(): Promise<string[]> {
    const dirs = await listDirs(this.launchesDir());
    return dirs
      .map((dir) => basename(dir))
      .filter((name) => HANDOFF_ID_PATTERN.test(name))
      .sort()
      .reverse();
  }

  private launchesDir(): string {
    return join(this.projectDir, MAESTRO_DIR, LAUNCHES_DIR);
  }

  private resolveLaunchDir(id: string): string {
    return join(this.launchesDir(), id);
  }
}
