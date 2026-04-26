import { describe, expect, it } from "bun:test";
import type { Task } from "@/features/task/domain/task-types.js";
import { groupTasksByTrack } from "@/features/task/usecases/group-tasks-by-track.usecase.js";

const BASE_TIME = "2026-04-26T00:00:00.000Z";

function makeTask(partial: Partial<Task> & { id: string; title: string }): Task {
  return {
    title: partial.title,
    type: partial.type ?? "feature",
    priority: partial.priority ?? 2,
    status: partial.status ?? "pending",
    parentId: partial.parentId,
    slug: partial.slug,
    labels: partial.labels ?? [],
    blocks: partial.blocks ?? [],
    blockedBy: partial.blockedBy ?? [],
    createdAt: partial.createdAt ?? BASE_TIME,
    updatedAt: partial.updatedAt ?? BASE_TIME,
    id: partial.id,
  };
}

describe("groupTasksByTrack", () => {
  it("H3: top-level tasks without slugs render with their tsk-id as identifier", () => {
    const tasks: Task[] = [
      makeTask({ id: "tsk-aaaaaa", title: "Slugless track" }),
    ];
    const projection = groupTasksByTrack(tasks);
    expect(projection.tracks).toHaveLength(1);
    expect(projection.tracks[0]!.identifier).toBe("tsk-aaaaaa");
    expect(projection.tracks[0]!.slug).toBeUndefined();
  });

  it("H4: tracks with zero steps still render as a headline", () => {
    const tasks: Task[] = [
      makeTask({ id: "tsk-aaaaaa", title: "Solo", slug: "implement/solo" }),
    ];
    const projection = groupTasksByTrack(tasks);
    expect(projection.tracks).toHaveLength(1);
    expect(projection.tracks[0]!.steps).toHaveLength(0);
  });

  it("H1: 3-deep grandchildren roll up to the nearest top-level ancestor", () => {
    const tasks: Task[] = [
      makeTask({ id: "tsk-aaaaaa", title: "Track", slug: "implement/foo" }),
      makeTask({ id: "tsk-bbbbbb", title: "Mid", parentId: "tsk-aaaaaa" }),
      makeTask({ id: "tsk-cccccc", title: "Leaf", parentId: "tsk-bbbbbb" }),
    ];
    const projection = groupTasksByTrack(tasks);
    expect(projection.tracks).toHaveLength(1);
    const track = projection.tracks[0]!;
    expect(track.steps.map((s) => s.id).sort()).toEqual(["tsk-bbbbbb", "tsk-cccccc"]);
  });

  it("H2: orphans (steps with missing parent) land in the orphan bucket", () => {
    const tasks: Task[] = [
      makeTask({ id: "tsk-aaaaaa", title: "Real track", slug: "implement/real" }),
      makeTask({ id: "tsk-cccccc", title: "Lost step", parentId: "tsk-zzzzzz" }),
    ];
    const projection = groupTasksByTrack(tasks);
    expect(projection.orphans.map((o) => o.id)).toEqual(["tsk-cccccc"]);
  });

  it("computes the header as active/pending/blocked", () => {
    const blocker = makeTask({ id: "tsk-aaaaaa", title: "Blocker", slug: "implement/blocker" });
    const tasks: Task[] = [
      blocker,
      makeTask({ id: "tsk-bbbbbb", title: "Active", status: "in_progress", slug: "implement/active" }),
      makeTask({
        id: "tsk-cccccc",
        title: "Blocked",
        slug: "implement/blocked",
        blockedBy: [blocker.id],
      }),
      makeTask({ id: "tsk-dddddd", title: "Pending", slug: "implement/pending" }),
      makeTask({ id: "tsk-eeeeee", title: "Done", status: "completed", slug: "implement/done" }),
    ];
    const projection = groupTasksByTrack(tasks);
    expect(projection.header).toEqual({
      open: 4,
      active: 1,
      ready: 2,
      pending: 2,
      blocked: 1,
      blockedTracks: 1,
    });
  });

  it("places track-tasks in_progress first, then by createdAt", () => {
    const a = makeTask({
      id: "tsk-aaaaaa",
      title: "First pending",
      slug: "implement/first-pending",
      createdAt: "2026-04-26T00:00:00.001Z",
    });
    const active = makeTask({
      id: "tsk-bbbbbb",
      title: "Active track-task",
      slug: "implement/active-track",
      status: "in_progress",
      createdAt: "2026-04-26T00:00:00.002Z",
    });
    const blocked = makeTask({
      id: "tsk-cccccc",
      title: "Blocked track",
      slug: "implement/blocked-track",
      createdAt: "2026-04-26T00:00:00.003Z",
    });
    const blockedStep = makeTask({
      id: "tsk-ccccc1",
      title: "Step blocked",
      parentId: blocked.id,
      blockedBy: [a.id],
    });
    const tasks = [a, active, blocked, blockedStep];
    const projection = groupTasksByTrack(tasks);
    expect(projection.tracks.map((t) => t.identifier)).toEqual([
      "implement/active-track",
      "implement/first-pending",
      "implement/blocked-track",
    ]);
  });

  it("filters via trackFilter to a single track", () => {
    const tasks = [
      makeTask({ id: "tsk-aaaaaa", title: "A", slug: "implement/a" }),
      makeTask({ id: "tsk-bbbbbb", title: "B", slug: "implement/b" }),
    ];
    const projection = groupTasksByTrack(tasks, { trackFilter: "implement/a" });
    expect(projection.tracks.map((t) => t.identifier)).toEqual(["implement/a"]);
  });
});
