import { createHash } from "node:crypto";

export interface TriggerRule {
  consequence: "DEGRADED" | "REJECT" | "REVIEW" | "STARTING" | "STOPPING";
  evidenceSource: string;
  id: string;
  minimumOccurrences: number;
  version: number;
}

export interface EvidencePacketInput {
  actor: string;
  authorityRef?: string;
  decisionRef?: string;
  evidence: string;
  excerpt?: string;
  generation: number;
  healthReceiptId: string;
  rule: TriggerRule;
  stopRef?: string;
  teamId: string;
  workRef?: string;
}

export interface BoundedEvidencePacket {
  actor: string;
  authorityRef: string | null;
  decisionRef: string | null;
  dedupeKey: string;
  evidence: string;
  excerpt: string;
  generation: number;
  healthReceiptId: string;
  ruleId: string;
  ruleVersion: number;
  stopRef: string | null;
  teamId: string;
  truncated: boolean;
  workRef: string | null;
}

export const coreTriggerRules: readonly TriggerRule[] = [
  {
    consequence: "STARTING",
    evidenceSource: "TeamRuntime readiness inspection",
    id: "mechanical.readiness-postcondition",
    minimumOccurrences: 1,
    version: 1,
  },
  {
    consequence: "DEGRADED",
    evidenceSource: "TeamRuntime required-resource inspection",
    id: "mechanical.required-resource",
    minimumOccurrences: 1,
    version: 1,
  },
  {
    consequence: "REJECT",
    evidenceSource: "Room-ledger generation and revision comparison",
    id: "mechanical.stale-ledger",
    minimumOccurrences: 1,
    version: 1,
  },
  {
    consequence: "STOPPING",
    evidenceSource: "TeamRuntime shutdown absence inspection",
    id: "mechanical.shutdown-leftover",
    minimumOccurrences: 1,
    version: 1,
  },
  {
    consequence: "REVIEW",
    evidenceSource: "same normalized failure signature inside one evidence window",
    id: "semantic.failure-third",
    minimumOccurrences: 3,
    version: 1,
  },
  {
    consequence: "REVIEW",
    evidenceSource: "claim paired with the cited Maestro status or work record",
    id: "semantic.status-contradiction",
    minimumOccurrences: 1,
    version: 1,
  },
  {
    consequence: "REVIEW",
    evidenceSource: "question ownership and responding role identity",
    id: "semantic.role-boundary",
    minimumOccurrences: 1,
    version: 1,
  },
  {
    consequence: "REVIEW",
    evidenceSource: "explicit stop condition and last observed progress time",
    id: "semantic.stop-silence",
    minimumOccurrences: 1,
    version: 1,
  },
  {
    consequence: "REVIEW",
    evidenceSource: "self-correction markers within one turn",
    id: "semantic.self-correction",
    minimumOccurrences: 2,
    version: 1,
  },
];

export const spotCheckRule: TriggerRule = {
  consequence: "REVIEW",
  evidenceSource: "one Supervisor question bound to one evidence window",
  id: "supervisor.spot-check",
  minimumOccurrences: 1,
  version: 1,
};

export function triggerRule(ruleId: string): TriggerRule | null {
  return coreTriggerRules.find((rule) => rule.id === ruleId) ?? null;
}

export function buildEvidencePacket(input: EvidencePacketInput): BoundedEvidencePacket {
  const evidence = input.evidence.slice(0, 4_096);
  const excerpt = (input.excerpt ?? "").slice(0, 8_192);
  const truncated = evidence.length !== input.evidence.length ||
    excerpt.length !== (input.excerpt ?? "").length;
  const dedupeKey = createHash("sha256")
    .update(JSON.stringify({
      actor: input.actor,
      authorityRef: input.authorityRef ?? null,
      decisionRef: input.decisionRef ?? null,
      evidence,
      excerpt,
      generation: input.generation,
      ruleId: input.rule.id,
      ruleVersion: input.rule.version,
      stopRef: input.stopRef ?? null,
      teamId: input.teamId,
      workRef: input.workRef ?? null,
    }))
    .digest("hex");
  return {
    actor: input.actor,
    authorityRef: input.authorityRef ?? null,
    decisionRef: input.decisionRef ?? null,
    dedupeKey,
    evidence,
    excerpt,
    generation: input.generation,
    healthReceiptId: input.healthReceiptId,
    ruleId: input.rule.id,
    ruleVersion: input.rule.version,
    stopRef: input.stopRef ?? null,
    teamId: input.teamId,
    truncated,
    workRef: input.workRef ?? null,
  };
}
