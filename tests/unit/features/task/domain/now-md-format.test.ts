import { describe, expect, it } from "bun:test";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
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

function contract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: 1,
    id: "c-123abc",
    taskId: "tsk-000001",
    repoRoot: "/repo",
    status: "locked",
    createdAt: "2026-04-21T00:00:00.000Z",
    intent: "Keep the task scoped",
    scope: {
      filesExpected: ["src/features/task/**"],
      filesForbidden: [],
    },
    doneWhen: [
      {
        id: "dw-123abc",
        text: "criterion one",
        kind: "manual",
      },
      {
        id: "dw-456def",
        text: "criterion two",
        kind: "manual",
        met: true,
        metAt: "2026-04-21T01:00:00.000Z",
        metBy: "alice",
      },
    ],
    amendments: [],
    createdBy: "alice",
    lockedBy: "alice",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
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

  it("renders active contract progress for in-progress tasks", () => {
    const md = buildNowMd({
      tasks: [
        task({
          id: "tsk-contract",
          title: "contracted work",
          status: "in_progress",
          contractId: "c-123abc",
        }),
      ],
      contracts: new Map([
        [
          "c-123abc",
          contract({
            id: "c-123abc",
            taskId: "tsk-contract",
          }),
        ],
      ]),
      now: new Date("2026-04-21T12:00:00.000Z"),
    });

    expect(md).toContain("Contract: c-123abc (locked, 1/2 done-when met, scope: src/features/task/**)");
  });

  it("renders inherited ownership details for transferred active contracts", () => {
    const md = buildNowMd({
      tasks: [
        task({
          id: "tsk-contract",
          title: "contracted work",
          status: "in_progress",
          contractId: "c-123abc",
        }),
      ],
      contracts: new Map([
        [
          "c-123abc",
          contract({
            id: "c-123abc",
            taskId: "tsk-contract",
            lockedBy: "operator-next",
            ownershipHistory: [
              {
                from: "codex-staleowner1",
                to: "operator-next",
                at: "2026-04-21T01:30:00.000Z",
                reason: "claim_reclaim",
              },
            ],
          }),
        ],
      ]),
      now: new Date("2026-04-21T12:00:00.000Z"),
    });

    expect(md).toContain("Contract: c-123abc (locked, inherited from codex-staleowner1, 1/2 done-when met, scope: src/features/task/**)");
  });

  it("skips discarded or completed contracts in NOW.md", () => {
    const md = buildNowMd({
      tasks: [
        task({
          id: "tsk-contract",
          title: "contracted work",
          status: "in_progress",
          contractId: "c-123abc",
        }),
      ],
      contracts: new Map([
        [
          "c-123abc",
          contract({
            id: "c-123abc",
            taskId: "tsk-contract",
            status: "fulfilled",
          }),
        ],
      ]),
      now: new Date("2026-04-21T12:00:00.000Z"),
    });

    expect(md).not.toContain("Contract: c-123abc");
  });
});
