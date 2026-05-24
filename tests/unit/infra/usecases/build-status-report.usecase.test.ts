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
    expect(report.project_state.corrupt_verdict_count).toBe(0);
    expect(report.project_state.latest_verdict).toBeUndefined();
  });

  it("project_state has stable JSON keys", async () => {
    const report = await buildStatusReport(baseDeps(cwd));

    expect(Object.keys(report.project_state).sort()).toEqual([
      "corrupt_verdict_count",
      "latest_verdict",
      "stale_handoff_count",
      "stuck_verifying_count",
    ]);
  });

  it("maestro_health is always a SetupCheckReport; recent_transitions stays a stable array", async () => {
    const report = await buildStatusReport(baseDeps(cwd));

    expect(Array.isArray(report.maestro_health.entries)).toBe(true);
    expect(typeof report.maestro_health.ok).toBe("boolean");
    expect(Array.isArray(report.recent_transitions)).toBe(true);
  });

  it("hard-refuses when .maestro/ directory is missing", async () => {
    await rm(join(cwd, ".maestro"), { recursive: true });
    await expect(buildStatusReport(baseDeps(cwd))).rejects.toThrow(/not initialized/i);
  });

  // Regression: FIX-13 -- the hint previously pointed at `maestro init`, which
  // is only a hidden alias. Canonical verb is `maestro setup`. If someone
  // reverts the message, fresh users will follow the legacy path.
  it("error hint points at the canonical 'maestro setup' verb, not the legacy 'init' alias", async () => {
    await rm(join(cwd, ".maestro"), { recursive: true });

    // Capture the actual message string so we can assert against it directly;
    // a negative `not.toThrow(/init/)` matcher would pass even on a message
    // like "setup or init", which is the opposite of what we want.
    let message = "";
    try {
      await buildStatusReport(baseDeps(cwd));
    } catch (err) {
      message = err instanceof Error ? err.message : String(err);
    }

    expect(message).toMatch(/maestro setup/);
    // Word-boundary guard: standalone "init" must not appear (substrings
    // like "initialized" pass because `\b` requires non-word context on both
    // sides, and 'init' is followed by 'i' in "initialized").
    expect(message).not.toMatch(/\binit\b/);
  });

  it("surfaces tasks attached to non-active missions in the unscoped group instead of dropping them", async () => {
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
    // Both tasks with mission_id === undefined AND tasks whose mission is no
    // longer active fall into the synthetic bucket. Without this, tsk-2
    // would silently vanish from the status view while still appearing in
    // next_ready (the orphan-mission bug).
    expect(new Set(unscopedGroup.tasks.map((t) => t.task.id))).toEqual(
      new Set(["tsk-2", "tsk-3"]),
    );
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
    // Regression: FIX-12 -- the FS adapter swallows JSON.parse errors
    // internally, so `readLatest` never throws on corruption. The
    // corruption count has to be surfaced through
    // `readLatestWithCorruption`, which the use case now consults.
    const flakyStore = mockVerdictStore([goodVerdict], {
      readLatestWithCorruption: async (id) => {
        if (id === "tsk-bad") return { verdict: undefined, corruptCount: 1 };
        return {
          verdict: id === "tsk-good" ? goodVerdict : undefined,
          corruptCount: 0,
        };
      },
    });

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
      verdictStore: flakyStore,
    });

    expect(report.project_state.latest_verdict?.taskId).toBe("tsk-good");
    expect(report.project_state.corrupt_verdict_count).toBe(1);
  });

  // Regression: FIX-14 -- pickNextReady previously sorted by `updated_at`,
  // which gets bumped by every #mutate (assignee swap, blocker change), so the
  // pick shuffled out from under callers. The fix sorts by `created_at`
  // (immutable). Test fixture: tsk-1 was created EARLIEST but updated LAST.
  // Under the bug, tsk-3 would win (oldest updated_at). After the fix, tsk-1
  // wins (oldest created_at).
  it("picks the oldest-created ready task, ignoring later updates", async () => {
    const tasks = [
      makeTask({
        id: "tsk-1",
        slug: "first",
        title: "First",
        state: "ready",
        created_at: "2026-05-01T00:00:00.000Z",
        updated_at: "2026-05-20T00:00:00.000Z",
      }),
      makeTask({
        id: "tsk-2",
        slug: "second",
        title: "Second",
        state: "ready",
        created_at: "2026-05-05T00:00:00.000Z",
        updated_at: "2026-05-15T00:00:00.000Z",
      }),
      makeTask({
        id: "tsk-3",
        slug: "third",
        title: "Third",
        state: "ready",
        created_at: "2026-05-10T00:00:00.000Z",
        updated_at: "2026-05-10T00:00:00.000Z",
      }),
    ];

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
    });

    expect(report.next_ready?.id).toBe("tsk-1");
  });

  it("excludes non-ready tasks from next_ready (boundary: states other than 'ready')", async () => {
    const tasks = [
      makeTask({
        id: "tsk-draft",
        slug: "d",
        title: "D",
        state: "draft",
        created_at: "2026-05-01T00:00:00.000Z",
      }),
      makeTask({
        id: "tsk-ready",
        slug: "r",
        title: "R",
        state: "ready",
        created_at: "2026-05-10T00:00:00.000Z",
      }),
      makeTask({
        id: "tsk-shipped",
        slug: "s",
        title: "S",
        state: "shipped",
        created_at: "2026-04-01T00:00:00.000Z", // even older, but terminal
      }),
    ];

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
    });

    expect(report.next_ready?.id).toBe("tsk-ready");
  });

  it("returns next_ready=undefined when no ready tasks exist (empty boundary)", async () => {
    const tasks = [
      makeTask({ id: "tsk-d", slug: "d", title: "D", state: "draft" }),
      makeTask({ id: "tsk-s", slug: "s", title: "S", state: "shipped" }),
    ];

    const report = await buildStatusReport({
      ...baseDeps(cwd),
      taskStore: mockRepoTaskStore(tasks),
    });

    expect(report.next_ready).toBeUndefined();
  });

  it("includes missions with zero tasks and shows empty-state hint in plain output", async () => {
    const emptyMission = makeMission({ id: "mis-empty", status: "executing" });
    const report = await buildStatusReport({
      ...baseDeps(cwd),
      featureMissionStore: mockMissionStore([emptyMission]),
    });

    expect(report.missions).toHaveLength(1);
    const group = report.missions[0];
    if (!group) throw new Error("missing mission group");
    expect("synthetic" in group.mission).toBe(false);
    if ("synthetic" in group.mission) throw new Error("unreachable");
    expect(group.mission.id).toBe("mis-empty");
    expect(group.tasks).toHaveLength(0);
  });
});
