import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { buildStatusReport } from "@/infra/usecases/build-status-report.usecase.js";
import {
  mockMissionStore,
  mockRepoTaskStore,
  mockRepoEvidenceStore,
  mockVerdictStore,
  mockHandoffEmitter,
} from "../../../helpers/mocks.js";
import type { Mission } from "@/shared/domain/legacy-mission";
import type { Task } from "@/types/task.js";
import type { Verdict } from "@/features/verdict/domain/types.js";
import type { HandoffEnvelope, HandoffPickup } from "@/repo/handoff-emitter.port.js";

let cwd: string;

beforeEach(async () => {
  cwd = await mkdtemp(join(tmpdir(), "maestro-build-status-"));
  await mkdir(join(cwd, ".maestro"), { recursive: true });
});

afterEach(async () => {
  await rm(cwd, { recursive: true, force: true });
});

function baseDeps(projectDir: string) {
  return {
    taskStore: mockRepoTaskStore(),
    featureMissionStore: mockMissionStore(),
    verdictStore: mockVerdictStore(),
    evidenceStore: mockRepoEvidenceStore(),
    handoffEmitter: mockHandoffEmitter(),
    projectDir,
  };
}

function makeTask(over: Partial<Task> & Pick<Task, "id" | "slug" | "title" | "state">): Task {
  const now = "2026-05-18T00:00:00.000Z";
  return {
    blocked_by: [],
    created_at: now,
    updated_at: now,
    ...over,
  };
}

function makeMission(over: Partial<Mission> & Pick<Mission, "id" | "status">): Mission {
  const now = "2026-05-18T00:00:00.000Z";
  return {
    title: `Mission ${over.id}`,
    description: "",
    milestones: [],
    features: [],
    createdAt: now,
    updatedAt: now,
    ...over,
  };
}

function makeVerdict(over: Pick<Verdict, "id" | "taskId" | "decision" | "computedAt">): Verdict {
  return {
    schemaVersion: 1,
    contractVersion: 1,
    effectiveRiskClass: "low",
    reasons: [],
    evidenceConsulted: [],
    policiesConsulted: [],
    trustVerifier: { findingsCount: 0, errors: 0, warns: 0, infos: 0 },
    ...over,
  };
}

