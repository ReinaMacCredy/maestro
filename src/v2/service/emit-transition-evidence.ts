import type { EvidenceStorePort, TransitionEvidenceRow } from "../repo/evidence-store.port.js";

export interface EmitTransitionEvidenceDeps {
  readonly store: EvidenceStorePort;
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
  return row;
}
