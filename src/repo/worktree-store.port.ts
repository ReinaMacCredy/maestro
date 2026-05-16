// PD-3: worktree state is anchored in the primary repo's .maestro/worktrees/,
// not in each worktree's own .maestro/. This keeps the canonical record in one
// place even when an agent is working inside a worktree.

export interface WorktreeRecord {
  readonly task_id: string;
  readonly slug: string;
  readonly path: string;
  readonly branch: string;
  readonly base_branch: string;
  readonly created_at: string;
}

export interface CreateWorktreeInput {
  readonly task_id: string;
  readonly slug: string;
  readonly base_branch?: string;
  readonly branch_prefix?: string;
}

export interface WorktreeStorePort {
  create(input: CreateWorktreeInput): Promise<WorktreeRecord>;
  get(task_id: string): Promise<WorktreeRecord | undefined>;
  list(): Promise<readonly WorktreeRecord[]>;
}

export class WorktreeAlreadyExistsError extends Error {
  readonly task_id: string;
  readonly existing: WorktreeRecord;
  constructor(taskId: string, existing: WorktreeRecord) {
    super(`Worktree for task ${taskId} already exists at ${existing.path}`);
    this.name = "WorktreeAlreadyExistsError";
    this.task_id = taskId;
    this.existing = existing;
  }
}

export class WorktreeCreateFailedError extends Error {
  readonly exit_code: number;
  readonly stderr: string;
  constructor(exitCode: number, stderr: string) {
    super(`git worktree add failed (exit ${exitCode}): ${stderr.trim()}`);
    this.name = "WorktreeCreateFailedError";
    this.exit_code = exitCode;
    this.stderr = stderr;
  }
}
