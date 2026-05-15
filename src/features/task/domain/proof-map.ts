import type { Spec } from "@/shared/domain/legacy-spec/index.js";
import type { Contract } from "@/features/task/index.js";
import type { EvidenceRow, EvidenceKind, WitnessLevel } from "@/features/evidence/index.js";

export interface ProofMapEvidence {
  readonly id: string;
  readonly kind: EvidenceKind;
  readonly witnessLevel: WitnessLevel;
  readonly createdAt: string;
}

export interface ProofMapEntry {
  readonly criterionId: string;
  readonly criterionText: string;
  readonly evidence: readonly ProofMapEvidence[];
  readonly covered: boolean;
}

export interface ProofMap {
  readonly taskId: string;
  readonly missionId?: string;
  readonly entries: readonly ProofMapEntry[];
  readonly uncoveredCount: number;
}

export function buildProofMap(args: {
  readonly taskId: string;
  readonly spec: Spec | undefined;
  readonly contract: Contract | undefined;
  readonly evidenceRows: readonly EvidenceRow[];
}): ProofMap {
  const criteria = args.spec?.acceptance_criteria ?? args.contract?.doneWhen ?? [];
  if (criteria.length === 0) {
    return { taskId: args.taskId, entries: [], uncoveredCount: 0 };
  }

  const evidenceByCriterion = new Map<string, ProofMapEvidence[]>();
  for (const row of args.evidenceRows) {
    const criterionId = extractCriterionId(row);
    if (criterionId === undefined) continue;
    const list = evidenceByCriterion.get(criterionId) ?? [];
    list.push({
      id: row.id,
      kind: row.kind,
      witnessLevel: row.witness_level,
      createdAt: row.created_at,
    });
    evidenceByCriterion.set(criterionId, list);
  }

  const entries: ProofMapEntry[] = criteria.map((c) => {
    const evidence = evidenceByCriterion.get(c.id) ?? [];
    return {
      criterionId: c.id,
      criterionText: c.text,
      evidence,
      covered: evidence.length > 0,
    };
  });
  const uncoveredCount = entries.filter((e) => !e.covered).length;
  return {
    taskId: args.taskId,
    missionId: args.spec?.mission_id,
    entries,
    uncoveredCount,
  };
}

function extractCriterionId(row: EvidenceRow): string | undefined {
  if (row.kind === "command" || row.kind === "manual-note" || row.kind === "ai-review" || row.kind === "threat-model") {
    return (row.payload as { criterion_id?: string }).criterion_id;
  }
  return undefined;
}
