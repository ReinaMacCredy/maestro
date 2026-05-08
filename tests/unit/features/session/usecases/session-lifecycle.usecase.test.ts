import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile, readFile, access } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { FsTaskContinuationStoreAdapter } from "@/features/task/adapters/fs-task-continuation-store.adapter.js";
import { FsTaskContinuationHistoryStoreAdapter } from "@/features/task/adapters/fs-task-continuation-history-store.adapter.js";
import { FsContractStoreAdapter } from "@/features/task/adapters/fs-contract-store.adapter.js";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import { FsRunStateStoreAdapter } from "@/features/task/adapters/fs-run-state-store.adapter.js";
import { FsEvidenceStoreAdapter } from "@/features/evidence";
import { FsSpecStoreAdapter } from "@/features/spec";
import type { Verdict, VerdictStorePort } from "@/features/verdict";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { sessionStart } from "@/features/session/usecases/session-start.usecase.js";
import { sessionExit } from "@/features/session/usecases/session-exit.usecase.js";

class StubVerdictStore implements VerdictStorePort {
  private latest = new Map<string, Verdict>();
  setLatest(taskId: string, v: Verdict): void {
    this.latest.set(taskId, v);
  }
  async write(taskId: string, v: Verdict): Promise<void> {
    this.latest.set(taskId, v);
  }
  async readLatest(taskId: string): Promise<Verdict | undefined> {
    return this.latest.get(taskId);
  }
  async readVersion(): Promise<Verdict | undefined> { return undefined; }
  async history(): Promise<readonly Verdict[]> { return []; }
  async findByTreeSha(): Promise<readonly Verdict[]> { return []; }
}

interface Fixtures {
  tmpDir: string;
  taskStore: JsonlTaskStoreAdapter;
  evidenceStore: FsEvidenceStoreAdapter;
  verdictStore: StubVerdictStore;
  composeDeps: () => Parameters<typeof sessionStart>[0];
  exitDeps: () => Parameters<typeof sessionExit>[0];
}

let f: Fixtures;

beforeEach(async () => {
  const tmpDir = await mkdtemp(join(tmpdir(), "session-lc-"));
  await mkdir(join(tmpDir, "src"), { recursive: true });
  const taskStore = new JsonlTaskStoreAdapter(tmpDir);
  const continuationStore = new FsTaskContinuationStoreAdapter(tmpDir);
  const continuationHistory = new FsTaskContinuationHistoryStoreAdapter(tmpDir);
  const evidenceStore = new FsEvidenceStoreAdapter(tmpDir);
  const specStore = new FsSpecStoreAdapter(tmpDir);
  const verdictStore = new StubVerdictStore();
  const runStateStore = new FsRunStateStoreAdapter(tmpDir);
  const contractStore = new FsContractStoreAdapter(tmpDir);
  const contractVersionStore = new FsContractVersionStoreAdapter(tmpDir);
  f = {
    tmpDir,
    taskStore,
    evidenceStore,
    verdictStore,
    composeDeps: () => ({
      taskStore,
      continuationStore,
      continuationHistory,
      specStore,
      verdictStore,
      evidenceStore,
      runStateStore,
      contractStore,
      contractVersionStore,
      repoRoot: tmpDir,
      resolveCommitsSince: async () => [],
      resolveHeadSha: async () => "deadbeef0000",
      runScript: async () => ({ exitCode: 0, stdout: "", stderr: "" }),
    }),
    exitDeps: () => ({
      evidenceStore,
      verdictStore,
      checkDirtyTree: async () => false,
    }),
  };
});

async function exists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

