import { describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerDeployGateCommand } from "@/features/deploy/index.js";
import { mockEvidenceStore, mockTaskStore } from "../../../../helpers/mocks.js";
import type { RecordEvidenceInput } from "@/features/evidence/index.js";
import type { EvidenceRow, DeployReadinessPayload } from "@/features/evidence/index.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";
import type { TaskStorePort } from "@/features/task/ports/task-store.port.js";
import type { Task } from "@/features/task";
import type { SpecStorePort } from "@/features/spec/index.js";
import type { Spec } from "@/features/spec/index.js";
import type { Owners } from "@/features/policy/index.js";

// ── Minimal task stub ──────────────────────────────────────────────────────
const STUB_TASK_NO_MISSION: Task = {
  id: "tsk-aaaaaa",
  title: "Stub task",
  type: "task",
  status: "in_progress",
  priority: 2,
  labels: [],
  blocks: [],
  blockedBy: [],
  createdAt: "2026-05-05T00:00:00.000Z",
  updatedAt: "2026-05-05T00:00:00.000Z",
};

const STUB_TASK_WITH_MISSION: Task = {
  ...STUB_TASK_NO_MISSION,
  missionId: "msn-aaaaaa",
};

// ── Spec with everything filled ────────────────────────────────────────────
const FULL_SPEC: Spec = {
  schema_version: 2,
  mission_id: "msn-aaaaaa",
  acceptance_criteria: [],
  non_goals: [],
  runtime_signals: [],
  rollout_plan: {
    feature_flag: "enable-new-feature",
    canary: {
      stages: [
        { percent: 10, hold_minutes: 30 },
        { percent: 100, hold_minutes: 0 },
      ],
    },
  },
  created_at: "2026-05-05T00:00:00.000Z",
  updated_at: "2026-05-05T00:00:00.000Z",
};

// ── Owners with and without deploy_approver ───────────────────────────────
const OWNERS_WITH_APPROVERS: Owners = {
  policyApprovers: [],
  ratchetApprovers: [],
  sensitiveWaivers: [],
  deployApprovers: ["alice"],
};

const OWNERS_EMPTY: Owners = {
  policyApprovers: [],
  ratchetApprovers: [],
  sensitiveWaivers: [],
  deployApprovers: [],
};

// ── Rollback evidence row (witnessed-by-ci) ───────────────────────────────
function makeRollbackRow(id = "evd-rollback01"): EvidenceRow {
  return {
    schema_version: 3,
    id,
    task_id: "tsk-aaaaaa",
    kind: "rollback-exercised",
    witness_level: "witnessed-by-ci",
    created_at: "2026-05-05T08:00:00.000Z",
    payload: { command: "kubectl rollout undo", exit: 0 },
  };
}

// ── Spec store mock ───────────────────────────────────────────────────────
function mockSpecStore(spec?: Spec): SpecStorePort {
  return {
    read: async (_missionId: string) => spec,
    write: async () => {},
    list: async () => (spec ? [spec] : []),
  };
}

// ── Program builder ───────────────────────────────────────────────────────
function makeProgram(opts: {
  evidenceStore: EvidenceStorePort;
  taskStore: TaskStorePort;
  spec?: Spec;
  owners?: Owners;
  isCI?: boolean;
}): Command {
  const {
    evidenceStore,
    taskStore,
    spec,
    owners = OWNERS_WITH_APPROVERS,
    isCI = false,
  } = opts;

  const program = new Command().exitOverride();
  const deployCmd = program
    .command("deploy")
    .description("Deploy safety commands");

  registerDeployGateCommand(deployCmd, program, {
    getServices: () => ({
      evidenceStore,
      taskStore,
      specStore: mockSpecStore(spec),
      projectRoot: "/test",
    }),
    recordEvidence: async (
      s: EvidenceStorePort,
      input: RecordEvidenceInput,
    ): Promise<EvidenceRow> => {
      const row: EvidenceRow = {
        schema_version: 3,
        id: "evd-gate-test01",
        task_id: input.task_id,
        kind: input.kind,
        witness_level: input.witness_level,
        created_at: "2026-05-05T09:00:00.000Z",
        payload: input.payload,
      };
      await s.append(row);
      return row;
    },
    loadOwnersFromBase: (_base: string, _projectRoot: string) => owners,
    resolveDefaultBase: async () => "main",
    isCI: () => isCI,
  });

  return program;
}

