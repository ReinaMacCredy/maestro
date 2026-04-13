import type { WorkflowTemplate } from "./workflow-types.js";

/** Built-in workflow templates */
export const BUILT_IN_WORKFLOWS: Readonly<Record<string, WorkflowTemplate>> = {
  "plan-implement": {
    description: "Standard planning then implementation",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "work", label: "Implementation", profile: "implementation" },
    ],
  },
  "plan-review-implement": {
    description: "Review gate before implementation",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "gate", label: "Plan Review", profile: "plan-review" },
      { kind: "work", label: "Implementation", profile: "implementation" },
    ],
  },
  "plan-implement-review": {
    description: "Post-implementation review",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "work", label: "Implementation", profile: "implementation" },
      { kind: "gate", label: "Code Review", profile: "code-review" },
    ],
  },
  "plan-review-implement-review": {
    description: "Review gates before and after implementation",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "gate", label: "Plan Review", profile: "plan-review" },
      { kind: "work", label: "Implementation", profile: "implementation" },
      { kind: "gate", label: "Code Review", profile: "code-review" },
    ],
  },
};
