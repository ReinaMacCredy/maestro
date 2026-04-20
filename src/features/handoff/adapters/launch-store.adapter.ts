import { readdir } from "node:fs/promises";
import { join } from "node:path";
import type { HandoffLaunchRecord, LaunchStorePort } from "../domain/launch-types.js";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { generateHandoffId, HANDOFF_ID_PATTERN } from "@/shared/domain/id.js";
import { assertSafeSegment } from "@/shared/lib/path-safety.js";
import { ensureDir, readJson, writeJson, writeText } from "@/shared/lib/fs.js";

const LAUNCHES_DIR = "launches";

export class FsLaunchStoreAdapter implements LaunchStorePort {
  constructor(private readonly projectDir: string) { }

  async create(input: {
    readonly task: string;
    readonly name: string;
    readonly provider: "codex" | "claude";
    readonly model: string;
    readonly wait: boolean;
    readonly sourceDir: string;
    readonly targetDir: string;
    readonly refs: HandoffLaunchRecord["refs"];
    readonly worktree?: HandoffLaunchRecord["worktree"];
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
      provider: input.provider,
      model: input.model,
      status: "launching",
      wait: input.wait,
      sourceDir: input.sourceDir,
      targetDir: input.targetDir,
      promptPath,
      outputPath,
      command: [],
      refs: input.refs,
      ...(input.worktree ? { worktree: input.worktree } : {}),
    };

    const launchDir = this.resolveLaunchDir(id);
    await ensureDir(launchDir);
    await writeText(join(this.projectDir, promptPath), input.prompt);
    await writeText(join(this.projectDir, outputPath), "");
    await writeJson(join(launchDir, "launch.json"), record);
    return record;
  }

  async update(record: HandoffLaunchRecord): Promise<HandoffLaunchRecord> {
    assertSafeSegment(record.id, "launch ID", HANDOFF_ID_PATTERN, "digits and dashes in YYYY-MM-DD-NNN format");
    await writeJson(join(this.resolveLaunchDir(record.id), "launch.json"), record);
    return record;
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
    try {
      const entries = await readdir(this.launchesDir(), { withFileTypes: true });
      return entries
        .filter((entry) => entry.isDirectory() && HANDOFF_ID_PATTERN.test(entry.name))
        .map((entry) => entry.name)
        .sort()
        .reverse();
    } catch {
      return [];
    }
  }

  private launchesDir(): string {
    return join(this.projectDir, MAESTRO_DIR, LAUNCHES_DIR);
  }

  private resolveLaunchDir(id: string): string {
    return join(this.launchesDir(), id);
  }
}
