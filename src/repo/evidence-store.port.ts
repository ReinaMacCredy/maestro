// ADR-0009: every state transition writes one kind=transition evidence row.
// Ad-hoc evidence rows (lint-violation, command output, etc.) coexist in the
// same store keyed by `kind`.

import type { TaskState } from "../types/task-state.js";
import type { ExecPlanState } from "../types/exec-plan-state.js";

export interface TransitionEvidenceRow {
  readonly id: string;
  readonly kind: "transition";
  readonly timestamp: string;
  readonly task_id?: string;
  readonly plan_id?: string;
  readonly from_state: TaskState | ExecPlanState | null;
  readonly to_state: TaskState | ExecPlanState;
  readonly trigger_verb: string;
  readonly verdict?: "PASS" | "FAIL" | "HUMAN" | "BLOCK";
  readonly agent_id?: string;
  readonly reason?: string;
}

export interface LintViolationEvidenceRow {
  readonly id: string;
  readonly kind: "lint-violation";
  readonly timestamp: string;
  readonly task_id?: string;
  readonly rule_id: string;
  readonly severity: "error" | "warn" | "info";
  readonly file: string;
  readonly line?: number;
  readonly message: string;
  readonly remediation?: string;
}

export type EvidenceRow = TransitionEvidenceRow | LintViolationEvidenceRow;

export interface EvidenceFilter {
  readonly task_id?: string;
  readonly plan_id?: string;
  readonly kind?: EvidenceRow["kind"];
}

export interface EvidenceStorePort {
  append(row: EvidenceRow): Promise<void>;
  list(filter?: EvidenceFilter): Promise<readonly EvidenceRow[]>;
  read(id: string): Promise<EvidenceRow | undefined>;
}
