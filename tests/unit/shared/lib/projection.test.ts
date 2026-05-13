import { describe, expect, it } from "bun:test";

import type { EvidenceRow } from "@/features/evidence/domain/types.js";
import type { HandoffRecord } from "@/features/handoff/domain/handoff-types.js";
import type { Mission } from "@/features/mission/domain/mission-types.js";
import type { Task } from "@/features/task/domain/task-types.js";
import {
  summarizeEvidence,
  summarizeHandoff,
  summarizeMission,
  summarizeTask,
} from "@/shared/lib/projection.js";

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: "task_001",
    title: "do thing",
    description: "long description that should not survive projection",
    type: "task",
    priority: 2,
    status: "pending",
    slug: "implement/do-thing",
    labels: ["alpha", "beta"],
    blocks: [],
    blockedBy: ["task_other"],
    parentId: undefined,
    assignee: "claude",
    claimedAt: "2026-05-13T00:00:00Z",
    createdAt: "2026-05-13T00:00:00Z",
    updatedAt: "2026-05-13T00:00:00Z",
    lastActivityAt: "2026-05-13T00:00:00Z",
    ...overrides,
  };
}

function makeMission(): Mission {
  return {
    id: "mission_001",
    status: "executing",
    title: "ship doctrine",
    description: "verbose description that the summary drops",
    proposal: "lengthy proposal body",
    milestones: [
      {
        id: "m1",
        title: "phase 1",
        description: "long description",
        order: 1,
        featureIds: ["f1", "f2"],
      },
    ],
    features: ["f1", "f2"],
    createdAt: "2026-05-13T00:00:00Z",
    updatedAt: "2026-05-13T01:00:00Z",
  };
}

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

function makeHandoff(): HandoffRecord {
  return {
    id: "ho_001",
    createdAt: "2026-05-13T00:00:00Z",
    task: "investigate flake",
    name: "investigate-flake",
    agent: "codex",
    model: "gpt-5.4",
    status: "launched",
    wait: false,
    sourceDir: "/src",
    targetDir: "/tgt",
    promptPath: "/p",
    outputPath: "/o",
    command: ["codex", "exec"],
    refs: { taskId: "task_001", missionId: "mission_001" },
  };
}

describe("summarizeTask", () => {
  it("preserves identifiers and routing fields, drops description and timestamps", () => {
    const summary = summarizeTask(makeTask());
    expect(summary).toEqual({
      slug: "implement/do-thing",
      id: "task_001",
      title: "do thing",
      status: "pending",
      type: "task",
      priority: 2,
      blockedByCount: 1,
      assignee: "claude",
    });
    expect("description" in summary).toBe(false);
    expect("labels" in summary).toBe(false);
    expect("createdAt" in summary).toBe(false);
    expect("updatedAt" in summary).toBe(false);
    expect("receipt" in summary).toBe(false);
  });

  it("omits optional fields when undefined", () => {
    const summary = summarizeTask(
      makeTask({ slug: undefined, assignee: undefined, parentId: undefined }),
    );
    expect("slug" in summary).toBe(false);
    expect("assignee" in summary).toBe(false);
    expect("parentId" in summary).toBe(false);
  });

  it("reduces blockedBy[] to a count", () => {
    const summary = summarizeTask(
      makeTask({ blockedBy: ["a", "b", "c", "d"] }),
    );
    expect(summary.blockedByCount).toBe(4);
  });
});

describe("summarizeMission", () => {
  it("replaces nested arrays with counts and drops description", () => {
    const summary = summarizeMission(makeMission());
    expect(summary).toEqual({
      id: "mission_001",
      title: "ship doctrine",
      status: "executing",
      milestoneCount: 1,
      featureCount: 2,
      updatedAt: "2026-05-13T01:00:00Z",
    });
    expect("description" in summary).toBe(false);
    expect("milestones" in summary).toBe(false);
    expect("features" in summary).toBe(false);
    expect("proposal" in summary).toBe(false);
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

describe("summarizeHandoff", () => {
  it("keeps semantic identifier and lifecycle fields, drops paths and command", () => {
    const summary = summarizeHandoff(makeHandoff());
    expect(summary).toEqual({
      name: "investigate-flake",
      id: "ho_001",
      status: "launched",
      task: "investigate flake",
      agent: "codex",
      model: "gpt-5.4",
      createdAt: "2026-05-13T00:00:00Z",
      wait: false,
      taskId: "task_001",
      missionId: "mission_001",
    });
    expect("sourceDir" in summary).toBe(false);
    expect("targetDir" in summary).toBe(false);
    expect("promptPath" in summary).toBe(false);
    expect("outputPath" in summary).toBe(false);
    expect("command" in summary).toBe(false);
    expect("worktree" in summary).toBe(false);
  });

  it("omits refs when absent", () => {
    const summary = summarizeHandoff({
      ...makeHandoff(),
      refs: {},
    });
    expect("taskId" in summary).toBe(false);
    expect("missionId" in summary).toBe(false);
  });
});
