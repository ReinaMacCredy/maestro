import { describe, expect, it } from "bun:test";
import { Command } from "commander";
import { registerDeployRollbackCommand } from "@/features/deploy/index.js";
import { mockEvidenceStore, mockTaskStore } from "../../../../helpers/mocks.js";
import type { RecordEvidenceInput } from "@/features/evidence/index.js";
import type { EvidenceRow, RollbackExercisedPayload } from "@/features/evidence/index.js";
import type { EvidenceStorePort } from "@/features/evidence/index.js";
import type { TaskStorePort, Task } from "@/shared/domain/task";

const STUB_TASK: Task = {
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

function makeProgram(opts: {
  evidenceStore: EvidenceStorePort;
  taskStore: TaskStorePort;
  spawnExit?: number;
  isCI?: boolean;
}): Command {
  const { evidenceStore, taskStore, spawnExit = 0, isCI = false } = opts;

  const program = new Command().exitOverride();

  const deployCmd = program
    .command("deploy")
    .description("Deploy safety commands");

  registerDeployRollbackCommand(deployCmd, program, {
    getServices: () => ({ legacyEvidenceStore: evidenceStore, legacyTaskStore: taskStore }),
    recordEvidence: async <K extends import("@/features/evidence/index.js").EvidenceKind>(
      s: EvidenceStorePort,
      input: RecordEvidenceInput<K>,
    ): Promise<EvidenceRow<K>> => {
      const row: EvidenceRow<K> = {
        schema_version: 3,
        id: "evd-test01",
        task_id: input.task_id,
        kind: input.kind,
        witness_level: input.witness_level,
        created_at: "2026-05-05T08:00:00.000Z",
        payload: input.payload,
      } as EvidenceRow<K>;
      await s.append(row);
      return row;
    },
    spawnSync: () => ({ exitCode: spawnExit }),
    isCI: () => isCI,
  });

  return program;
}

describe("deploy rollback command", () => {
  it("successful rollback exits 0 and writes rollback-exercised at witnessed-by-maestro", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const program = makeProgram({ evidenceStore, taskStore, spawnExit: 0, isCI: false });

    await program.parseAsync([
      "node", "maestro",
      "deploy", "rollback",
      "--task", "tsk-aaaaaa",
      "--command", "echo ok",
    ]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows).toHaveLength(1);

    const row = rows[0]!;
    expect(row.kind).toBe("rollback-exercised");
    expect(row.task_id).toBe("tsk-aaaaaa");
    expect(row.witness_level).toBe("witnessed-by-maestro");

    const payload = row.payload as RollbackExercisedPayload;
    expect(payload.command).toBe("echo ok");
    expect(payload.exit).toBe(0);
  });

  it("failed rollback still writes Evidence but exits 1", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const program = makeProgram({ evidenceStore, taskStore, spawnExit: 7 });

    let exited = false;
    let exitCode = 0;
    const originalExit = process.exit;
    (process as NodeJS.Process).exit = ((code?: number) => {
      exited = true;
      exitCode = code ?? 0;
    }) as typeof process.exit;

    try {
      await program.parseAsync([
        "node", "maestro",
        "deploy", "rollback",
        "--task", "tsk-aaaaaa",
        "--command", "bash -c 'exit 7'",
      ]);
    } finally {
      (process as NodeJS.Process).exit = originalExit;
    }

    expect(exited).toBe(true);
    expect(exitCode).toBe(1);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows).toHaveLength(1);

    const payload = rows[0]!.payload as RollbackExercisedPayload;
    expect(payload.exit).toBe(7);
  });

  it("uses witnessed-by-ci when GITHUB_ACTIONS=true", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([STUB_TASK]);
    const program = makeProgram({ evidenceStore, taskStore, spawnExit: 0, isCI: true });

    await program.parseAsync([
      "node", "maestro",
      "deploy", "rollback",
      "--task", "tsk-aaaaaa",
      "--command", "echo ok",
    ]);

    const rows = await evidenceStore.list({ task_id: "tsk-aaaaaa" });
    expect(rows).toHaveLength(1);
    expect(rows[0]!.witness_level).toBe("witnessed-by-ci");
  });

  it("rejects when task does not exist", async () => {
    const evidenceStore = mockEvidenceStore();
    const taskStore = mockTaskStore([]);
    const program = makeProgram({ evidenceStore, taskStore });

    await expect(
      program.parseAsync([
        "node", "maestro",
        "deploy", "rollback",
        "--task", "tsk-ffffff",
        "--command", "echo ok",
      ]),
    ).rejects.toThrow("Task not found");

    const rows = await evidenceStore.list({ task_id: "tsk-ffffff" });
    expect(rows).toHaveLength(0);
  });
});
