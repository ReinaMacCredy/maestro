import { describe, expect, it } from "bun:test";
import {
  STUCK_THRESHOLD_MS,
  buildNowMd,
  isStuck,
} from "@/service/build-now-md.js";
import type { Task } from "@/types/task.js";
import type { TaskState } from "@/types/task-state.js";

const NOW = new Date("2026-05-16T12:00:00.000Z");

function makeTask(overrides: Partial<Task> & { id: string; state: TaskState }): Task {
  return {
    id: overrides.id,
    slug: overrides.slug ?? `slug-${overrides.id}`,
    title: overrides.title ?? `title ${overrides.id}`,
    state: overrides.state,
    spec_path: overrides.spec_path,
    mission_id: overrides.mission_id,
    assignee: overrides.assignee,
    claimed_at: overrides.claimed_at,
    pr_url: overrides.pr_url,
    merged_at: overrides.merged_at,
    blocked_by: overrides.blocked_by ?? [],
    block_reason: overrides.block_reason,
    abandon_reason: overrides.abandon_reason,
    worktree_path: overrides.worktree_path,
    created_at: overrides.created_at ?? "2026-05-16T10:00:00.000Z",
    updated_at: overrides.updated_at ?? "2026-05-16T11:00:00.000Z",
  };
}

describe("buildNowMd", () => {
  it("renders empty-state stub when no tasks exist", () => {
    const out = buildNowMd({ tasks: [], now: NOW });
    expect(out).toBe(`# NOW\nUpdated: ${NOW.toISOString()}\n\nNo tasks yet.\n`);
  });

  it("places drafts under Ready to pick up and skips other sections", () => {
    const tasks = [
      makeTask({ id: "tsk-a-1", state: "draft", created_at: "2026-05-16T08:00:00.000Z" }),
      makeTask({ id: "tsk-a-2", state: "draft", created_at: "2026-05-16T09:00:00.000Z" }),
    ];
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).toContain("## In flight (0)");
    expect(out).toContain("## Ready to pick up (2)");
    expect(out).toContain("## Ready to ship (0)");
    expect(out).toContain("## Blocked (0)");
    expect(out).toContain("## Stuck (0)");
    expect(out).toMatch(/tsk-a-1[\s\S]*tsk-a-2/);
  });

  it("excludes draft tasks that have unresolved blockers", () => {
    const tasks = [
      makeTask({ id: "tsk-a-1", state: "draft", blocked_by: ["tsk-a-9"] }),
      makeTask({ id: "tsk-a-2", state: "draft" }),
    ];
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).toContain("## Ready to pick up (1)");
    expect(out).toContain("tsk-a-2");
    expect(out).not.toContain("tsk-a-1");
  });

  it("excludes terminal (shipped, abandoned) tasks from every section", () => {
    const tasks = [
      makeTask({ id: "tsk-z-1", state: "shipped" }),
      makeTask({ id: "tsk-z-2", state: "abandoned" }),
      makeTask({ id: "tsk-a-1", state: "draft" }),
    ];
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).not.toContain("tsk-z-1");
    expect(out).not.toContain("tsk-z-2");
    expect(out).toContain("tsk-a-1");
  });

  it("sorts in-flight by claimed_at asc, then id asc as deterministic tiebreaker", () => {
    const tasks = [
      makeTask({ id: "tsk-c-2", state: "doing", claimed_at: "2026-05-16T10:00:00.000Z" }),
      makeTask({ id: "tsk-c-1", state: "doing", claimed_at: "2026-05-16T10:00:00.000Z" }),
      makeTask({ id: "tsk-c-3", state: "claimed", claimed_at: "2026-05-16T09:00:00.000Z" }),
    ];
    const out = buildNowMd({ tasks, now: NOW });
    const order = ["tsk-c-3", "tsk-c-1", "tsk-c-2"];
    let cursor = 0;
    for (const id of order) {
      const idx = out.indexOf(id, cursor);
      expect(idx).toBeGreaterThan(-1);
      cursor = idx;
    }
    expect(out).toContain("## In flight (3)");
  });

  it("includes claimed, doing, and verifying in In flight", () => {
    const tasks = [
      makeTask({ id: "tsk-c-1", state: "claimed", claimed_at: "2026-05-16T09:00:00.000Z" }),
      makeTask({ id: "tsk-c-2", state: "doing", claimed_at: "2026-05-16T09:30:00.000Z" }),
      makeTask({ id: "tsk-c-3", state: "verifying", claimed_at: "2026-05-16T09:45:00.000Z" }),
    ];
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).toContain("## In flight (3)");
  });

  it("renders Ready to ship for state===ready, sorted by updated_at asc", () => {
    const tasks = [
      makeTask({
        id: "tsk-r-2",
        state: "ready",
        updated_at: "2026-05-16T11:30:00.000Z",
      }),
      makeTask({
        id: "tsk-r-1",
        state: "ready",
        updated_at: "2026-05-16T10:30:00.000Z",
      }),
    ];
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).toContain("## Ready to ship (2)");
    expect(out.indexOf("tsk-r-1")).toBeLessThan(out.indexOf("tsk-r-2"));
  });

  it("renders Blocked tasks with reason and blockers list", () => {
    const tasks = [
      makeTask({
        id: "tsk-b-1",
        state: "blocked",
        block_reason: "waiting on upstream",
        blocked_by: ["tsk-x-1"],
      }),
    ];
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).toContain("## Blocked (1)");
    expect(out).toContain("Reason: waiting on upstream");
    expect(out).toContain("Blocked by: tsk-x-1");
  });

  it("caps Ready to pick up at 5 rendered entries, reports the true count, and appends an overflow line", () => {
    const tasks = Array.from({ length: 8 }, (_, i) =>
      makeTask({
        id: `tsk-d-${i + 1}`,
        state: "draft",
        created_at: `2026-05-16T0${i}:00:00.000Z`,
      }),
    );
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).toContain("## Ready to pick up (8)");
    expect(out).toContain("tsk-d-1");
    expect(out).toContain("tsk-d-5");
    expect(out).not.toContain("tsk-d-6");
    expect(out).not.toContain("tsk-d-7");
    expect(out).not.toContain("tsk-d-8");
    expect(out).toContain("(and 3 more)");
  });

  it("omits the overflow line when the draft count is at or below the cap", () => {
    const tasks = Array.from({ length: 5 }, (_, i) =>
      makeTask({
        id: `tsk-d-${i + 1}`,
        state: "draft",
        created_at: `2026-05-16T0${i}:00:00.000Z`,
      }),
    );
    const out = buildNowMd({ tasks, now: NOW });
    expect(out).toContain("## Ready to pick up (5)");
    expect(out).not.toContain("(and 0 more)");
    expect(out).not.toMatch(/\(and \d+ more\)/);
  });

  it("classifies stuck strictly above the 4h threshold (boundary)", () => {
    const fourHoursAgo = new Date(NOW.getTime() - STUCK_THRESHOLD_MS).toISOString();
    const justUnder = new Date(NOW.getTime() - STUCK_THRESHOLD_MS + 60_000).toISOString();
    const justOver = new Date(NOW.getTime() - STUCK_THRESHOLD_MS - 60_000).toISOString();

    const under = makeTask({ id: "tsk-s-1", state: "doing", updated_at: justUnder });
    const exactly = makeTask({ id: "tsk-s-2", state: "doing", updated_at: fourHoursAgo });
    const over = makeTask({ id: "tsk-s-3", state: "doing", updated_at: justOver });

    expect(isStuck(under, NOW)).toBe(false);
    expect(isStuck(exactly, NOW)).toBe(false);
    expect(isStuck(over, NOW)).toBe(true);

    const out = buildNowMd({ tasks: [under, exactly, over], now: NOW });
    expect(out).toContain("## Stuck (1)");
    expect(out).toContain("tsk-s-3");
  });

  it("never marks non-inflight tasks stuck even with old updated_at", () => {
    const ancient = "2020-01-01T00:00:00.000Z";
    const draftStale = makeTask({ id: "tsk-d-1", state: "draft", updated_at: ancient });
    const blockedStale = makeTask({ id: "tsk-b-1", state: "blocked", updated_at: ancient });
    expect(isStuck(draftStale, NOW)).toBe(false);
    expect(isStuck(blockedStale, NOW)).toBe(false);
  });
});
