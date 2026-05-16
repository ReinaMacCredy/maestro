// HandoffEmitterPort: a single point where task lifecycle verbs drop a
// launch envelope so the next process picking up the task has everything
// it needs (task id, agent assignment, worktree path, reason). Write-only:
// emit() never replays envelopes back into the lifecycle.

export type HandoffTrigger =
  | "task:claim"
  | "task:block"
  | "task:abandon"
  | "task:ship"
  | "task:verify";

export interface HandoffEnvelope {
  readonly id: string;
  readonly task_id: string;
  readonly trigger_verb: HandoffTrigger;
  readonly created_at: string;
  readonly agent_id?: string;
  readonly worktree_path?: string;
  readonly spec_path?: string;
  readonly reason?: string;
}

export interface HandoffEmitterPort {
  emit(envelope: HandoffEnvelope): Promise<void>;
  list(): Promise<readonly HandoffEnvelope[]>;
  get(id: string): Promise<HandoffEnvelope | undefined>;
}
