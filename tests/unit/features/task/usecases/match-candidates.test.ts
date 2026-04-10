import { describe, expect, it } from "bun:test";
import { matchCandidates } from "@/features/task/usecases/match-candidates.usecase.js";
import type { Task } from "@/features/task/domain/task-types.js";
import type { TaskCandidate } from "@/features/task/domain/task-candidate.js";

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-ready",
    title: "JWT middleware",
    type: "task",
    priority: 2,
    status: "open",
    labels: [],
    dependsOn: [],
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
    const result = matchCandidates(task(), []);
    expect(result).toEqual([]);
  });

  it("returns empty when the task has no extractable keywords", () => {
    const result = matchCandidates(
      task({ title: "a b c", labels: [] }),
      [candidate({ keywords: ["argon2"] })],
    );
    expect(result).toEqual([]);
  });

  it("surfaces a candidate whose keywords overlap the task title", () => {
    const result = matchCandidates(
      task({ title: "JWT middleware", labels: ["auth"] }),
      [
        candidate({
          id: "tsk-past1",
          sourceTaskId: "tsk-past1",
          title: "Implement JWT signing",
          reason: "use HS256 not RS256",
          keywords: ["jwt", "signing", "use", "hs256", "rs256"],
        }),
      ],
    );
    expect(result.length).toBe(1);
    expect(result[0]?.sourceTaskId).toBe("tsk-past1");
    expect(result[0]?.matchedKeywords).toContain("jwt");
  });

  it("also matches against labels, not only title", () => {
    const result = matchCandidates(
      task({ title: "Unrelated work", labels: ["auth"] }),
      [
        candidate({
          id: "tsk-past-auth",
          sourceTaskId: "tsk-past-auth",
          title: "Auth token expiry",
          reason: "default TTL is too long",
          keywords: ["auth", "token", "expiry", "default", "long"],
        }),
      ],
    );
    expect(result.length).toBe(1);
    expect(result[0]?.matchedKeywords).toContain("auth");
  });

  it("excludes a candidate whose sourceTaskId matches the task itself", () => {
    // Reopened task that was previously closed — should not see its own
    // past close as a hint.
    const result = matchCandidates(
      task({ id: "tsk-reopened", title: "JWT middleware" }),
      [
        candidate({
          id: "tsk-reopened",
          sourceTaskId: "tsk-reopened",
          title: "JWT middleware",
          reason: "initial implementation",
          keywords: ["jwt", "middleware", "initial", "implementation"],
        }),
      ],
    );
    expect(result).toEqual([]);
  });

  it("ignores candidates with zero keyword overlap", () => {
    const result = matchCandidates(
      task({ title: "JWT middleware" }),
      [
        candidate({
          id: "tsk-other",
          sourceTaskId: "tsk-other",
          keywords: ["database", "migration", "rollback"],
        }),
      ],
    );
    expect(result).toEqual([]);
  });

  it("sorts by overlap count desc, then capturedAt desc", () => {
    const result = matchCandidates(
      task({ title: "JWT auth middleware" }),
      [
        candidate({
          id: "tsk-one-match",
          sourceTaskId: "tsk-one-match",
          keywords: ["middleware"],
          capturedAt: "2026-04-01T00:00:00.000Z",
        }),
        candidate({
          id: "tsk-two-match",
          sourceTaskId: "tsk-two-match",
          keywords: ["jwt", "auth"],
          capturedAt: "2026-03-15T00:00:00.000Z",
        }),
        candidate({
          id: "tsk-newer-one-match",
          sourceTaskId: "tsk-newer-one-match",
          keywords: ["auth"],
          capturedAt: "2026-04-08T00:00:00.000Z",
        }),
      ],
    );
    expect(result.length).toBe(3);
    // Two matches wins regardless of date.
    expect(result[0]?.sourceTaskId).toBe("tsk-two-match");
    // Between the two single-match candidates, newer wins.
    expect(result[1]?.sourceTaskId).toBe("tsk-newer-one-match");
    expect(result[2]?.sourceTaskId).toBe("tsk-one-match");
  });

  it("caps at maxHints (default 3)", () => {
    const cands = Array.from({ length: 10 }, (_, i) =>
      candidate({
        id: `tsk-past-${i}`,
        sourceTaskId: `tsk-past-${i}`,
        keywords: ["jwt"],
        capturedAt: `2026-04-0${(i % 9) + 1}T00:00:00.000Z`,
      }),
    );
    const result = matchCandidates(task({ title: "JWT middleware" }), cands);
    expect(result.length).toBe(3);
  });

  it("respects a custom maxHints value", () => {
    const cands = Array.from({ length: 5 }, (_, i) =>
      candidate({
        id: `tsk-past-${i}`,
        sourceTaskId: `tsk-past-${i}`,
        keywords: ["jwt"],
        capturedAt: `2026-04-0${(i % 9) + 1}T00:00:00.000Z`,
      }),
    );
    const result = matchCandidates(task({ title: "JWT middleware" }), cands, 1);
    expect(result.length).toBe(1);
  });

  it("returns an empty array when maxHints is 0", () => {
    const result = matchCandidates(
      task({ title: "JWT middleware" }),
      [candidate({ keywords: ["jwt"] })],
      0,
    );
    expect(result).toEqual([]);
  });

  it("carries the matched keywords in the hint for debugging", () => {
    const result = matchCandidates(
      task({ title: "JWT auth middleware" }),
      [
        candidate({
          id: "tsk-multi",
          sourceTaskId: "tsk-multi",
          keywords: ["jwt", "auth", "unrelated", "stuff"],
        }),
      ],
    );
    expect(result[0]?.matchedKeywords).toEqual(["jwt", "auth"]);
  });
});
