import type {
  EvidenceStorePort,
  LintViolationEvidenceRow,
} from "../repo/evidence-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { TaskNotFoundError } from "../repo/task-store.port.js";
import type { ArchitectureRulesPort } from "../repo/architecture-rules.port.js";
import { assertTaskTransition } from "../types/task-state.js";
import type { Task, TaskId } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import {
  runArchitectureLints,
  type LintViolation,
} from "./architecture-lint.usecase.js";

export interface TaskVerifyDeps {
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly architectureRules: ArchitectureRulesPort;
  readonly repoRoot: string;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface TaskVerifyInput {
  readonly id: TaskId;
  // When set, skip running lints and record the explicit human verdict instead.
  // HUMAN keeps the task at verifying (awaiting human review); BLOCK transitions
  // verifying -> blocked. Both require a reason.
  readonly verdict?: "HUMAN" | "BLOCK";
  readonly reason?: string;
}

export interface TaskVerifyResult {
  readonly task: Task;
  readonly verdict: "PASS" | "FAIL" | "HUMAN" | "BLOCK";
  readonly violations: readonly LintViolation[];
}

export class TaskVerifyReasonRequiredError extends Error {
  readonly verdict: "HUMAN" | "BLOCK";
  constructor(verdict: "HUMAN" | "BLOCK") {
    super(`task verify --verdict ${verdict.toLowerCase()} requires --reason`);
    this.name = "TaskVerifyReasonRequiredError";
    this.verdict = verdict;
  }
}

function defaultIdFactory(): string {
  return `evd-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export async function taskVerify(
  deps: TaskVerifyDeps,
  input: TaskVerifyInput,
): Promise<TaskVerifyResult> {
  const existing = await deps.taskStore.get(input.id);
  if (!existing) throw new TaskNotFoundError(input.id);

  if (input.verdict !== undefined) {
    if (input.reason === undefined || input.reason.trim().length === 0) {
      throw new TaskVerifyReasonRequiredError(input.verdict);
    }
  }

  // Enter verifying. Accepts claimed | doing | verifying (re-run allowed).
  // Self-transition (verifying -> verifying) is not in TASK_TRANSITIONS, so
  // only assert for genuine state changes.
  if (existing.state !== "verifying") {
    assertTaskTransition(existing.state, "verifying");
  }
  const entered =
    existing.state === "verifying"
      ? existing
      : await deps.taskStore.update(input.id, { state: "verifying" });

  if (existing.state !== "verifying") {
    await emitTransitionEvidence(
      {
        store: deps.evidenceStore,
        observabilityStore: deps.observabilityStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      {
        task_id: existing.id,
        from_state: existing.state,
        to_state: "verifying",
        trigger_verb: "task:verify",
      },
    );
  }

  if (input.verdict === "HUMAN") {
    await emitTransitionEvidence(
      {
        store: deps.evidenceStore,
        observabilityStore: deps.observabilityStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      {
        task_id: existing.id,
        from_state: "verifying",
        to_state: "verifying",
        trigger_verb: "task:verify",
        verdict: "HUMAN",
        reason: input.reason,
      },
    );
    return { task: entered, verdict: "HUMAN", violations: [] };
  }

  if (input.verdict === "BLOCK") {
    const blocked = await deps.taskStore.update(input.id, {
      state: "blocked",
      block_reason: input.reason,
    });
    await emitTransitionEvidence(
      {
        store: deps.evidenceStore,
        observabilityStore: deps.observabilityStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      {
        task_id: existing.id,
        from_state: "verifying",
        to_state: "blocked",
        trigger_verb: "task:verify",
        verdict: "BLOCK",
        reason: input.reason,
      },
    );
    return { task: blocked, verdict: "BLOCK", violations: [] };
  }

  const report = await runArchitectureLints({
    repoRoot: deps.repoRoot,
    rulesPort: deps.architectureRules,
  });

  if (report.violations.length === 0) {
    const advanced = await deps.taskStore.update(input.id, { state: "ready" });
    await emitTransitionEvidence(
      {
        store: deps.evidenceStore,
        observabilityStore: deps.observabilityStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      {
        task_id: existing.id,
        from_state: "verifying",
        to_state: "ready",
        trigger_verb: "task:verify",
        verdict: "PASS",
      },
    );
    return { task: advanced, verdict: "PASS", violations: [] };
  }

  const clock = deps.clock ?? (() => new Date());
  const idFactory = deps.idFactory ?? defaultIdFactory;
  for (const v of report.violations) {
    const row: LintViolationEvidenceRow = {
      id: idFactory(),
      kind: "lint-violation",
      timestamp: clock().toISOString(),
      task_id: existing.id,
      rule_id: v.rule_id,
      severity: v.severity,
      file: v.file,
      line: v.line,
      message: v.message,
      remediation: v.remediation,
    };
    await deps.evidenceStore.append(row);
  }

  return { task: entered, verdict: "FAIL", violations: report.violations };
}
