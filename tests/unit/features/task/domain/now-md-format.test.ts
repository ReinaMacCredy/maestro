import { describe, expect, it } from "bun:test";
import type { Task } from "@/features/task/domain/task-types.js";
import { buildNowMd } from "@/features/task/domain/now-md-format.js";

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "tsk-000001",
    title: "Default task",
    type: "task",
    priority: 2,
    status: "pending",
    labels: [],
    blocks: [],
    blockedBy: [],
    createdAt: "2026-04-21T00:00:00.000Z",
    updatedAt: "2026-04-21T00:00:00.000Z",
    ...overrides,
  };
}

describe("buildNowMd", () => {
  it("returns an empty header when there are no tasks", () => {
    const md = buildNowMd({ tasks: [], now: new Date("2026-04-21T12:00:00.000Z") });
    expect(md).toContain("# NOW");
    expect(md).toContain("Updated: 2026-04-21T12:00:00.000Z");
    expect(md).toContain("No tasks yet.");
  });

  it("groups in-progress and ready tasks", () => {
    const now = new Date("2026-04-21T12:00:00.000Z");
    const md = buildNowMd({
      tasks: [
        task({
          id: "tsk-aaaaaa",
          title: "active work",
          status: "in_progress",
          assignee: "alice",
          claimedAt: "2026-04-21T10:00:00.000Z",
          updatedAt: "2026-04-21T11:30:00.000Z",
        }),
        task({
          id: "tsk-bbbbbb",
          title: "pick up next",
          priority: 1,
        }),
      ],
      now,
    });

    expect(md).toContain("## In progress (1)");
    expect(md).toContain("tsk-aaaaaa . active work");
    expect(md).toContain("Owner: alice");
    expect(md).toContain("## Ready to pick up (1)");
    expect(md).toContain("tsk-bbbbbb . pick up next");
  });

  it("flags in-progress tasks older than 4h as stuck", () => {
    const now = new Date("2026-04-21T12:00:00.000Z");
    const md = buildNowMd({
      tasks: [
        task({
          id: "tsk-stuck0",
          title: "stale",
          status: "in_progress",
          assignee: "bob",
          claimedAt: "2026-04-21T03:00:00.000Z",
          updatedAt: "2026-04-21T03:30:00.000Z",
        }),
      ],
      now,
    });

    expect(md).toContain("## Stuck (1)");
    expect(md).toMatch(/tsk-stuck0 \. stale/);
  });

  it("hides unblocked-by tasks from Ready and shows blockers inline", () => {
    const now = new Date("2026-04-21T12:00:00.000Z");
    const md = buildNowMd({
      tasks: [
        task({
          id: "tsk-block1",
          title: "blocker",
          status: "pending",
        }),
        task({
          id: "tsk-block2",
          title: "waiting",
          status: "pending",
          blockedBy: ["tsk-block1"],
        }),
      ],
      now,
    });

    expect(md).toContain("## Ready to pick up (1)");
    expect(md).toContain("tsk-block1 . blocker");
    expect(md).not.toMatch(/### tsk-block2 \. waiting/);
  });

  it("truncates long descriptions at 300 chars", () => {
    const longDesc = "x".repeat(500);
    const md = buildNowMd({
      tasks: [
        task({
          id: "tsk-longgg",
          title: "long",
          description: longDesc,
        }),
      ],
      now: new Date("2026-04-21T00:00:00.000Z"),
    });

    expect(md).toContain("x".repeat(300) + "...");
    expect(md).not.toContain("x".repeat(301));
  });
});
