// HandoffEmitterPort: a single point where task lifecycle verbs drop a
// launch envelope so the next process picking up the task has everything
// it needs (task id, agent assignment, worktree path, reason). Envelopes
// are immutable; pickup is recorded out-of-band via a sidecar so the
// envelope file is never rewritten.

export const HANDOFF_TRIGGERS = [
  "task:claim",
  "task:block",
  "task:abandon",
  "task:ship",
  "task:verify",
] as const;

export type HandoffTrigger = (typeof HANDOFF_TRIGGERS)[number];

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

export interface HandoffPickup {
  readonly id: string;
  readonly envelope_id: string;
  readonly picked_up_by: string;
  readonly picked_up_at: string;
  readonly note?: string;
}

export interface HandoffEmitterPort {
  emit(envelope: HandoffEnvelope): Promise<void>;
  list(): Promise<readonly HandoffEnvelope[]>;
  get(id: string): Promise<HandoffEnvelope | undefined>;
  markPickedUp(envelopeId: string, pickup: HandoffPickup): Promise<void>;
  getPickup(envelopeId: string): Promise<HandoffPickup | undefined>;
  listPickups(): Promise<readonly HandoffPickup[]>;
}
