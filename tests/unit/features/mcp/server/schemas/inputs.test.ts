import { describe, expect, it } from "bun:test";
import { z } from "zod";
import {
  ContractAmendInput,
  ContractShowInput,
  EvidenceListInput,
  EvidenceRecordInput,
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

function schemaOf<T extends z.ZodRawShape>(shape: T) {
  return z.object(shape);
}

describe("input schemas — id format", () => {
  it("accepts valid task ids in TaskGetInput", () => {
    const r = schemaOf(TaskGetInput).safeParse({ id: "tsk-abc123" });
    expect(r.success).toBe(true);
  });

  it("rejects malformed task ids", () => {
    expect(schemaOf(TaskGetInput).safeParse({ id: "tsk-ABC" }).success).toBe(false);
    expect(schemaOf(TaskGetInput).safeParse({ id: "tsk-" }).success).toBe(false);
    expect(schemaOf(TaskGetInput).safeParse({ id: "task-abc123" }).success).toBe(false);
  });

  it("accepts valid mission ids in TaskListInput", () => {
    const r = schemaOf(TaskListInput).safeParse({ missionId: "msn-abc123" });
    expect(r.success).toBe(true);
  });
});

describe("TaskListInput", () => {
  it("accepts an empty payload (all fields optional)", () => {
    const r = schemaOf(TaskListInput).safeParse({});
    expect(r.success).toBe(true);
  });

  it("validates the status enum", () => {
    expect(schemaOf(TaskListInput).safeParse({ status: "pending" }).success).toBe(true);
    expect(schemaOf(TaskListInput).safeParse({ status: "in_progress" }).success).toBe(true);
    expect(schemaOf(TaskListInput).safeParse({ status: "completed" }).success).toBe(true);
    expect(schemaOf(TaskListInput).safeParse({ status: "fubar" }).success).toBe(false);
  });

  it("clamps limit to 1..100", () => {
    expect(schemaOf(TaskListInput).safeParse({ limit: 0 }).success).toBe(false);
    expect(schemaOf(TaskListInput).safeParse({ limit: 101 }).success).toBe(false);
    expect(schemaOf(TaskListInput).safeParse({ limit: 50 }).success).toBe(true);
  });

  it("rejects negative offsets", () => {
    expect(schemaOf(TaskListInput).safeParse({ offset: -1 }).success).toBe(false);
    expect(schemaOf(TaskListInput).safeParse({ offset: 0 }).success).toBe(true);
  });
});

describe("TaskCreateInput", () => {
  it("accepts a minimal payload", () => {
    expect(schemaOf(TaskCreateInput).safeParse({ title: "do a thing" }).success).toBe(true);
  });

  it("rejects an empty title", () => {
    expect(schemaOf(TaskCreateInput).safeParse({ title: "" }).success).toBe(false);
  });

  it("rejects a title longer than 200 chars", () => {
    expect(schemaOf(TaskCreateInput).safeParse({ title: "x".repeat(201) }).success).toBe(false);
    expect(schemaOf(TaskCreateInput).safeParse({ title: "x".repeat(200) }).success).toBe(true);
  });
});

describe("TaskClaimInput / TaskCompleteInput", () => {
  it("require an id", () => {
    expect(schemaOf(TaskClaimInput).safeParse({}).success).toBe(false);
    expect(schemaOf(TaskCompleteInput).safeParse({}).success).toBe(false);
  });

  it("TaskCompleteInput accepts an optional summary", () => {
    expect(
      schemaOf(TaskCompleteInput).safeParse({ id: "tsk-abc123", summary: "done" }).success,
    ).toBe(true);
  });
});

describe("TaskBlockInput / TaskUnblockInput", () => {
  it("require at least one blockedTaskId", () => {
    expect(
      schemaOf(TaskBlockInput).safeParse({ id: "tsk-abc123", blockedTaskIds: [] }).success,
    ).toBe(false);
    expect(
      schemaOf(TaskBlockInput).safeParse({
        id: "tsk-abc123",
        blockedTaskIds: ["tsk-def456"],
      }).success,
    ).toBe(true);
  });

  it("validate every id in blockedTaskIds", () => {
    expect(
      schemaOf(TaskUnblockInput).safeParse({
        id: "tsk-abc123",
        blockedTaskIds: ["tsk-def456", "bogus"],
      }).success,
    ).toBe(false);
  });

  it("accept optional force flag", () => {
    expect(
      schemaOf(TaskBlockInput).safeParse({
        id: "tsk-abc123",
        blockedTaskIds: ["tsk-def456"],
        force: true,
      }).success,
    ).toBe(true);
  });
});

describe("EvidenceListInput / EvidenceRecordInput", () => {
  it("EvidenceListInput requires taskId", () => {
    expect(schemaOf(EvidenceListInput).safeParse({}).success).toBe(false);
    expect(schemaOf(EvidenceListInput).safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
  });

  it("EvidenceListInput validates witnessLevel enum", () => {
    expect(
      schemaOf(EvidenceListInput).safeParse({
        taskId: "tsk-abc123",
        witnessLevel: "witnessed-by-maestro",
      }).success,
    ).toBe(true);
    expect(
      schemaOf(EvidenceListInput).safeParse({
        taskId: "tsk-abc123",
        witnessLevel: "trust-me-bro",
      }).success,
    ).toBe(false);
  });

  it("EvidenceRecordInput accepts a command-style payload", () => {
    expect(
      schemaOf(EvidenceRecordInput).safeParse({
        taskId: "tsk-abc123",
        command: "bun test",
        exitCode: 0,
      }).success,
    ).toBe(true);
  });

  it("EvidenceRecordInput accepts a note-only payload", () => {
    expect(
      schemaOf(EvidenceRecordInput).safeParse({
        taskId: "tsk-abc123",
        note: "verified manually",
      }).success,
    ).toBe(true);
  });
});

describe("VerdictShowInput / VerdictRequestInput", () => {
  it("VerdictShowInput requires taskId, optional verdict id", () => {
    expect(schemaOf(VerdictShowInput).safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
    expect(
      schemaOf(VerdictShowInput).safeParse({
        taskId: "tsk-abc123",
        id: "vdt-abc123",
      }).success,
    ).toBe(true);
    expect(
      schemaOf(VerdictShowInput).safeParse({
        taskId: "tsk-abc123",
        id: "bogus",
      }).success,
    ).toBe(false);
  });

  it("VerdictRequestInput accepts an optional base ref", () => {
    expect(
      schemaOf(VerdictRequestInput).safeParse({
        taskId: "tsk-abc123",
        base: "origin/main",
      }).success,
    ).toBe(true);
  });
});

describe("ContractShowInput / ContractAmendInput", () => {
  it("ContractShowInput accepts an optional version", () => {
    expect(
      schemaOf(ContractShowInput).safeParse({ taskId: "tsk-abc123" }).success,
    ).toBe(true);
    expect(
      schemaOf(ContractShowInput).safeParse({
        taskId: "tsk-abc123",
        version: 3,
      }).success,
    ).toBe(true);
    expect(
      schemaOf(ContractShowInput).safeParse({
        taskId: "tsk-abc123",
        version: 0,
      }).success,
    ).toBe(false);
  });

  it("ContractAmendInput requires a non-empty reason", () => {
    expect(
      schemaOf(ContractAmendInput).safeParse({
        taskId: "tsk-abc123",
        reason: "",
      }).success,
    ).toBe(false);
    expect(
      schemaOf(ContractAmendInput).safeParse({
        taskId: "tsk-abc123",
        addPaths: ["src/a.ts"],
        reason: "scope creep",
      }).success,
    ).toBe(true);
  });
});

describe("PolicyCheckInput", () => {
  it("requires taskId", () => {
    expect(schemaOf(PolicyCheckInput).safeParse({}).success).toBe(false);
    expect(schemaOf(PolicyCheckInput).safeParse({ taskId: "tsk-abc123" }).success).toBe(true);
  });
});
