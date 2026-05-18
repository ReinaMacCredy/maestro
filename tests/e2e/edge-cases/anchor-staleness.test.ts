/**
 * Edge Case 1 (anchor staleness): when the head SHA recorded by the last
 * session-start no longer exists in the working tree (e.g., a force-push
 * orphaned it), `task introspect` must mark the anchor stale and surface a
 * recovery hint instead of pretending no commits happened.
 */
import { describe, it, expect } from "bun:test";
import {
  composeTaskIntrospection,
  formatTaskIntrospectionMarkdown,
} from "@/shared/domain/task/usecases/compose-task-introspection.usecase.js";
import type { TaskIntrospectionDeps } from "@/shared/domain/task/usecases/compose-task-introspection.usecase.js";
import type { EvidenceRow, SessionStartPayload } from "@/features/evidence/index.js";
import type { Task } from "@/shared/domain/task";

function makeTask(): Task {
  return {
    id: "task-anchor-001",
    title: "Anchor test",
    type: "implementation",
    priority: 2,
    status: "in-progress",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-05-01T00:00:00.000Z",
    updatedAt: "2026-05-01T00:00:00.000Z",
  };
}

function makeSessionStartRow(headSha: string, createdAt: string): EvidenceRow<"session-start"> {
  return {
    schema_version: 3,
    id: `ev-${headSha.slice(0, 6)}`,
    task_id: "task-anchor-001",
    kind: "session-start",
    witness_level: "witnessed-by-maestro",
    created_at: createdAt,
    payload: { headSha, baselineLintStatus: "clean" } as SessionStartPayload,
  };
}

function makeDeps(overrides: Partial<TaskIntrospectionDeps> = {}): TaskIntrospectionDeps {
  const task = makeTask();
  return {
    taskStore: {
      get: async () => task,
      all: async () => [task],
    } as any,
    continuationStore: {
      getActive: async () => undefined,
      getCompleted: async () => undefined,
    } as any,
    continuationHistory: { listRecent: async () => [] } as any,
    specStore: { read: async () => undefined } as any,
    verdictStore: { readLatest: async () => undefined } as any,
    evidenceStore: { list: async () => [] } as any,
    runStateStore: { read: async () => undefined } as any,
    contractStore: { getByTaskId: async () => undefined } as any,
    contractVersionStore: {
      readCurrent: async () => undefined,
      listVersions: async () => [],
      readAtVersion: async () => undefined,
    } as any,
    repoRoot: "/tmp/repo-fake",
    ...overrides,
  };
}

describe("Edge Case 1: anchor staleness", () => {
  it("marks anchor stale when checkCommitReachable returns false", async () => {
    const sha = "abcdef1234567890abcdef1234567890abcdef12";
    const deps = makeDeps({
      evidenceStore: { list: async () => [makeSessionStartRow(sha, "2026-05-01T00:00:00.000Z")] } as any,
      checkCommitReachable: async () => false,
    });
    const view = await composeTaskIntrospection(deps, "task-anchor-001");
    expect(view.anchor).toBeDefined();
    expect(view.anchor?.sha).toBe(sha);
    expect(view.anchor?.stale).toBe(true);
    expect(view.recentCommits).toEqual([]);
  });

  it("leaves anchor non-stale when commit is reachable", async () => {
    const sha = "abcdef1234567890abcdef1234567890abcdef12";
    const deps = makeDeps({
      evidenceStore: { list: async () => [makeSessionStartRow(sha, "2026-05-01T00:00:00.000Z")] } as any,
      checkCommitReachable: async () => true,
      resolveCommitsSince: async () => [{ sha: "ffffffffffffffffffffffffffffffffffffffff", subject: "test commit" }],
    });
    const view = await composeTaskIntrospection(deps, "task-anchor-001");
    expect(view.anchor?.stale).toBe(false);
    expect(view.recentCommits).toHaveLength(1);
  });

  it("markdown surfaces recovery hint when stale", async () => {
    const sha = "abcdef1234567890abcdef1234567890abcdef12";
    const deps = makeDeps({
      evidenceStore: { list: async () => [makeSessionStartRow(sha, "2026-05-01T00:00:00.000Z")] } as any,
      checkCommitReachable: async () => false,
    });
    const view = await composeTaskIntrospection(deps, "task-anchor-001");
    const md = formatTaskIntrospectionMarkdown(view);
    expect(md).toContain("anchor: stale");
    expect(md).toContain("maestro session start");
  });
});