// ── Tests ─────────────────────────────────────────────────────────────────
describe("deploy gate command", () => {
  it("happy path — all checks pass → exits 0 and records deploy-readiness at agent-claimed-locally", async () => {
    const evidenceStore = mockEvidenceStore([makeRollbackRow()]);
    const taskStore = mockTaskStore([STUB_TASK_WITH_MISSION]);
    const program = makeProgram({ evidenceStore, taskStore, spec: FULL_SPEC });

    await program.parseAsync(["node", "maestro", "deploy", "gate", "--task", "tsk-aaaaaa"]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "deploy-readiness" });
    expect(rows).toHaveLength(1);

    const row = rows[0]!;
    expect(row.kind).toBe("deploy-readiness");
    expect(row.witness_level).toBe("agent-claimed-locally");

    const payload = row.payload as DeployReadinessPayload;
    expect(payload.gate).toBe("pass");
    expect(payload.checks.feature_flag.ok).toBe(true);
    expect(payload.checks.canary_plan.ok).toBe(true);
    expect(payload.checks.rollback.ok).toBe(true);
    expect(payload.checks.owner.ok).toBe(true);
    expect(payload.task_id).toBe("tsk-aaaaaa");
  });

  it("GITHUB_ACTIONS=true → witness is witnessed-by-ci", async () => {
    const evidenceStore = mockEvidenceStore([makeRollbackRow()]);
    const taskStore = mockTaskStore([STUB_TASK_WITH_MISSION]);
    const program = makeProgram({
      evidenceStore,
      taskStore,
      spec: FULL_SPEC,
      isCI: true,
    });

    await program.parseAsync(["node", "maestro", "deploy", "gate", "--task", "tsk-aaaaaa"]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "deploy-readiness" });
    expect(rows[0]!.witness_level).toBe("witnessed-by-ci");
  });

  it("missing rollback evidence → exits 1 and records gate=fail", async () => {
    const evidenceStore = mockEvidenceStore(); // no rollback rows
    const taskStore = mockTaskStore([STUB_TASK_WITH_MISSION]);
    const program = makeProgram({ evidenceStore, taskStore, spec: FULL_SPEC });

    let exitCode = 0;
    const originalExit = process.exit;
    (process as NodeJS.Process).exit = ((code?: number) => {
      exitCode = code ?? 0;
    }) as typeof process.exit;

    try {
      await program.parseAsync(["node", "maestro", "deploy", "gate", "--task", "tsk-aaaaaa"]);
    } finally {
      (process as NodeJS.Process).exit = originalExit;
    }

    expect(exitCode).toBe(1);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "deploy-readiness" });
    expect(rows).toHaveLength(1);
    const payload = rows[0]!.payload as DeployReadinessPayload;
    expect(payload.gate).toBe("fail");
    expect(payload.checks.rollback.ok).toBe(false);
  });

  it("empty deploy_approver → exits 1 and owner.ok=false", async () => {
    const evidenceStore = mockEvidenceStore([makeRollbackRow()]);
    const taskStore = mockTaskStore([STUB_TASK_WITH_MISSION]);
    const program = makeProgram({
      evidenceStore,
      taskStore,
      spec: FULL_SPEC,
      owners: OWNERS_EMPTY,
    });

    let exitCode = 0;
    const originalExit = process.exit;
    (process as NodeJS.Process).exit = ((code?: number) => {
      exitCode = code ?? 0;
    }) as typeof process.exit;

    try {
      await program.parseAsync(["node", "maestro", "deploy", "gate", "--task", "tsk-aaaaaa"]);
    } finally {
      (process as NodeJS.Process).exit = originalExit;
    }

    expect(exitCode).toBe(1);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "deploy-readiness" });
    const payload = rows[0]!.payload as DeployReadinessPayload;
    expect(payload.gate).toBe("fail");
    expect(payload.checks.owner.ok).toBe(false);
  });

  it("task without missionId → spec-derived checks fail; owner check can pass standalone", async () => {
    const evidenceStore = mockEvidenceStore([makeRollbackRow()]);
    const taskStore = mockTaskStore([STUB_TASK_NO_MISSION]); // no missionId
    const program = makeProgram({
      evidenceStore,
      taskStore,
      spec: FULL_SPEC, // spec won't be loaded (no missionId)
      owners: OWNERS_WITH_APPROVERS,
    });

    let exitCode = 0;
    const originalExit = process.exit;
    (process as NodeJS.Process).exit = ((code?: number) => {
      exitCode = code ?? 0;
    }) as typeof process.exit;

    try {
      await program.parseAsync(["node", "maestro", "deploy", "gate", "--task", "tsk-aaaaaa"]);
    } finally {
      (process as NodeJS.Process).exit = originalExit;
    }

    expect(exitCode).toBe(1);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa", kind: "deploy-readiness" });
    const payload = rows[0]!.payload as DeployReadinessPayload;
    expect(payload.gate).toBe("fail");
    expect(payload.checks.feature_flag.ok).toBe(false);
    expect(payload.checks.canary_plan.ok).toBe(false);
    // rollback and owner can still pass
    expect(payload.checks.rollback.ok).toBe(true);
    expect(payload.checks.owner.ok).toBe(true);
  });

  it("--json output includes gate, checks, and evidence_id", async () => {
    const evidenceStore = mockEvidenceStore([makeRollbackRow()]);
    const taskStore = mockTaskStore([STUB_TASK_WITH_MISSION]);
    const program = makeProgram({ evidenceStore, taskStore, spec: FULL_SPEC });

    const lines: string[] = [];
    const origLog = console.log;
    console.log = (msg: string) => lines.push(msg);

    try {
      await program.parseAsync(["node", "maestro", "deploy", "gate", "--task", "tsk-aaaaaa", "--json"]);
    } finally {
      console.log = origLog;
    }

    const output = JSON.parse(lines.join(""));
    expect(output.gate).toBe("pass");
    expect(output.evidence_id).toBe("evd-gate-test01");
    expect(output.checks.feature_flag.ok).toBe(true);
    expect(output.checks.canary_plan.ok).toBe(true);
    expect(output.checks.rollback.ok).toBe(true);
    expect(output.checks.owner.ok).toBe(true);
  });

  it("task not found → throws with task-not-found message", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([]);
    const program = makeProgram({ evidenceStore, taskStore });

    await expect(
      program.parseAsync(["node", "maestro", "deploy", "gate", "--task", "tsk-unknown"]),
    ).rejects.toThrow("Task not found");

    const rows = await evidenceStore.list({ task_id: "tsk-unknown" });
    expect(rows).toHaveLength(0);
  });
});
