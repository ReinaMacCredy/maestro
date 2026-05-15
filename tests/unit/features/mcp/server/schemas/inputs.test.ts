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
  TaskFromSpecInput,
  TaskGetInput,
  TaskListInput,
  TaskShipInput,
  PrinciplePromoteInput,
  SetupCheckInput,
  SetupMigrateV2Input,
  VerdictRequestInput,
  VerdictShowInput,
} from "@/features/mcp/server/schemas/inputs.js";

describe("input schemas — id format", () => {
  it("accepts v1 task ids in TaskGetInput", () => {
    expect(TaskGetInput.safeParse({ id: "tsk-abc123" }).success).toBe(true);
  });

  it("accepts v2 task ids in TaskGetInput", () => {
    expect(TaskGetInput.safeParse({ id: "tsk-lp1abc-xy1234" }).success).toBe(true);
  });

  it("rejects malformed task ids", () => {
    expect(TaskGetInput.safeParse({ id: "tsk-ABC" }).success).toBe(false);
    expect(TaskGetInput.safeParse({ id: "tsk-" }).success).toBe(false);
    expect(TaskGetInput.safeParse({ id: "task-abc123" }).success).toBe(false);
  });

  it("accepts valid exec-plan ids in TaskListInput", () => {
    expect(TaskListInput.safeParse({ plan_id: "pln-1a2b3c4d5e6f-a1b2c3" }).success).toBe(true);
  });

  it("rejects v1 mission id format on TaskListInput plan_id", () => {
    expect(TaskListInput.safeParse({ plan_id: "msn-abc123" }).success).toBe(false);
  });
});

describe("strict mode (unknown fields)", () => {
  it("TaskFromSpecInput rejects unknown fields", () => {
    const r = TaskFromSpecInput.safeParse({ spec_path: "docs/specs/foo.md", plan_id: "pln-1a2b3c4d5e6f-a1b2c3" });
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

  it("validates v2 state enum", () => {
    expect(TaskListInput.safeParse({ state: "draft" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "claimed" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "doing" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "verifying" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "blocked" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "ready" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "shipped" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "abandoned" }).success).toBe(true);
    expect(TaskListInput.safeParse({ state: "fubar" }).success).toBe(false);
  });

  it("rejects v1-only status values", () => {
    expect(TaskListInput.safeParse({ state: "pending" }).success).toBe(false);
    expect(TaskListInput.safeParse({ state: "in_progress" }).success).toBe(false);
    expect(TaskListInput.safeParse({ state: "completed" }).success).toBe(false);
  });

  it("rejects removed v1 filters (type, priority, label, parentId, assignee)", () => {
    expect(TaskListInput.safeParse({ type: "bug" }).success).toBe(false);
    expect(TaskListInput.safeParse({ priority: 1 }).success).toBe(false);
    expect(TaskListInput.safeParse({ label: "release" }).success).toBe(false);
    expect(TaskListInput.safeParse({ parentId: "tsk-abc123" }).success).toBe(false);
    expect(TaskListInput.safeParse({ assignee: "someone" }).success).toBe(false);
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

describe("TaskFromSpecInput", () => {
  it("accepts a minimal payload", () => {
    expect(TaskFromSpecInput.safeParse({ spec_path: "docs/specs/foo.md" }).success).toBe(true);
  });

  it("rejects empty spec_path", () => {
    expect(TaskFromSpecInput.safeParse({ spec_path: "" }).success).toBe(false);
  });

  it("rejects missing spec_path", () => {
    expect(TaskFromSpecInput.safeParse({}).success).toBe(false);
  });
});

describe("TaskClaimInput", () => {
  it("requires an id", () => {
    expect(TaskClaimInput.safeParse({}).success).toBe(false);
  });

  it("accepts id with optional agent_id", () => {
    expect(TaskClaimInput.safeParse({ id: "tsk-abc123" }).success).toBe(true);
    expect(TaskClaimInput.safeParse({ id: "tsk-abc123", agent_id: "claude-code" }).success).toBe(true);
  });
});

describe("TaskShipInput", () => {
  it("requires an id", () => {
    expect(TaskShipInput.safeParse({}).success).toBe(false);
  });

  it("accepts id with optional pr_url", () => {
    expect(TaskShipInput.safeParse({ id: "tsk-abc123" }).success).toBe(true);
    expect(
      TaskShipInput.safeParse({ id: "tsk-abc123", pr_url: "https://github.com/owner/repo/pull/1" }).success,
    ).toBe(true);
  });

  it("rejects non-URL pr_url", () => {
    expect(TaskShipInput.safeParse({ id: "tsk-abc123", pr_url: "not-a-url" }).success).toBe(false);
  });
});

describe("TaskBlockInput (v2 — reason, not blockedTaskIds)", () => {
  it("requires id and reason", () => {
    expect(TaskBlockInput.safeParse({ id: "tsk-abc123" }).success).toBe(false);
    expect(TaskBlockInput.safeParse({ reason: "waiting on infra" }).success).toBe(false);
    expect(TaskBlockInput.safeParse({ id: "tsk-abc123", reason: "waiting on infra" }).success).toBe(true);
  });

  it("rejects removed v1 fields (blockedTaskIds, force)", () => {
    expect(
      TaskBlockInput.safeParse({
        id: "tsk-abc123",
        reason: "x",
        blockedTaskIds: ["tsk-def456"],
      }).success,
    ).toBe(false);
    expect(
      TaskBlockInput.safeParse({
        id: "tsk-abc123",
        reason: "x",
        force: true,
      }).success,
    ).toBe(false);
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

describe("PrinciplePromoteInput", () => {
  it("requires correction_id", () => {
    expect(PrinciplePromoteInput.safeParse({}).success).toBe(false);
    expect(PrinciplePromoteInput.safeParse({ correction_id: "evd-1714747200123-a1b2c3" }).success).toBe(true);
  });

  it("rejects empty correction_id", () => {
    expect(PrinciplePromoteInput.safeParse({ correction_id: "" }).success).toBe(false);
  });
});

describe("SetupCheckInput / SetupMigrateV2Input", () => {
  it("SetupCheckInput accepts empty payload", () => {
    expect(SetupCheckInput.safeParse({}).success).toBe(true);
  });

  it("SetupMigrateV2Input accepts optional flags", () => {
    expect(SetupMigrateV2Input.safeParse({}).success).toBe(true);
    expect(SetupMigrateV2Input.safeParse({ dry_run: true }).success).toBe(true);
    expect(SetupMigrateV2Input.safeParse({ force: true }).success).toBe(true);
    expect(SetupMigrateV2Input.safeParse({ dry_run: true, force: true }).success).toBe(true);
  });

  it("SetupMigrateV2Input rejects unknown fields", () => {
    expect(SetupMigrateV2Input.safeParse({ unknown: true }).success).toBe(false);
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
