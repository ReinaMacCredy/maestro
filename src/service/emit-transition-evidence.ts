import type { EvidenceStorePort, TransitionEvidenceRow } from "../repo/evidence-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";

export interface EmitTransitionEvidenceDeps {
  readonly store: EvidenceStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export type EmitTransitionEvidenceInput = Omit<TransitionEvidenceRow, "id" | "kind" | "timestamp">;

function defaultIdFactory(): string {
  return `evd-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export async function emitTransitionEvidence(
  deps: EmitTransitionEvidenceDeps,
  input: EmitTransitionEvidenceInput,
): Promise<TransitionEvidenceRow> {
  const clock = deps.clock ?? (() => new Date());
  const idFactory = deps.idFactory ?? defaultIdFactory;
  const row: TransitionEvidenceRow = {
    id: idFactory(),
    kind: "transition",
    timestamp: clock().toISOString(),
    ...input,
  };
  await deps.store.append(row);
  if (deps.observabilityStore !== undefined && row.task_id !== undefined) {
    await deps.observabilityStore.emit({
      task_id: row.task_id,
      kind: "transition",
      timestamp: row.timestamp,
      payload: {
        evidence_id: row.id,
        from_state: row.from_state,
        to_state: row.to_state,
        trigger_verb: row.trigger_verb,
        ...(row.verdict !== undefined ? { verdict: row.verdict } : {}),
        ...(row.reason !== undefined ? { reason: row.reason } : {}),
      },
    });
  }
  return row;
}
