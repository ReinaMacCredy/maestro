import { describe, expect, it } from "bun:test";

import type { EvidenceRow } from "@/features/evidence/domain/types.js";
import type { Task } from "@/types/task.js";
import { summarizeEvidence, summarizeTask } from "@/shared/lib/projection.js";

function makeEvidence(): EvidenceRow<"command"> {
  return {
    schema_version: 3,
    id: "ev_001",
    task_id: "task_001",
    session_id: "sess_001",
    kind: "command",
    witness_level: "witnessed-by-maestro",
    created_at: "2026-05-13T00:00:00Z",
    payload: {
      command: "bun test",
      exit: 0,
      log_path: "/some/path",
      duration_ms: 1234,
    },
  };
}

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-abc-123",
    slug: "implement-thing",
    title: "implement thing",
    state: "doing",
    mission_id: "mis-xyz-789",
    assignee: "claude",
    claimed_at: "2026-05-13T00:00:00Z",
    pr_url: "https://example/pr/1",
    blocked_by: ["tsk-blk-001"],
    worktree_path: "/tmp/wt",
    created_at: "2026-05-13T00:00:00Z",
    updated_at: "2026-05-13T01:00:00Z",
    ...overrides,
  };
}

describe("summarizeTask", () => {
  it("preserves lean fields, drops detail timestamps and paths", () => {
    const summary = summarizeTask(makeTask());
    expect(summary).toEqual({
      id: "tsk-abc-123",
      slug: "implement-thing",
      title: "implement thing",
      state: "doing",
      mission_id: "mis-xyz-789",
      assignee: "claude",
      blocked_by_count: 1,
    });
    expect("created_at" in summary).toBe(false);
    expect("updated_at" in summary).toBe(false);
    expect("claimed_at" in summary).toBe(false);
    expect("pr_url" in summary).toBe(false);
    expect("worktree_path" in summary).toBe(false);
  });

  it("omits mission_id and assignee when undefined", () => {
    const summary = summarizeTask(
      makeTask({ mission_id: undefined, assignee: undefined }),
    );
    expect("mission_id" in summary).toBe(false);
    expect("assignee" in summary).toBe(false);
  });

  it("reduces blocked_by[] to a count", () => {
    const summary = summarizeTask(
      makeTask({ blocked_by: ["a", "b", "c"] }),
    );
    expect(summary.blocked_by_count).toBe(3);
  });
});

describe("summarizeEvidence", () => {
  it("drops payload and keeps routing fields", () => {
    const summary = summarizeEvidence(makeEvidence());
    expect(summary).toEqual({
      id: "ev_001",
      task_id: "task_001",
      kind: "command",
      witness_level: "witnessed-by-maestro",
      created_at: "2026-05-13T00:00:00Z",
      session_id: "sess_001",
    });
    expect("payload" in summary).toBe(false);
    expect("schema_version" in summary).toBe(false);
  });

  it("omits session_id when undefined", () => {
    const row = { ...makeEvidence(), session_id: undefined };
    const summary = summarizeEvidence(row);
    expect("session_id" in summary).toBe(false);
  });
});
