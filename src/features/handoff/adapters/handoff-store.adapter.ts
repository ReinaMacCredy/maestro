import { basename, join } from "node:path";
import { open } from "node:fs/promises";
import type { HandoffAgent, HandoffRecord, HandoffRefs, HandoffStorePort, HandoffWorktree } from "../domain/handoff-types.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { generateHandoffId, HANDOFF_ID_PATTERN } from "@/shared/domain/id.js";
import { assertSafeSegment } from "@/shared/lib/path-safety.js";
import { ensureDir, listDirs, readJson, removeIfExists, writeJson, writeText } from "@/shared/lib/fs.js";
import { MaestroError } from "@/shared/errors.js";

export const HANDOFF_DIR = "handoff";
const PICKUP_LOCK_WAIT_MS = 2_000;
const PICKUP_LOCK_RETRY_MS = 20;

export class FsHandoffStoreAdapter implements HandoffStorePort {
  constructor(private readonly root: string) { }

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
  }): Promise<HandoffRecord> {
    const existingIds = await this.listIds();
    const id = generateHandoffId(existingIds, new Date());
    const createdAt = new Date().toISOString();
    const handoffDirRelative = join(MAESTRO_DIR, HANDOFF_DIR, id);
    const promptPath = join(handoffDirRelative, "prompt.md");
    const outputPath = join(handoffDirRelative, "output.log");
    const record: HandoffRecord = {
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

    const handoffDir = this.resolveHandoffDir(id);
    await ensureDir(handoffDir);
    await Promise.all([
      writeText(join(this.root, promptPath), input.prompt),
      writeText(join(this.root, outputPath), ""),
      writeJson(join(handoffDir, "handoff.json"), record),
    ]);
    return record;
  }

  async update(record: HandoffRecord): Promise<HandoffRecord> {
    assertSafeSegment(record.id, "handoff ID", HANDOFF_ID_PATTERN, "adjective-noun-N (e.g. swift-otter-3) or legacy YYYY-MM-DD-NNN");
    await writeJson(join(this.resolveHandoffDir(record.id), "handoff.json"), record);
    return record;
  }

  async consume(input: {
    readonly id: string;
    readonly agent: string;
    readonly sessionId?: string;
    readonly pickedUpAt: string;
  }): Promise<HandoffRecord> {
    assertSafeSegment(input.id, "handoff ID", HANDOFF_ID_PATTERN, "adjective-noun-N (e.g. swift-otter-3) or legacy YYYY-MM-DD-NNN");
    const lockPath = join(this.resolveHandoffDir(input.id), ".pickup.lock");
    const deadline = Date.now() + PICKUP_LOCK_WAIT_MS;

    while (true) {
      try {
        const handle = await open(lockPath, "wx");
        try {
          const current = await this.get(input.id);
          if (!current) {
            throw new MaestroError(`Handoff not found: ${input.id}`);
          }
          if (current.consumedAt) {
            throw new MaestroError(
              `Handoff ${input.id} was already consumed by ${current.pickedUpByAgent ?? "another agent"} at ${current.consumedAt}`,
            );
          }
          const updated: HandoffRecord = {
            ...current,
            status: "consumed",
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

  async get(id: string): Promise<HandoffRecord | undefined> {
    assertSafeSegment(id, "handoff ID", HANDOFF_ID_PATTERN, "adjective-noun-N (e.g. swift-otter-3) or legacy YYYY-MM-DD-NNN");
    const raw = await readJson<HandoffRecord>(join(this.resolveHandoffDir(id), "handoff.json"));
    return raw ? normalizeHandoffRecord(raw) : undefined;
  }

  async list(): Promise<readonly HandoffRecord[]> {
    const ids = await this.listIds();
    const records = await Promise.all(ids.map((id) => this.get(id)));
    return records.filter((record): record is HandoffRecord => record !== undefined);
  }

  resolveArtifactPath(relativePath: string): string {
    return join(this.root, relativePath);
  }

  private async listIds(): Promise<string[]> {
    const dirs = await listDirs(this.handoffDir());
    return dirs
      .map((dir) => basename(dir))
      .filter((name) => HANDOFF_ID_PATTERN.test(name))
      .sort()
      .reverse();
  }

  private handoffDir(): string {
    return join(this.root, MAESTRO_DIR, HANDOFF_DIR);
  }

  private resolveHandoffDir(id: string): string {
    return join(this.handoffDir(), id);
  }
}

// Packets consumed by pre-0.56 binaries wrote `consumedAt` but left
// `status: "launched"` on disk (the dedicated "consumed" status didn't exist
// yet). Project the true status at read time so `handoff show --json` and any
// downstream consumer of the record sees a single consistent lifecycle.
function normalizeHandoffRecord(record: HandoffRecord): HandoffRecord {
  if (record.consumedAt && record.status !== "consumed") {
    return { ...record, status: "consumed" };
  }
  return record;
}
