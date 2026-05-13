import { describe, expect, it } from "bun:test";
import {
  ContractAmendInput,
  ContractShowInput,
  EvidenceListInput,
  EvidenceRecordInput,
  HandoffListInput,
  HandoffOpenForTaskInput,
  HandoffPickupInput,
  HandoffShowInput,
  PolicyCheckInput,
  TaskBlockInput,
  TaskClaimInput,
  TaskCompleteInput,
  TaskCreateInput,
  TaskGetInput,
  TaskListInput,
  TaskUnblockInput,
  VerdictRequestInput,
  VerdictShowInput,
} from "@/features/mcp/server/schemas/inputs.js";

describe("input schemas — id format", () => {
  it("accepts valid task ids in TaskGetInput", () => {
    expect(TaskGetInput.safeParse({ id: "tsk-abc123" }).success).toBe(true);
  });

  it("rejects malformed task ids", () => {
    expect(TaskGetInput.safeParse({ id: "tsk-ABC" }).success).toBe(false);
    expect(TaskGetInput.safeParse({ id: "tsk-" }).success).toBe(false);
    expect(TaskGetInput.safeParse({ id: "task-abc123" }).success).toBe(false);
  });

  it("accepts valid mission ids in TaskListInput", () => {
    expect(TaskListInput.safeParse({ missionId: "msn-abc123" }).success).toBe(true);
  });
});

describe("strict mode (unknown fields)", () => {
  it("TaskCreateInput rejects unknown fields like missionId", () => {
    const r = TaskCreateInput.safeParse({ title: "ok", missionId: "msn-abc123" });
    expect(r.success).toBe(false);
  });

  it("TaskCreateInput rejects unknown riskClass field", () => {
    const r = TaskCreateInput.safeParse({ title: "ok", riskClass: "low" });
    expect(r.success).toBe(false);
  });

  it("TaskListInput rejects typo'd field names", () => {
    const r = TaskListInput.safeParse({ statuss: "pending" });
    expect(r.success).toBe(false);
  });

  it("ContractAmendInput rejects unknown fields", () => {
    const r = ContractAmendInput.safeParse({
      taskId: "tsk-abc123",
      reason: "x",
      sneaky: 1,
    });
    expect(r.success).toBe(false);
  });
});

describe("TaskListInput", () => {
  it("accepts an empty payload (all fields optional)", () => {
    expect(TaskListInput.safeParse({}).success).toBe(true);
  });

  it("validates the status enum", () => {
    expect(TaskListInput.safeParse({ status: "pending" }).success).toBe(true);
    expect(TaskListInput.safeParse({ status: "in_progress" }).success).toBe(true);
    expect(TaskListInput.safeParse({ status: "completed" }).success).toBe(true);
    expect(TaskListInput.safeParse({ status: "fubar" }).success).toBe(false);
  });

  it("validates the type enum", () => {
    expect(TaskListInput.safeParse({ type: "bug" }).success).toBe(true);
    expect(TaskListInput.safeParse({ type: "feature" }).success).toBe(true);
    expect(TaskListInput.safeParse({ type: "spike" }).success).toBe(false);
  });

  it("validates the priority literal range", () => {
    for (const p of [0, 1, 2, 3, 4]) {
      expect(TaskListInput.safeParse({ priority: p }).success).toBe(true);
    }
    expect(TaskListInput.safeParse({ priority: 5 }).success).toBe(false);
    expect(TaskListInput.safeParse({ priority: -1 }).success).toBe(false);
  });

  it("accepts label / parentId / assignee filters", () => {
    expect(TaskListInput.safeParse({ label: "release-prep" }).success).toBe(true);
    expect(TaskListInput.safeParse({ parentId: "tsk-aa0001" }).success).toBe(true);
    expect(TaskListInput.safeParse({ assignee: "session:ada" }).success).toBe(true);
  });

  it("rejects empty-string label or assignee", () => {
    expect(TaskListInput.safeParse({ label: "" }).success).toBe(false);
    expect(TaskListInput.safeParse({ assignee: "" }).success).toBe(false);
  });

  it("clamps limit to 1..100", () => {
    expect(TaskListInput.safeParse({ limit: 0 }).success).toBe(false);
    expect(TaskListInput.safeParse({ limit: 101 }).success).toBe(false);
    expect(TaskListInput.safeParse({ limit: 50 }).success).toBe(true);
  });

  it("rejects negative offsets", () => {
    expect(TaskListInput.safeParse({ offset: -1 }).success).toBe(false);
    expect(TaskListInput.safeParse({ offset: 0 }).success).toBe(true);
  });
});

