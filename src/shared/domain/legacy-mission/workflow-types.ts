import type { MilestoneKind, MilestoneProfile } from "./types.js";

/** A single phase in a workflow template */
export interface WorkflowPhase {
  readonly kind: MilestoneKind;
  readonly label: string;
  readonly profile?: MilestoneProfile;
  readonly description?: string;
}

/** Named workflow template -- a reusable milestone sequence */
export interface WorkflowTemplate {
  readonly description: string;
  readonly phases: readonly WorkflowPhase[];
}
