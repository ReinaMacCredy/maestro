import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { dirExists, fileExists } from "@/shared/lib/fs.js";
import type { ProcessRunnerPort } from "./process-runner.port.js";
import {
  WorktreeAlreadyExistsError,
  WorktreeCreateFailedError,
  type CreateWorktreeInput,
  type WorktreeRecord,
  type WorktreeStorePort,
} from "./worktree-store.port.js";

const DEFAULT_STATE_DIR = ".maestro/worktrees";
const DEFAULT_BRANCH_PREFIX = "feat";
const DEFAULT_BASE_BRANCH = "main";

export interface GitWorktreeStoreOptions {
  readonly repoRoot: string;
  readonly processRunner: ProcessRunnerPort;
  readonly stateDir?: string;
  readonly clock?: () => Date;
}

export class GitWorktreeStore implements WorktreeStorePort {
  readonly #repoRoot: string;
  readonly #runner: ProcessRunnerPort;
  readonly #stateDir: string;
  readonly #clock: () => Date;

  constructor(options: GitWorktreeStoreOptions) {
    this.#repoRoot = options.repoRoot;
    this.#runner = options.processRunner;
    this.#stateDir = join(options.repoRoot, options.stateDir ?? DEFAULT_STATE_DIR);
    this.#clock = options.clock ?? (() => new Date());
  }

  async create(input: CreateWorktreeInput): Promise<WorktreeRecord> {
    const existing = await this.get(input.task_id);
    if (existing) throw new WorktreeAlreadyExistsError(input.task_id, existing);

    const baseBranch = input.base_branch ?? DEFAULT_BASE_BRANCH;
    const branchPrefix = input.branch_prefix ?? DEFAULT_BRANCH_PREFIX;
    const branch = `${branchPrefix}/${input.slug}`;
    const parent = dirname(this.#repoRoot);
    const repoName = basename(this.#repoRoot);
    const path = join(parent, `${repoName}-${input.task_id}`);

    const cmd = `git -C ${shellQuote(this.#repoRoot)} worktree add -b ${shellQuote(
      branch,
    )} ${shellQuote(path)} ${shellQuote(baseBranch)}`;
    const result = await this.#runner.run(cmd);
    if (result.exitCode !== 0) {
      throw new WorktreeCreateFailedError(result.exitCode, result.stderr);
    }

    const record: WorktreeRecord = {
      task_id: input.task_id,
      slug: input.slug,
      path,
      branch,
      base_branch: baseBranch,
      created_at: this.#clock().toISOString(),
    };
    await this.#persist(record);
    return record;
  }

  async get(taskId: string): Promise<WorktreeRecord | undefined> {
    const path = this.#filePath(taskId);
    if (!(await fileExists(path))) return undefined;
    const raw = await readFile(path, "utf8");
    return JSON.parse(raw) as WorktreeRecord;
  }

  async list(): Promise<readonly WorktreeRecord[]> {
    if (!(await dirExists(this.#stateDir))) return [];
    const entries = await readdir(this.#stateDir);
    const files = entries.filter((e) => e.endsWith(".json"));
    const records: WorktreeRecord[] = [];
    for (const f of files) {
      const raw = await readFile(join(this.#stateDir, f), "utf8");
      records.push(JSON.parse(raw) as WorktreeRecord);
    }
    return records;
  }

  async #persist(record: WorktreeRecord): Promise<void> {
    await mkdir(this.#stateDir, { recursive: true });
    await writeFile(
      this.#filePath(record.task_id),
      `${JSON.stringify(record, null, 2)}\n`,
      "utf8",
    );
  }

  #filePath(taskId: string): string {
    return join(this.#stateDir, `${taskId}.json`);
  }
}

function shellQuote(s: string): string {
  return `'${s.replaceAll("'", "'\\''")}'`;
}
