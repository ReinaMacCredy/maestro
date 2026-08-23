import type { PluginContext } from "../kernel/loader.ts";

const prefixedCompletionGates = [
  {
    name: "policy-tdd",
    claim: "test: <test claim>",
    proof: "<test output>",
  },
  {
    name: "policy-qa",
    claim: "qa: <checked behavior>",
    proof: "<QA evidence>",
  },
] as const;

type PrefixedCompletionGate = (typeof prefixedCompletionGates)[number];
type PrefixedCompletionGateName = PrefixedCompletionGate["name"];

function completionCommand(workId: string, gates: readonly PrefixedCompletionGate[]): string {
  const pairs = gates
    .map((gate) => `--claim "${gate.claim}" --proof "${gate.proof}"`)
    .join(" ");
  return `maestro work done ${workId} ${pairs}`;
}

export function prefixedCompletionGateReason(
  context: PluginContext,
  workId: string,
  gateName: PrefixedCompletionGateName,
  requirement: string,
): string {
  const gate = prefixedCompletionGates.find((candidate) => candidate.name === gateName);
  if (!gate) throw new Error(`unknown prefixed completion gate: ${gateName}`);
  const activeNames = new Set(
    context.loader.records
      .filter((record) => record.status === "active")
      .map((record) => record.name),
  );
  const activeGates = prefixedCompletionGates.filter((candidate) => activeNames.has(candidate.name));
  const stackedHint =
    activeGates.length < 2
      ? ""
      : `; multiple --claim/--proof pairs may be combined in one invocation, for example: ${completionCommand(workId, activeGates)}`;
  return `${requirement}; run: ${completionCommand(workId, [gate])}${stackedHint}`;
}