describe("TaskCreateInput", () => {
  it("accepts a minimal payload", () => {
    expect(TaskCreateInput.safeParse({ title: "do a thing" }).success).toBe(true);
  });

  it("rejects an empty title", () => {
    expect(TaskCreateInput.safeParse({ title: "" }).success).toBe(false);
  });

  it("rejects a title longer than 200 chars", () => {
    expect(TaskCreateInput.safeParse({ title: "x".repeat(201) }).success).toBe(false);
    expect(TaskCreateInput.safeParse({ title: "x".repeat(200) }).success).toBe(true);
  });
});

describe("TaskClaimInput / TaskCompleteInput", () => {
  it("require an id", () => {
    expect(TaskClaimInput.safeParse({}).success).toBe(false);
    expect(TaskCompleteInput.safeParse({}).success).toBe(false);
  });

  it("TaskCompleteInput accepts an optional summary", () => {
    expect(
      TaskCompleteInput.safeParse({ id: "tsk-abc123", summary: "done" }).success,
    ).toBe(true);
  });
});

describe("TaskBlockInput / TaskUnblockInput", () => {
  it("require at least one blockedTaskId", () => {
    expect(
      TaskBlockInput.safeParse({ id: "tsk-abc123", blockedTaskIds: [] }).success,
    ).toBe(false);
    expect(
      TaskBlockInput.safeParse({
        id: "tsk-abc123",
        blockedTaskIds: ["tsk-def456"],
      }).success,
    ).toBe(true);
  });

  it("validate every id in blockedTaskIds", () => {
    expect(
      TaskUnblockInput.safeParse({
        id: "tsk-abc123",
        blockedTaskIds: ["tsk-def456", "bogus"],
      }).success,
    ).toBe(false);
  });

  it("accept optional force flag", () => {
    expect(
      TaskBlockInput.safeParse({
        id: "tsk-abc123",
        blockedTaskIds: ["tsk-def456"],
        force: true,
      }).success,
    ).toBe(true);
  });
});

