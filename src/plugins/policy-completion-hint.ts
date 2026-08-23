import type { PluginContext } from "../kernel/loader.ts";

const prefixedClaimProofGates = new Set(["policy-qa", "policy-tdd"]);

export function stackedClaimProofHint(context: PluginContext, workId: string): string {
  const activeGateCount = context.loader.records.filter(
    (record) => record.status === "active" && prefixedClaimProofGates.has(record.name),
  ).length;
  if (activeGateCount < 2) return "";
  return (
    `; multiple --claim/--proof pairs may be combined in one invocation, for example: ` +
    `maestro work done ${workId} ` +
    `--claim "test: <test claim>" --proof "<test output>" ` +
    `--claim "qa: <checked behavior>" --proof "<QA evidence>"`
  );
}
