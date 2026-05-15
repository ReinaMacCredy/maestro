import type {
  HandoffEmitterPort,
  HandoffEnvelope,
  HandoffTrigger,
} from "../repo/handoff-emitter.port.js";

export interface EmitHandoffDeps {
  readonly emitter?: HandoffEmitterPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface EmitHandoffInput {
  readonly task_id: string;
  readonly trigger_verb: HandoffTrigger;
  readonly agent_id?: string;
  readonly worktree_path?: string;
  readonly spec_path?: string;
  readonly reason?: string;
}

function defaultIdFactory(): string {
  return `hnd-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export async function emitHandoff(
  deps: EmitHandoffDeps,
  input: EmitHandoffInput,
): Promise<HandoffEnvelope | undefined> {
  if (!deps.emitter) return undefined;
  const envelope: HandoffEnvelope = {
    id: (deps.idFactory ?? defaultIdFactory)(),
    task_id: input.task_id,
    trigger_verb: input.trigger_verb,
    created_at: (deps.clock ?? (() => new Date()))().toISOString(),
    ...(input.agent_id !== undefined ? { agent_id: input.agent_id } : {}),
    ...(input.worktree_path !== undefined ? { worktree_path: input.worktree_path } : {}),
    ...(input.spec_path !== undefined ? { spec_path: input.spec_path } : {}),
    ...(input.reason !== undefined ? { reason: input.reason } : {}),
  };
  await deps.emitter.emit(envelope);
  return envelope;
}