describe("EvidenceListInput / EvidenceRecordInput", () => {
  it("EvidenceListInput requires taskId", () => {
    expect(EvidenceListInput.safeParse({}).success).toBe(false);
    expect(EvidenceListInput.safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
  });

  it("EvidenceListInput validates witnessLevel enum", () => {
    expect(
      EvidenceListInput.safeParse({
        taskId: "tsk-abc123",
        witnessLevel: "witnessed-by-maestro",
      }).success,
    ).toBe(true);
    expect(
      EvidenceListInput.safeParse({
        taskId: "tsk-abc123",
        witnessLevel: "trust-me-bro",
      }).success,
    ).toBe(false);
  });

  it("EvidenceRecordInput accepts a command-style payload", () => {
    expect(
      EvidenceRecordInput.safeParse({
        taskId: "tsk-abc123",
        command: "bun test",
        exitCode: 0,
      }).success,
    ).toBe(true);
  });

  it("EvidenceRecordInput accepts a note-only payload", () => {
    expect(
      EvidenceRecordInput.safeParse({
        taskId: "tsk-abc123",
        note: "verified manually",
      }).success,
    ).toBe(true);
  });

  it("EvidenceRecordInput rejects passing both command and note", () => {
    const r = EvidenceRecordInput.safeParse({
      taskId: "tsk-abc123",
      command: "bun test",
      exitCode: 0,
      note: "and also a note",
    });
    expect(r.success).toBe(false);
    if (!r.success) {
      expect(r.error.issues[0]?.message).toContain("exactly one");
    }
  });

  it("EvidenceRecordInput rejects passing neither command nor note", () => {
    const r = EvidenceRecordInput.safeParse({ taskId: "tsk-abc123" });
    expect(r.success).toBe(false);
  });

  it("EvidenceRecordInput rejects command without exitCode", () => {
    const r = EvidenceRecordInput.safeParse({
      taskId: "tsk-abc123",
      command: "bun test",
    });
    expect(r.success).toBe(false);
    if (!r.success) {
      expect(r.error.issues.some((i) => i.message.includes("exitCode"))).toBe(true);
    }
  });
});

describe("VerdictShowInput / VerdictRequestInput", () => {
  it("VerdictShowInput requires taskId, optional verdict id", () => {
    expect(VerdictShowInput.safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
    expect(
      VerdictShowInput.safeParse({
        taskId: "tsk-abc123",
        id: "vrd-1714747200123-a1b2c3",
      }).success,
    ).toBe(true);
    expect(
      VerdictShowInput.safeParse({
        taskId: "tsk-abc123",
        id: "vdt-abc123",
      }).success,
    ).toBe(false);
    expect(
      VerdictShowInput.safeParse({
        taskId: "tsk-abc123",
        id: "bogus",
      }).success,
    ).toBe(false);
  });

  it("VerdictRequestInput accepts an optional base ref", () => {
    expect(
      VerdictRequestInput.safeParse({
        taskId: "tsk-abc123",
        base: "origin/main",
      }).success,
    ).toBe(true);
  });
});

describe("ContractShowInput / ContractAmendInput", () => {
  it("ContractShowInput accepts an optional version", () => {
    expect(ContractShowInput.safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
    expect(
      ContractShowInput.safeParse({
        taskId: "tsk-abc123",
        version: 3,
      }).success,
    ).toBe(true);
    expect(
      ContractShowInput.safeParse({
        taskId: "tsk-abc123",
        version: 0,
      }).success,
    ).toBe(false);
  });

  it("ContractAmendInput requires a non-empty reason", () => {
    expect(
      ContractAmendInput.safeParse({
        taskId: "tsk-abc123",
        reason: "",
      }).success,
    ).toBe(false);
    expect(
      ContractAmendInput.safeParse({
        taskId: "tsk-abc123",
        addPaths: ["src/a.ts"],
        reason: "scope creep",
      }).success,
    ).toBe(true);
  });
});

describe("PolicyCheckInput", () => {
  it("requires taskId", () => {
    expect(PolicyCheckInput.safeParse({}).success).toBe(false);
    expect(PolicyCheckInput.safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
  });
});

describe("HandoffListInput", () => {
  it("accepts an empty payload", () => {
    expect(HandoffListInput.safeParse({}).success).toBe(true);
  });

  it("validates handoff agent enum", () => {
    expect(HandoffListInput.safeParse({ agent: "codex" }).success).toBe(true);
    expect(HandoffListInput.safeParse({ agent: "claude" }).success).toBe(true);
    expect(HandoffListInput.safeParse({ agent: "hermes" }).success).toBe(true);
    expect(HandoffListInput.safeParse({ agent: "gpt" }).success).toBe(false);
  });

  it("validates displayState enum", () => {
    for (const s of ["open", "consumed", "completed", "failed"]) {
      expect(HandoffListInput.safeParse({ displayState: s }).success).toBe(true);
    }
    expect(HandoffListInput.safeParse({ displayState: "draft" }).success).toBe(false);
  });

  it("filters by linked task id", () => {
    expect(HandoffListInput.safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
    expect(HandoffListInput.safeParse({ taskId: "bogus" }).success).toBe(false);
  });

  it("schema accepts openOnly + displayState together (handler enforces mutual-exclusion)", () => {
    // The schema stays a plain ZodObject so the SDK can serialize properties
    // for `tools/list`. Mutual-exclusion is enforced by the handler with code
    // INVALID_FILTER_COMBINATION (covered in handoff-tools tests).
    expect(
      HandoffListInput.safeParse({ openOnly: true, displayState: "consumed" }).success,
    ).toBe(true);
  });

  it("rejects unknown fields (strict)", () => {
    expect(HandoffListInput.safeParse({ statuss: "open" }).success).toBe(false);
  });
});

describe("HandoffShowInput / HandoffOpenForTaskInput", () => {
  it("HandoffShowInput accepts handoff id formats", () => {
    expect(HandoffShowInput.safeParse({ id: "bold-otter-1" }).success).toBe(true);
    expect(HandoffShowInput.safeParse({ id: "2026-05-08-001" }).success).toBe(true);
    expect(HandoffShowInput.safeParse({ id: "Bold-Otter-1" }).success).toBe(false);
  });

  it("HandoffOpenForTaskInput requires a valid task id", () => {
    expect(HandoffOpenForTaskInput.safeParse({}).success).toBe(false);
    expect(HandoffOpenForTaskInput.safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
    expect(HandoffOpenForTaskInput.safeParse({ taskId: "task-abc123" }).success).toBe(false);
  });
});

describe("HandoffPickupInput", () => {
  it("requires id and actorAgent", () => {
    expect(HandoffPickupInput.safeParse({}).success).toBe(false);
    expect(
      HandoffPickupInput.safeParse({ id: "bold-otter-1", actorAgent: "codex" }).success,
    ).toBe(true);
  });

  it("rejects unknown actorAgent values", () => {
    expect(
      HandoffPickupInput.safeParse({ id: "bold-otter-1", actorAgent: "gpt" }).success,
    ).toBe(false);
  });

  it("accepts optional standalone, ownerId, actorSessionId", () => {
    expect(
      HandoffPickupInput.safeParse({
        id: "bold-otter-1",
        actorAgent: "claude",
        actorSessionId: "session:1",
        ownerId: "claude:session:1",
        standalone: true,
      }).success,
    ).toBe(true);
  });
});
