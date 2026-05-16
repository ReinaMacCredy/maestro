import type { Principle } from "./types.js";

export const DEFAULT_PRINCIPLES: readonly Principle[] = [
  {
    id: "think-before-coding",
    name: "Think Before Coding",
    source: "karpathy",
    rule: "Before writing any code, articulate your assumptions and verify them against the specification. Write assumptions into the handoff so reviewers can challenge them.",
    profiles: ["planning", "implementation"],
    mode: "gate",
    gateField: "assumptions",
    gateCheck: "array_min_length:1",
  },
  {
    id: "simplicity-first",
    name: "Simplicity First",
    source: "karpathy",
    rule: "Prefer the simplest solution that satisfies the requirements. Avoid premature abstraction, speculative generality, and unnecessary indirection.",
    profiles: ["implementation", "simplify", "code-review"],
    mode: "advisory",
  },
  {
    id: "surgical-changes",
    name: "Surgical Changes",
    source: "karpathy",
    rule: "Make the smallest change that achieves the goal. Declare scope explicitly in the handoff so reviewers can verify nothing extra was touched.",
    profiles: ["implementation", "code-review", "simplify"],
    mode: "gate",
    gateField: "scopeDeclaration",
    gateCheck: "object_non_empty",
  },
  {
    id: "goal-driven-execution",
    name: "Goal-Driven Execution",
    source: "karpathy",
    rule: "Every change must be verified against the acceptance criteria. Report verification results in the handoff so the gate can confirm all checks passed.",
    profiles: ["implementation", "validation"],
    mode: "gate",
    gateField: "verificationResults",
    gateCheck: "array_all_passed",
  },
] as const;
