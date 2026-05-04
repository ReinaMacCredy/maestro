import type { Verdict, VerdictDecision } from "./domain/types.js";
import type { VerdictOverridePayload } from "@/features/evidence/index.js";

export function exitCodeForDecision(decision: VerdictDecision): number {
  switch (decision) {
    case "PASS": return 0;
    case "FAIL": return 1;
    case "HUMAN": return 2;
    case "BLOCK": return 3;
  }
}

export function printVerdict(
  verdict: Verdict,
  overrides?: readonly VerdictOverridePayload[],
): void {
  console.log(`Decision:   ${verdict.decision}`);
  console.log(`Risk:       ${verdict.effectiveRiskClass}${verdict.proposedRiskClass !== undefined ? ` (proposed: ${verdict.proposedRiskClass})` : ""}`);
  console.log(`ComputedAt: ${verdict.computedAt}`);
  console.log(`Task:       ${verdict.taskId}`);
  console.log(`ID:         ${verdict.id}`);
  if (verdict.reasons.length > 0) {
    console.log("Reasons:");
    for (const r of verdict.reasons) {
      console.log(`  [${r.category}] ${r.code}: ${r.message}`);
    }
  }
  console.log(`Evidence consulted: ${verdict.evidenceConsulted.length}`);
  if (verdict.policiesConsulted.length > 0) {
    const policyNames = verdict.policiesConsulted.map((p) => p.file).join(", ");
    console.log(`Policies consulted: ${policyNames}`);
  }
  console.log(`Trust verifier: ${verdict.trustVerifier.findingsCount} findings (${verdict.trustVerifier.errors} errors, ${verdict.trustVerifier.warns} warns, ${verdict.trustVerifier.infos} infos)`);
  if (overrides !== undefined && overrides.length > 0) {
    console.log(`Overrides (${overrides.length}):`);
    for (const ov of overrides) {
      console.log(`  Overridden by ${ov.overriddenBy}: ${ov.reason}`);
    }
  }
}