describe("sessionStart", () => {
  it("throws when task does not exist", async () => {
    expect.assertions(1);
    try {
      await sessionStart(f.composeDeps(), {
        taskId: "tsk-deadbe",
        projectRoot: f.tmpDir,
      });
    } catch (err) {
      expect((err as Error).message).toContain("not found");
    }
  });

  it("writes orient.md and records session-start evidence", async () => {
    const task = await createTask(f.taskStore, { title: "Open me" });
    const result = await sessionStart(f.composeDeps(), {
      taskId: task.id,
      projectRoot: f.tmpDir,
    });
    expect(await exists(result.orientPath)).toBe(true);
    const body = await readFile(result.orientPath, "utf8");
    expect(body).toContain(`# Task: ${task.id}`);
    expect(result.headSha).toBe("deadbeef0000");

    const evidenceRows = await f.evidenceStore.list({ task_id: task.id, kind: "session-start" });
    expect(evidenceRows.length).toBe(1);
    expect(evidenceRows[0]!.witness_level).toBe("witnessed-by-maestro");
    expect((evidenceRows[0]!.payload as { headSha: string }).headSha).toBe("deadbeef0000");
  });

  it("blocks (no orient, no evidence) when baseline arch lint fails", async () => {
    const task = await createTask(f.taskStore, { title: "Block me" });
    // Plant a runner-inversion violation so baseline arch lint fails.
    await writeFile(
      join(f.tmpDir, "src/danger.ts"),
      `Bun.spawn(["claude", "--help"]);`,
    );

    let threwExpected = false;
    try {
      await sessionStart(f.composeDeps(), {
        taskId: task.id,
        projectRoot: f.tmpDir,
      });
    } catch (err) {
      threwExpected = true;
      expect((err as Error & { code?: string }).code).toBe(
        "session-start-baseline-blocked",
      );
    }
    expect(threwExpected).toBe(true);

    const orientPath = join(f.tmpDir, ".maestro", "runs", task.id, "orient.md");
    expect(await exists(orientPath)).toBe(false);
    const evidenceRows = await f.evidenceStore.list({ task_id: task.id, kind: "session-start" });
    expect(evidenceRows.length).toBe(0);
  });
});

describe("sessionExit", () => {
  async function makeTask() {
    return createTask(f.taskStore, { title: "Close me" });
  }

  it("returns exitCode=0 with no warnings on clean run", async () => {
    const task = await makeTask();
    const result = await sessionExit(f.exitDeps(), {
      taskId: task.id,
      projectRoot: f.tmpDir,
    });
    expect(result.exitCode).toBe(0);
    expect(result.summary.lintViolations).toBe(0);
    expect(result.summary.baselineClean).toBe(true);
    expect(result.warnings).toEqual([]);
    const rows = await f.evidenceStore.list({ task_id: task.id, kind: "session-exit" });
    expect(rows.length).toBe(1);
    expect(rows[0]!.witness_level).toBe("witnessed-by-maestro");
  });

  it("returns exitCode=2 when arch-lint violations are present", async () => {
    const task = await makeTask();
    await writeFile(
      join(f.tmpDir, "src/danger.ts"),
      `Bun.spawn(["claude", "--help"]);`,
    );
    const result = await sessionExit(f.exitDeps(), {
      taskId: task.id,
      projectRoot: f.tmpDir,
    });
    expect(result.exitCode).toBe(2);
    expect(result.summary.lintViolations).toBeGreaterThanOrEqual(1);
    expect(result.summary.baselineClean).toBe(false);
  });

  it("warns but does not block on dirty working tree", async () => {
    const task = await makeTask();
    const result = await sessionExit(
      {
        ...f.exitDeps(),
        checkDirtyTree: async () => true,
      },
      { taskId: task.id, projectRoot: f.tmpDir },
    );
    expect(result.exitCode).toBe(0);
    expect(result.summary.dirtyTree).toBe(true);
    expect(result.warnings.some((w) => w.includes("uncommitted"))).toBe(true);
  });

  it("warns but does not block on FAIL verdict", async () => {
    const task = await makeTask();
    const verdict: Verdict = {
      schemaVersion: 1,
      id: "vrd-1234567890123-abcdef",
      taskId: task.id,
      contractVersion: 1,
      computedAt: "2026-05-01T00:00:00.000Z",
      decision: "FAIL",
      effectiveRiskClass: "low",
      reasons: [],
      evidenceConsulted: [],
      policiesConsulted: [],
      trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    };
    f.verdictStore.setLatest(task.id, verdict);
    const result = await sessionExit(f.exitDeps(), {
      taskId: task.id,
      projectRoot: f.tmpDir,
    });
    expect(result.exitCode).toBe(0);
    expect(result.summary.verdictDecision).toBe("FAIL");
    expect(result.warnings.some((w) => w.includes("FAIL"))).toBe(true);
  });

  it("writes progress.md with summary lines", async () => {
    const task = await makeTask();
    const result = await sessionExit(f.exitDeps(), {
      taskId: task.id,
      projectRoot: f.tmpDir,
    });
    expect(await exists(result.progressPath)).toBe(true);
    const body = await readFile(result.progressPath, "utf8");
    expect(body).toContain(`Session exit progress — task ${task.id}`);
    expect(body).toContain("Lint violations (error):");
  });
});