describe("buildStatusReport", () => {
  it("returns the five top-level sections in the locked order", async () => {
    const report = await buildStatusReport(baseDeps(cwd));

    expect(Object.keys(report)).toEqual([
      "maestro_health",
      "project_state",
      "missions",
      "next_ready",
      "recent_transitions",
    ]);
  });

  it("emits empty-state hints under each empty section", async () => {
    const report = await buildStatusReport(baseDeps(cwd));

    expect(report.missions).toEqual([]);
    expect(report.next_ready).toBeUndefined();
    expect(report.recent_transitions).toEqual([]);
    expect(report.project_state.stuck_verifying_count).toBe(0);
    expect(report.project_state.stale_handoff_count).toBe(0);
    expect(report.project_state.latest_verdict).toBeUndefined();
  });

  it("project_state has stable JSON keys", async () => {
    const report = await buildStatusReport(baseDeps(cwd));

    expect(Object.keys(report.project_state).sort()).toEqual([
      "latest_verdict",
      "stale_handoff_count",
      "stuck_verifying_count",
    ]);
  });

  it("terse mode collapses maestro_health to a SetupCheckEntry array; recent_transitions stays a stable array", async () => {
    const report = await buildStatusReport({ ...baseDeps(cwd), terse: true });

    expect(Array.isArray(report.maestro_health)).toBe(true);
    expect(Array.isArray(report.recent_transitions)).toBe(true);
  });

  it("hard-refuses when .maestro/ directory is missing", async () => {
    await rm(join(cwd, ".maestro"), { recursive: true });
    await expect(buildStatusReport(baseDeps(cwd))).rejects.toThrow(/not initialized/i);
  });

  it("filters non-active missions and buckets tasks of non-active missions as unscoped", async () => {
    const activeMission = makeMission({ id: "mis-active", status: "executing" });
    const inactiveMission = makeMission({ id: "mis-done", status: "completed" });
    const tasks = [
      makeTask({ id: "tsk-1", slug: "do-a", title: "A", state: "ready", mission_id: "mis-active" }),
      makeTask({ id: "tsk-2", slug: "do-b", title: "B", state: "ready", mission_id: "mis-done" }),
      makeTask({ id: "tsk-3", slug: "do-c", title: "C", state: "draft" }),
    ];

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
      featureMissionStore: mockMissionStore([activeMission, inactiveMission]),
    });

    expect(report.missions).toHaveLength(2);
    const activeGroup = report.missions[0];
    if (!activeGroup) throw new Error("missing active group");
    expect("synthetic" in activeGroup.mission).toBe(false);
    if ("synthetic" in activeGroup.mission) throw new Error("unreachable");
    expect(activeGroup.mission.id).toBe("mis-active");
    expect(activeGroup.tasks.map((t) => t.task.id)).toEqual(["tsk-1"]);

    const unscopedGroup = report.missions[1];
    if (!unscopedGroup) throw new Error("missing unscoped group");
    expect("synthetic" in unscopedGroup.mission).toBe(true);
    // Tasks attached to non-active missions stay grouped under their mission.
    // Only tasks with mission_id === undefined fall into the synthetic bucket.
    expect(unscopedGroup.tasks.map((t) => t.task.id)).toEqual(["tsk-3"]);
  });

  it("excludes terminal-state tasks (shipped, abandoned) from Active missions", async () => {
    const mission = makeMission({ id: "mis-x", status: "executing" });
    const tasks = [
      makeTask({ id: "tsk-ready", slug: "a", title: "A", state: "ready", mission_id: "mis-x" }),
      makeTask({ id: "tsk-shipped", slug: "b", title: "B", state: "shipped", mission_id: "mis-x" }),
      makeTask({ id: "tsk-abandoned", slug: "c", title: "C", state: "abandoned", mission_id: "mis-x" }),
      makeTask({ id: "tsk-unscoped-shipped", slug: "d", title: "D", state: "shipped" }),
      makeTask({ id: "tsk-unscoped-draft", slug: "e", title: "E", state: "draft" }),
    ];

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
      featureMissionStore: mockMissionStore([mission]),
    });

    const missionGroup = report.missions[0];
    if (!missionGroup) throw new Error("missing mission group");
    expect(missionGroup.tasks.map((t) => t.task.id)).toEqual(["tsk-ready"]);

    const unscopedGroup = report.missions[1];
    if (!unscopedGroup) throw new Error("missing unscoped group");
    expect("synthetic" in unscopedGroup.mission).toBe(true);
    expect(unscopedGroup.tasks.map((t) => t.task.id)).toEqual(["tsk-unscoped-draft"]);
  });

  it("only stale handoffs (no pickup, >24h old) count toward stale_handoff_count", async () => {
    const now = Date.now();
    const dayAgo = new Date(now - 86_400_000 - 60_000).toISOString();
    const recent = new Date(now - 60_000).toISOString();
    const envelopes: HandoffEnvelope[] = [
      { id: "hnd-stale", task_id: "tsk-1", trigger_verb: "task:claim", created_at: dayAgo },
      { id: "hnd-picked", task_id: "tsk-2", trigger_verb: "task:claim", created_at: dayAgo },
      { id: "hnd-fresh", task_id: "tsk-3", trigger_verb: "task:claim", created_at: recent },
    ];
    const pickups: HandoffPickup[] = [
      { id: "pkp-1", envelope_id: "hnd-picked", picked_up_by: "agent", picked_up_at: dayAgo },
    ];

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      handoffEmitter: mockHandoffEmitter({ envelopes, pickups }),
    });

    expect(report.project_state.stale_handoff_count).toBe(1);
  });

  it("picks the latest verdict across all tasks for latest_verdict", async () => {
    const tasks = [
      makeTask({ id: "tsk-1", slug: "a", title: "A", state: "verifying" }),
      makeTask({ id: "tsk-2", slug: "b", title: "B", state: "verifying" }),
    ];
    const verdicts = [
      makeVerdict({ id: "v1", taskId: "tsk-1", decision: "FAIL", computedAt: "2026-05-01T00:00:00.000Z" }),
      makeVerdict({ id: "v2", taskId: "tsk-2", decision: "PASS", computedAt: "2026-05-10T00:00:00.000Z" }),
    ];

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
      verdictStore: mockVerdictStore(verdicts),
    });

    expect(report.project_state.latest_verdict?.taskId).toBe("tsk-2");
    expect(report.project_state.latest_verdict?.decision).toBe("PASS");
  });

  it("tolerates a single corrupt verdict file without failing the whole report", async () => {
    const tasks = [
      makeTask({ id: "tsk-good", slug: "g", title: "G", state: "ready" }),
      makeTask({ id: "tsk-bad", slug: "b", title: "B", state: "ready" }),
    ];
    const goodVerdict = makeVerdict({
      id: "v-good",
      taskId: "tsk-good",
      decision: "PASS",
      computedAt: "2026-05-10T00:00:00.000Z",
    });
    const flakyStore = mockVerdictStore([goodVerdict], {
      readLatest: async (id) => {
        if (id === "tsk-bad") throw new Error("corrupt verdict file");
        return id === "tsk-good" ? goodVerdict : undefined;
      },
    });

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
      verdictStore: flakyStore,
    });

    expect(report.project_state.latest_verdict?.taskId).toBe("tsk-good");
  });
});
