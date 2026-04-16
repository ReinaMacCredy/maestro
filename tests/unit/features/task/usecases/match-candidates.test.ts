import { describe, expect, it } from "bun:test";
import type { Task } from "@/features/task/domain/task-types.js";
import type { TaskCandidate } from "@/features/task/domain/task-candidate.js";
import { matchCandidates } from "@/features/task/usecases/match-candidates.usecase.js";

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-ready",
    title: "JWT middleware",
    type: "task",
    priority: 2,
    status: "pending",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-04-10T00:00:00.000Z",
    updatedAt: "2026-04-10T00:00:00.000Z",
    ...overrides,
  };
}

function candidate(overrides: Partial<TaskCandidate> = {}): TaskCandidate {
  return {
    id: "tsk-past",
    sourceTaskId: "tsk-past",
    sourceType: "task-close",
    title: "Past task",
    reason: "a lesson",
    keywords: [],
    capturedAt: "2026-04-09T00:00:00.000Z",
    ...overrides,
  };
}

describe("matchCandidates", () => {
  it("returns empty when there are no candidates", () => {
    expect(matchCandidates(task(), [])).toEqual([]);
  });

  it("surfaces overlapping candidates from title or labels", () => {
    const result = matchCandidates(
      task({ title: "JWT middleware", labels: ["auth"] }),
      [
        candidate({
          id: "tsk-past1",
          sourceTaskId: "tsk-past1",
          keywords: ["jwt", "auth", "middleware"],
        }),
      ],
    );

    expect(result).toHaveLength(1);
    expect(result[0]?.matchedKeywords).toContain("jwt");
  });

  it("filters out a task's own past completion", () => {
    const result = matchCandidates(
      task({ id: "tsk-reopened" }),
      [candidate({ id: "tsk-reopened", sourceTaskId: "tsk-reopened", keywords: ["jwt"] })],
    );

    expect(result).toEqual([]);
  });

  it("sorts by overlap first, then recency", () => {
    const result = matchCandidates(
      task({ title: "JWT auth middleware" }),
      [
        candidate({
          id: "older-stronger",
          sourceTaskId: "older-stronger",
          keywords: ["jwt", "auth"],
          capturedAt: "2026-03-15T00:00:00.000Z",
        }),
        candidate({
          id: "newer-weaker",
          sourceTaskId: "newer-weaker",
          keywords: ["auth"],
          capturedAt: "2026-04-08T00:00:00.000Z",
        }),
      ],
    );

    expect(result[0]?.sourceTaskId).toBe("older-stronger");
    expect(result[1]?.sourceTaskId).toBe("newer-weaker");
  });
});
