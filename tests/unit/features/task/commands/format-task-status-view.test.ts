import { describe, expect, it } from "bun:test";
import type { Task } from "@/features/task/domain/task-types.js";
import { groupTasksByTrack } from "@/features/task/usecases/group-tasks-by-track.usecase.js";
import { formatTaskStatusView } from "@/features/task/commands/task-command-formatters.js";

const NOW = "2026-04-26T00:00:00.000Z";

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
    createdAt: partial.createdAt ?? NOW,
    updatedAt: partial.updatedAt ?? NOW,
    id: partial.id,
  };
}

describe("formatTaskStatusView", () => {
  it("renders the track board by default and preserves grouped detail with compact disabled", () => {
    const worktree = makeTask({
      id: "tsk-100001",
      title: "Pass git config overrides to prevent .git/config.lock race",
      slug: "implement/worktree-config-lock-race",
      status: "in_progress",
      createdAt: "2026-04-26T00:00:01.000Z",
    });

    const promptTrack = makeTask({
      id: "tsk-200001",
      title: "implement/template-prompt-fixes",
      slug: "implement/template-prompt-fixes",
      createdAt: "2026-04-26T00:00:02.000Z",
    });
    const promptStep1 = makeTask({
      id: "tsk-200002",
      title: "Remove contradictory close-issue instruction from implement-prompt.md",
      parentId: promptTrack.id,
      status: "in_progress",
    });
    const promptStep2 = makeTask({
      id: "tsk-200003",
      title: "Replace hardcoded 'main' in review-prompt.md with {{SOURCE_BRANCH}}",
      parentId: promptTrack.id,
    });
    const promptStep3 = makeTask({
      id: "tsk-200004",
      title: "Return reviewer result from Phase 2 callback in parallel-planner-with-review",
      parentId: promptTrack.id,
    });

    const initTrack = makeTask({
      id: "tsk-300001",
      title: "implement/init-template-e2e-tests",
      slug: "implement/init-template-e2e-tests",
      createdAt: "2026-04-26T00:00:03.000Z",
    });
    const initStep1 = makeTask({
      id: "tsk-300002",
      title: "Add AgentInvoker seam, test support module, and blank template e2e test",
      parentId: initTrack.id,
      blockedBy: [promptTrack.id],
    });
    const initStep2 = makeTask({
      id: "tsk-300003",
      title: "Add e2e test for simple-loop init template",
      parentId: initTrack.id,
    });
    const initStep3 = makeTask({
      id: "tsk-300004",
      title: "Add e2e test for sequential-reviewer init template",
      parentId: initTrack.id,
    });
    const initStep4 = makeTask({
      id: "tsk-300005",
      title: "Add e2e test for parallel-planner init template",
      parentId: initTrack.id,
    });

    const agentErrTrack = makeTask({
      id: "tsk-400001",
      title: "implement/agent-error-text-investigation",
      slug: "implement/agent-error-text-investigation",
      createdAt: "2026-04-26T00:00:04.000Z",
    });
    const agentErrStep1 = makeTask({
      id: "tsk-400002",
      title: "Investigate and surface Pi agent error text on non-zero exit",
      parentId: agentErrTrack.id,
      status: "in_progress",
    });
    const agentErrStep2 = makeTask({
      id: "tsk-400003",
      title: "Investigate and surface Codex agent error text on non-zero exit",
      parentId: agentErrTrack.id,
    });
    const agentErrStep3 = makeTask({
      id: "tsk-400004",
      title: "Investigate and surface OpenCode agent error text on non-zero exit",
      parentId: agentErrTrack.id,
    });

    const tasks: Task[] = [
      worktree,
      promptTrack, promptStep1, promptStep2, promptStep3,
      initTrack, initStep1, initStep2, initStep3, initStep4,
      agentErrTrack, agentErrStep1, agentErrStep2, agentErrStep3,
    ];

    const projection = groupTasksByTrack(tasks);
    const lines = formatTaskStatusView(projection, { color: false });

    expect(lines).toEqual([
      "tasks: 12 open | 3 active | 7 ready | 2 blocked | 1 blocked track",
      "",
      "implement/worktree-config-lock-race",
      "  o Pass git config overrides to prevent .git/config.lock race",
      "      in-progress",
      "",
      "implement/template-prompt-fixes",
      "  o Remove contradictory close-issue instruction from implement-prompt.md",
      "      in-progress",
      "  · Replace hardcoded 'main' in review-prompt.md with {{SOURCE_BRANCH}}",
      "  · Return reviewer result from Phase 2 callback in parallel-planner-with-review",
      "",
      "implement/init-template-e2e-tests",
      "  ! Add AgentInvoker seam, test support module, and blank template e2e test",
      "      blocked by implement/template-prompt-fixes",
      "  · Add e2e test for simple-loop init template",
      "  · Add e2e test for sequential-reviewer init template",
      "  · Add e2e test for parallel-planner init template",
      "",
      "implement/agent-error-text-investigation",
      "  o Investigate and surface Pi agent error text on non-zero exit",
      "      in-progress",
      "  · Investigate and surface Codex agent error text on non-zero exit",
      "  · Investigate and surface OpenCode agent error text on non-zero exit",
    ]);

    const grouped = formatTaskStatusView(projection, { color: false, compact: false });
    expect(grouped).toEqual([
      "tasks: 3 active, 7 pending, 2 blocked",
      "",
      "  o implement/worktree-config-lock-race  Pass git config overrides to prevent .git/config.lock race  in-progress",
      "",
      "implement/template-prompt-fixes",
      "  o Remove contradictory close-issue instruction from implement-prompt.md",
      "      in-progress",
      "  · Replace hardcoded 'main' in review-prompt.md with {{SOURCE_BRANCH}}",
      "  · Return reviewer result from Phase 2 callback in parallel-planner-with-review",
      "",
      "implement/init-template-e2e-tests",
      "  ! Add AgentInvoker seam, test support module, and blank template e2e test",
      "      blocked by implement/template-prompt-fixes",
      "  · Add e2e test for simple-loop init template",
      "  · Add e2e test for sequential-reviewer init template",
      "  · Add e2e test for parallel-planner init template",
      "",
      "implement/agent-error-text-investigation",
      "  o Investigate and surface Pi agent error text on non-zero exit",
      "      in-progress",
      "  · Investigate and surface Codex agent error text on non-zero exit",
      "  · Investigate and surface OpenCode agent error text on non-zero exit",
    ]);
  });

  it("renders solo tracks as track blocks", () => {
    const a = makeTask({
      id: "tsk-aaaaaa",
      title: "Update agents",
      slug: "chore/update-agents",
      status: "in_progress",
      createdAt: "2026-04-26T00:00:01.000Z",
    });
    const b = makeTask({
      id: "tsk-bbbbbb",
      title: "Bump deps",
      slug: "chore/bump-deps",
      createdAt: "2026-04-26T00:00:02.000Z",
    });
    const blocker = makeTask({
      id: "tsk-cccccc",
      title: "Open blocker",
      slug: "implement/blocker",
      createdAt: "2026-04-26T00:00:03.000Z",
    });
    const blocked = makeTask({
      id: "tsk-dddddd",
      title: "Blocked work",
      slug: "implement/blocked-work",
      blockedBy: [blocker.id],
      createdAt: "2026-04-26T00:00:04.000Z",
    });

    const projection = groupTasksByTrack([a, b, blocker, blocked]);
    const lines = formatTaskStatusView(projection, { color: false });

    expect(lines).toEqual([
      "tasks: 4 open | 1 active | 2 ready | 1 blocked | 1 blocked track",
      "next: implement/blocker / Open blocker (1 unblock)",
      "",
      "chore/update-agents",
      "  o Update agents",
      "      in-progress",
      "",
      "chore/bump-deps",
      "  · Bump deps",
      "",
      "implement/blocker",
      "  · Open blocker",
      "      ready, 1 unblock",
      "",
      "implement/blocked-work",
      "  ! Blocked work",
      "      blocked by implement/blocker",
    ]);
  });

  it("keeps multi-line form for tracks that have steps", () => {
    const trackTask = makeTask({
      id: "tsk-aaaaaa",
      title: "Track epic",
      slug: "implement/track-epic",
    });
    const step = makeTask({
      id: "tsk-bbbbbb",
      title: "Step one",
      parentId: trackTask.id,
    });

    const projection = groupTasksByTrack([trackTask, step]);
    const lines = formatTaskStatusView(projection, { color: false });

    expect(lines).toEqual([
      "tasks: 1 open | 0 active | 1 ready | 0 blocked | 0 blocked tracks",
      "",
      "implement/track-epic",
      "  · Step one",
    ]);
  });

  it("renders only the header line when there are no tasks", () => {
    const projection = groupTasksByTrack([]);
    const lines = formatTaskStatusView(projection, { color: false });
    expect(lines).toEqual([
      "tasks: 0 open | 0 active | 0 ready | 0 blocked | 0 blocked tracks",
    ]);
  });

  it("renders only the header line when every task is completed and --all is not set", () => {
    const tasks: Task[] = [
      { ...makeTrack("tsk-aaaaaa", "implement/done", "Done"), status: "completed" } as Task,
    ];
    const projection = groupTasksByTrack(tasks);
    const lines = formatTaskStatusView(projection, { color: false, all: false });
    expect(lines).toEqual([
      "tasks: 0 open | 0 active | 0 ready | 0 blocked | 0 blocked tracks",
    ]);
  });

  it("B1: renders blocked-by with (done) suffix for already-completed blockers in mixed lists", () => {
    const doneBlocker = makeTrack("tsk-aaaaaa", "implement/done-blocker", "Done blocker");
    const completedDone: Task = { ...doneBlocker, status: "completed" };
    const stillOpen = makeTrack("tsk-bbbbbb", "implement/still-open", "Still open");
    const blockedTrack = makeTrack("tsk-cccccc", "implement/blocked", "Blocked container");
    const blockedStep = makeTask({
      id: "tsk-ccccc1",
      title: "Step",
      parentId: blockedTrack.id,
      blockedBy: [doneBlocker.id, stillOpen.id],
    });

    const projection = groupTasksByTrack([completedDone, stillOpen, blockedTrack, blockedStep]);
    const lines = formatTaskStatusView(projection, { color: false });
    const joined = lines.join("\n");
    expect(joined).toContain(
      "blocked by implement/done-blocker (done), implement/still-open",
    );
  });

  it("renders child-step blockers by title instead of raw id", () => {
    const track = makeTrack("tsk-aaaaaa", "epic/example", "Example");
    const blocker = makeTask({
      id: "tsk-bbbbbb",
      title: "Phase 0: prepare",
      parentId: track.id,
    });
    const blocked = makeTask({
      id: "tsk-cccccc",
      title: "Phase 1: build",
      parentId: track.id,
      blockedBy: [blocker.id],
    });

    const projection = groupTasksByTrack([track, blocker, blocked]);
    const lines = formatTaskStatusView(projection, { color: false });

    expect(lines.join("\n")).toContain("blocked by Phase 0: prepare");
  });
});

function makeTrack(id: string, slug: string, title: string): Task {
  return makeTask({ id, title, slug, createdAt: NOW });
}
